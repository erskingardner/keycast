//! Bounded-memory backup format. The encrypted manifest binds size, root and a random archive ID;
//! each AEAD chunk binds that ID and its position. Truncation, concatenation and splicing fail.
use keycast_core::v2::envelope::{EnvelopeCipher, EnvelopeContext};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use zeroize::{Zeroize, Zeroizing};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub(crate) const MAX_DATABASE: u64 = 256 * 1024 * 1024;
const CHUNK: usize = 1024 * 1024;
const MANIFEST: EnvelopeContext<'static> = EnvelopeContext {
    team_id: 0,
    record_id: 0,
    public_key: "backup",
    purpose: "keycast-backup-v3-manifest",
};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub root: String,
    archive_id: String,
    length: u64,
}
impl Drop for Manifest {
    fn drop(&mut self) {
        self.root.zeroize();
    }
}
fn write_frame(output: &mut impl Write, bytes: &[u8]) -> Result<()> {
    output.write_all(&u32::try_from(bytes.len())?.to_be_bytes())?;
    output.write_all(bytes)?;
    Ok(())
}
fn read_frame(input: &mut impl Read, bound: usize) -> Result<Vec<u8>> {
    let mut length = [0u8; 4];
    input.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > bound {
        return Err("invalid backup frame length".into());
    }
    let mut bytes = vec![0u8; length];
    input.read_exact(&mut bytes)?;
    Ok(bytes)
}
pub(crate) fn seal(
    cipher: &EnvelopeCipher,
    root: &str,
    length: u64,
    input: &mut impl Read,
    output: &mut impl Write,
) -> Result<()> {
    if length == 0 || length > MAX_DATABASE || root.len() > 64 {
        return Err("backup input exceeds supported bound".into());
    }
    let manifest = Manifest {
        root: root.to_owned(),
        archive_id: nostr::prelude::Keys::generate().public_key().to_hex(),
        length,
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&manifest)?);
    output.write_all(b"KCB3")?;
    write_frame(output, &cipher.seal(&bytes, &MANIFEST)?)?;
    let mut remaining = length;
    let mut position = 1;
    let mut buffer = Zeroizing::new(vec![0u8; CHUNK]);
    while remaining > 0 {
        let size = remaining.min(CHUNK as u64) as usize;
        input.read_exact(&mut buffer[..size])?;
        let context = EnvelopeContext {
            team_id: 0,
            record_id: position,
            public_key: &manifest.archive_id,
            purpose: "keycast-backup-v3-chunk",
        };
        write_frame(output, &cipher.seal(&buffer[..size], &context)?)?;
        remaining -= size as u64;
        position += 1;
    }
    if input.read(&mut [0u8; 1])? != 0 {
        return Err("snapshot length changed".into());
    }
    Ok(())
}
pub(crate) fn manifest(cipher: &EnvelopeCipher, input: &mut impl Read) -> Result<Manifest> {
    let mut magic = [0u8; 4];
    input.read_exact(&mut magic)?;
    if &magic != b"KCB3" {
        return Err("unsupported backup format".into());
    }
    let bytes = cipher.open(&read_frame(input, 4096)?, &MANIFEST)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    if manifest.length == 0
        || manifest.length > MAX_DATABASE
        || manifest.root.len() > 64
        || nostr::prelude::PublicKey::from_hex(&manifest.archive_id).is_err()
    {
        return Err("invalid backup manifest".into());
    }
    Ok(manifest)
}
pub(crate) fn open(
    cipher: &EnvelopeCipher,
    manifest: &Manifest,
    input: &mut impl Read,
    output: &mut impl Write,
) -> Result<()> {
    let mut remaining = manifest.length;
    let mut position = 1;
    while remaining > 0 {
        let context = EnvelopeContext {
            team_id: 0,
            record_id: position,
            public_key: &manifest.archive_id,
            purpose: "keycast-backup-v3-chunk",
        };
        let bytes = cipher.open(&read_frame(input, CHUNK + 256)?, &context)?;
        let size = remaining.min(CHUNK as u64) as usize;
        if bytes.len() != size {
            return Err("invalid backup chunk length".into());
        }
        output.write_all(&bytes)?;
        remaining -= size as u64;
        position += 1;
    }
    if input.read(&mut [0u8; 1])? != 0 {
        return Err("unexpected data after backup".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (EnvelopeCipher, Vec<u8>, Vec<u8>) {
        let cipher = EnvelopeCipher::from_key(Zeroizing::new([42; 32]));
        let input = vec![17u8; CHUNK * 2 + 45];
        let mut archive = Vec::new();
        seal(
            &cipher,
            &"a".repeat(64),
            input.len() as u64,
            &mut input.as_slice(),
            &mut archive,
        )
        .unwrap();
        (cipher, input, archive)
    }
    fn restore(cipher: &EnvelopeCipher, archive: &[u8]) -> Result<Vec<u8>> {
        let mut input = archive;
        let manifest = manifest(cipher, &mut input)?;
        let mut output = Vec::new();
        open(cipher, &manifest, &mut input, &mut output)?;
        Ok(output)
    }
    #[test]
    fn multiple_chunks_roundtrip_and_tampering_never_completes() {
        let (cipher, plain, archive) = fixture();
        assert_eq!(restore(&cipher, &archive).unwrap(), plain);
        for cut in [0, 4, 8, 100, archive.len() - 1] {
            assert!(restore(&cipher, &archive[..cut]).is_err());
        }
        let mut changed = archive.clone();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        assert!(restore(&cipher, &changed).is_err());
        let mut changed = archive.clone();
        changed.extend_from_slice(b"trailing");
        assert!(restore(&cipher, &changed).is_err());
        let wrong = EnvelopeCipher::from_key(Zeroizing::new([43; 32]));
        assert!(restore(&wrong, &archive).is_err());
    }
    #[test]
    fn chunks_cannot_be_reordered_or_spliced_between_archives() {
        let (cipher, _, archive) = fixture();
        let (_, _, second) = fixture();
        let first_chunk = 8 + u32::from_be_bytes(archive[4..8].try_into().unwrap()) as usize;
        let frame =
            4 + u32::from_be_bytes(archive[first_chunk..first_chunk + 4].try_into().unwrap())
                as usize;
        let mut reordered = archive.clone();
        reordered[first_chunk..first_chunk + frame]
            .copy_from_slice(&archive[first_chunk + frame..first_chunk + 2 * frame]);
        assert!(restore(&cipher, &reordered).is_err());
        let other_start = 8 + u32::from_be_bytes(second[4..8].try_into().unwrap()) as usize;
        let mut spliced = archive.clone();
        spliced[first_chunk..first_chunk + frame]
            .copy_from_slice(&second[other_start..other_start + frame]);
        assert!(restore(&cipher, &spliced).is_err());
    }
}
