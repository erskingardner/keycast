use crate::{
    traits::CustomPermission,
    types::permission::{Permission, PermissionError},
};
use async_trait::async_trait;
use nostr_sdk::{PublicKey, UnsignedEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct EncryptToSelfConfig {}

pub struct EncryptToSelf {}

#[async_trait]
impl CustomPermission for EncryptToSelf {
    fn from_permission(
        permission: &Permission,
    ) -> Result<Box<dyn CustomPermission>, PermissionError> {
        let _parsed_config: EncryptToSelfConfig = serde_json::from_value(permission.config.clone())
            .map_err(|e| PermissionError::InvalidConfig(e.to_string()))?;

        Ok(Box::new(Self {}))
    }

    fn identifier(&self) -> &'static str {
        "encrypt_to_self"
    }

    // This permission doesn't care about signing events
    fn can_sign(&self, _event: &UnsignedEvent) -> bool {
        true
    }

    fn can_encrypt(
        &self,
        _plaintext: &str,
        sender_pubkey: &PublicKey,
        recipient_pubkey: &PublicKey,
    ) -> bool {
        *sender_pubkey == *recipient_pubkey
    }

    fn can_decrypt(
        &self,
        _ciphertext: &str,
        sender_pubkey: &PublicKey,
        recipient_pubkey: &PublicKey,
    ) -> bool {
        *sender_pubkey == *recipient_pubkey
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permission(config: serde_json::Value) -> Permission {
        Permission {
            id: 0,
            identifier: "encrypt_to_self".to_string(),
            config,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn rejects_unknown_config_fields() {
        assert!(
            EncryptToSelf::from_permission(&permission(serde_json::json!({
                "ignored": true
            })))
            .is_err()
        );
    }

    #[test]
    fn allows_only_same_sender_and_recipient_for_encryption() {
        let permission =
            EncryptToSelf::from_permission(&permission(serde_json::json!({}))).unwrap();
        let sender = nostr_sdk::Keys::generate().public_key();
        let recipient = nostr_sdk::Keys::generate().public_key();

        assert!(permission.can_encrypt("hello", &sender, &sender));
        assert!(!permission.can_encrypt("hello", &sender, &recipient));
    }
}
