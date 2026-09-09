use std::fs;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, Generate, KeyInit, Nonce, Payload};
use aes_gcm::Aes256Gcm;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zeroize::Zeroizing;

use super::ENVELOPE_VERSION;

const NONCE_LEN: usize = 12;
/// Domain separator for the stable management-reply identity derived from the root credential.
const REPLY_IDENTITY_INFO: &[u8] = b"keycast-management-reply-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeContext<'a> {
    pub team_id: i64,
    pub record_id: i64,
    pub public_key: &'a str,
    pub purpose: &'a str,
}

#[derive(Clone)]
pub struct EnvelopeCipher {
    cipher: Aes256Gcm,
    key_id: String,
    /// Seed for the stable management-reply identity. Never leaves this type.
    reply_seed: Zeroizing<[u8; 32]>,
}

#[derive(Debug, Error)]
pub enum EnvelopeError {
    #[error("root credential not found")]
    CredentialNotFound,
    #[error("root credential permissions allow group or world access: {0}")]
    InsecurePermissions(PathBuf),
    #[error("failed to read root credential: {0}")]
    Read(#[from] std::io::Error),
    #[error("root credential must be 32 bytes encoded as base64 or hex")]
    InvalidCredential,
    #[error("both CREDENTIALS_DIRECTORY and KEYCAST_ROOT_KEY_FILE are set; unset one so the root credential source is unambiguous")]
    AmbiguousCredential,
    #[error("encrypted envelope is truncated")]
    Truncated,
    #[error("envelope encryption failed")]
    Encrypt,
    #[error("envelope authentication failed")]
    Decrypt,
}

impl EnvelopeCipher {
    pub fn load() -> Result<Self, EnvelopeError> {
        // The systemd credential used to win silently. Loading the wrong key fails
        // closed later, but an ambiguous configuration should be rejected outright.
        if std::env::var_os("CREDENTIALS_DIRECTORY").is_some()
            && std::env::var_os("KEYCAST_ROOT_KEY_FILE").is_some()
        {
            return Err(EnvelopeError::AmbiguousCredential);
        }
        let path = credential_path().ok_or(EnvelopeError::CredentialNotFound)?;
        Self::from_file(&path)
    }

    pub fn from_file(path: &Path) -> Result<Self, EnvelopeError> {
        ensure_private_permissions(path)?;
        let encoded = Zeroizing::new(fs::read_to_string(path)?);
        let key = decode_root_key(encoded.trim())?;
        Ok(Self::from_key(key))
    }

    pub fn from_key(key: Zeroizing<[u8; 32]>) -> Self {
        let digest = Sha256::digest(key.as_ref());
        let key_id = digest[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        // Domain-separated derivation. Publishing the reply public key reveals
        // nothing about the root credential, and the seed is never exported.
        let mut reply_seed = Zeroizing::new([0_u8; 32]);
        let mut hasher = Sha256::new();
        hasher.update(REPLY_IDENTITY_INFO);
        hasher.update(key.as_ref());
        reply_seed.copy_from_slice(&hasher.finalize());
        let cipher = Aes256Gcm::new((&*key).into());
        Self {
            cipher,
            key_id,
            reply_seed,
        }
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Stable identity used to encrypt management replies, so a browser can
    /// authenticate their origin instead of trusting a per-reply ephemeral key.
    pub fn management_reply_keys(&self) -> Result<nostr::prelude::Keys, EnvelopeError> {
        let mut candidate = self.reply_seed.clone();
        // A SHA-256 digest is a valid secp256k1 scalar with overwhelming probability;
        // rehash on the astronomically unlikely miss rather than failing the instance.
        for attempt in 0_u8..=8 {
            if let Ok(secret) = nostr::prelude::SecretKey::from_slice(candidate.as_ref()) {
                return Ok(nostr::prelude::Keys::new(secret));
            }
            let mut hasher = Sha256::new();
            hasher.update(REPLY_IDENTITY_INFO);
            hasher.update([attempt]);
            hasher.update(candidate.as_ref());
            candidate.copy_from_slice(&hasher.finalize());
        }
        Err(EnvelopeError::InvalidCredential)
    }

    /// Hex public key of [`Self::management_reply_keys`], safe to publish.
    pub fn management_reply_public_key(&self) -> Option<String> {
        self.management_reply_keys()
            .ok()
            .map(|keys| keys.public_key().to_hex())
    }

    pub fn seal(
        &self,
        plaintext: &[u8],
        context: &EnvelopeContext<'_>,
    ) -> Result<Vec<u8>, EnvelopeError> {
        let nonce = Nonce::<Aes256Gcm>::generate();
        let aad = associated_data(context);
        let ciphertext = self
            .cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| EnvelopeError::Encrypt)?;
        let mut envelope = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        envelope.extend_from_slice(&nonce);
        envelope.extend_from_slice(&ciphertext);
        Ok(envelope)
    }

    pub fn open(
        &self,
        envelope: &[u8],
        context: &EnvelopeContext<'_>,
    ) -> Result<Zeroizing<Vec<u8>>, EnvelopeError> {
        if envelope.len() <= NONCE_LEN {
            return Err(EnvelopeError::Truncated);
        }
        let (nonce, ciphertext) = envelope.split_at(NONCE_LEN);
        let nonce: &Nonce<Aes256Gcm> = nonce.try_into().map_err(|_| EnvelopeError::Truncated)?;
        let aad = associated_data(context);
        self.cipher
            .decrypt(
                nonce,
                Payload {
                    msg: ciphertext,
                    aad: &aad,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| EnvelopeError::Decrypt)
    }
}

fn associated_data(context: &EnvelopeContext<'_>) -> Vec<u8> {
    format!(
        "keycast\0v{ENVELOPE_VERSION}\0{}\0{}\0{}\0{}",
        context.team_id, context.record_id, context.public_key, context.purpose
    )
    .into_bytes()
}

pub fn credential_path() -> Option<PathBuf> {
    if let Ok(directory) = std::env::var("CREDENTIALS_DIRECTORY") {
        let name = std::env::var("KEYCAST_ROOT_KEY_CREDENTIAL")
            .unwrap_or_else(|_| "keycast-root-key".to_string());
        return Some(PathBuf::from(directory).join(name));
    }
    std::env::var_os("KEYCAST_ROOT_KEY_FILE").map(PathBuf::from)
}

fn decode_root_key(encoded: &str) -> Result<Zeroizing<[u8; 32]>, EnvelopeError> {
    let decoded = if encoded.len() == 64 {
        decode_hex(encoded)
    } else {
        BASE64.decode(encoded).map_err(|_| ())
    };
    let decoded = Zeroizing::new(decoded.map_err(|_| EnvelopeError::InvalidCredential)?);
    let mut key = Zeroizing::new([0u8; 32]);
    if decoded.len() != 32 {
        return Err(EnvelopeError::InvalidCredential);
    }
    key.copy_from_slice(&decoded);
    Ok(key)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ()> {
    // `from_str_radix` accepts a leading sign, so require strict hex digits first.
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).map_err(|_| ())?;
            u8::from_str_radix(pair, 16).map_err(|_| ())
        })
        .collect()
}

#[cfg(unix)]
fn ensure_private_permissions(path: &Path) -> Result<(), EnvelopeError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(EnvelopeError::InvalidCredential);
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(EnvelopeError::InsecurePermissions(path.to_path_buf()));
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_private_permissions(path: &Path) -> Result<(), EnvelopeError> {
    fs::metadata(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cipher() -> EnvelopeCipher {
        EnvelopeCipher::from_key(Zeroizing::new([7_u8; 32]))
    }

    #[test]
    fn envelope_round_trips_only_with_exact_context() {
        let first = EnvelopeContext {
            team_id: 1,
            record_id: 2,
            public_key: "abc",
            purpose: "stored-key",
        };
        let second = EnvelopeContext {
            record_id: 3,
            ..first.clone()
        };
        let envelope = cipher().seal(b"secret", &first).unwrap();
        assert_eq!(&*cipher().open(&envelope, &first).unwrap(), b"secret");
        assert!(matches!(
            cipher().open(&envelope, &second),
            Err(EnvelopeError::Decrypt)
        ));
    }

    #[test]
    fn key_identifier_is_stable_and_not_the_key() {
        assert_eq!(cipher().key_id().len(), 16);
        assert!(!cipher().key_id().contains("0707"));
    }
    #[test]
    fn management_reply_identity_is_stable_distinct_and_hides_the_root() {
        let first = cipher();
        let again = EnvelopeCipher::from_key(Zeroizing::new([7_u8; 32]));
        let other = EnvelopeCipher::from_key(Zeroizing::new([8_u8; 32]));

        let public = first.management_reply_public_key().expect("reply identity");
        assert_eq!(public.len(), 64);
        assert_eq!(
            public,
            again
                .management_reply_public_key()
                .expect("stable identity")
        );
        assert_ne!(
            public,
            other.management_reply_public_key().expect("reply identity")
        );

        // The published identity must not disclose the root credential or its fingerprint.
        let secret = first
            .management_reply_keys()
            .expect("reply identity")
            .secret_key()
            .to_secret_hex();
        assert_ne!(secret, "07".repeat(32));
        assert!(!public.contains(first.key_id()));
        assert!(!secret.contains("0707070707"));
    }

    #[test]
    fn both_credential_sources_set_is_rejected_instead_of_silently_preferring_one() {
        // These variables are not read by any other test in this crate.
        static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = GUARD.lock().unwrap();
        std::env::set_var("CREDENTIALS_DIRECTORY", "/run/credentials/keycast");
        std::env::set_var("KEYCAST_ROOT_KEY_FILE", "/run/secrets/keycast-root-key");
        assert!(matches!(
            EnvelopeCipher::load(),
            Err(EnvelopeError::AmbiguousCredential)
        ));
        std::env::remove_var("CREDENTIALS_DIRECTORY");
        std::env::remove_var("KEYCAST_ROOT_KEY_FILE");
        assert!(matches!(
            EnvelopeCipher::load(),
            Err(EnvelopeError::CredentialNotFound)
        ));
    }

    #[test]
    fn root_decoder_accepts_hex_and_base64_and_rejects_invalid_lengths() {
        assert_eq!(*decode_root_key(&"07".repeat(32)).unwrap(), [7; 32]);
        assert_eq!(*decode_root_key(&BASE64.encode([7; 32])).unwrap(), [7; 32]);
        assert!(decode_root_key(&"x".repeat(64)).is_err());
        // A signed hex pair must not be accepted as a byte.
        assert!(decode_root_key(&format!("+f{}", "0".repeat(62))).is_err());
        assert!(decode_root_key(&BASE64.encode([7; 31])).is_err());
    }
}
