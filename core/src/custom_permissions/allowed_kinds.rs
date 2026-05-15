use crate::{
    traits::CustomPermission,
    types::permission::{Permission, PermissionError},
};
use async_trait::async_trait;
use nostr_sdk::{PublicKey, UnsignedEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct AllowedKindsConfig {
    pub allowed_kinds: Option<Vec<u16>>,
}

pub struct AllowedKinds {
    config: AllowedKindsConfig,
}

#[async_trait]
impl CustomPermission for AllowedKinds {
    fn from_permission(
        permission: &Permission,
    ) -> Result<Box<dyn CustomPermission>, PermissionError> {
        let parsed_config: AllowedKindsConfig =
            serde_json::from_value(permission.config.clone())
                .map_err(|e| PermissionError::InvalidConfig(e.to_string()))?;

        Ok(Box::new(Self {
            config: parsed_config,
        }))
    }

    fn identifier(&self) -> &'static str {
        "allowed_kinds"
    }

    fn can_sign(&self, event: &UnsignedEvent) -> bool {
        match &self.config.allowed_kinds {
            None => true,
            Some(kinds) => kinds.contains(&event.kind.into()),
        }
    }

    // We don't get event info from these requests, so we must always allow
    fn can_encrypt(
        &self,
        _plaintext: &str,
        _sender_pubkey: &PublicKey,
        _recipient_pubkey: &PublicKey,
    ) -> bool {
        true
    }
    // We don't get event info from these requests, so we must always allow
    fn can_decrypt(
        &self,
        _ciphertext: &str,
        _sender_pubkey: &PublicKey,
        _recipient_pubkey: &PublicKey,
    ) -> bool {
        true
    }
}

#[test]
fn test_default() {
    let config = AllowedKindsConfig::default();
    assert!(config.allowed_kinds.is_none());
}

#[test]
fn rejects_unknown_config_fields() {
    let invalid = serde_json::json!({
        "sign": [1],
        "encrypt": null,
        "decrypt": null
    });

    assert!(serde_json::from_value::<AllowedKindsConfig>(invalid).is_err());
}

#[test]
fn allows_only_configured_event_kinds() {
    let permission = Permission {
        id: 0,
        identifier: "allowed_kinds".to_string(),
        config: serde_json::json!({"allowed_kinds": [7]}),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let allowed_kinds = AllowedKinds::from_permission(&permission).unwrap();
    let keys = nostr_sdk::Keys::generate();

    let reaction =
        nostr_sdk::EventBuilder::new(nostr_sdk::Kind::Reaction, "+").build(keys.public_key());
    let text_note =
        nostr_sdk::EventBuilder::new(nostr_sdk::Kind::TextNote, "hello").build(keys.public_key());

    assert!(allowed_kinds.can_sign(&reaction));
    assert!(!allowed_kinds.can_sign(&text_note));
}
