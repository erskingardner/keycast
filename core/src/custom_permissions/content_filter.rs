use crate::{
    traits::CustomPermission,
    types::permission::{Permission, PermissionError},
};
use async_trait::async_trait;
use nostr_sdk::{PublicKey, UnsignedEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct ContentFilterConfig {
    pub blocked_words: Option<Vec<String>>,
}

pub struct ContentFilter {
    config: ContentFilterConfig,
}

#[async_trait]
impl CustomPermission for ContentFilter {
    fn from_permission(
        permission: &Permission,
    ) -> Result<Box<dyn CustomPermission>, PermissionError> {
        let parsed_config: ContentFilterConfig = serde_json::from_value(permission.config.clone())
            .map_err(|e| PermissionError::InvalidConfig(e.to_string()))?;

        Ok(Box::new(Self {
            config: parsed_config,
        }))
    }

    fn identifier(&self) -> &'static str {
        "content_filter"
    }

    fn can_sign(&self, event: &UnsignedEvent) -> bool {
        !self
            .blocked_words()
            .any(|word| event.content.contains(word))
    }

    fn can_encrypt(
        &self,
        plaintext: &str,
        _sender_pubkey: &PublicKey,
        _recipient_pubkey: &PublicKey,
    ) -> bool {
        !self.blocked_words().any(|word| plaintext.contains(word))
    }

    // We can't know what is in the content of the event, so we always allow decryption
    fn can_decrypt(
        &self,
        _ciphertext: &str,
        _sender_pubkey: &PublicKey,
        _recipient_pubkey: &PublicKey,
    ) -> bool {
        true
    }
}

impl ContentFilter {
    fn blocked_words(&self) -> impl Iterator<Item = &str> {
        self.config
            .blocked_words
            .iter()
            .flatten()
            .map(String::as_str)
            .map(str::trim)
            .filter(|word| !word.is_empty())
    }
}

#[test]
fn test_default() {
    let config = ContentFilterConfig::default();
    assert!(config.blocked_words.is_none());
}

#[test]
fn rejects_unknown_config_fields() {
    let invalid = serde_json::json!({
        "blocked_words": ["secret"],
        "ignored": true
    });

    assert!(serde_json::from_value::<ContentFilterConfig>(invalid).is_err());
}

#[test]
fn ignores_empty_blocked_words() {
    let permission = Permission {
        id: 0,
        identifier: "content_filter".to_string(),
        config: serde_json::json!({"blocked_words": ["", "  "]}),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let filter = ContentFilter::from_permission(&permission).unwrap();
    let keys = nostr_sdk::Keys::generate();
    let event =
        nostr_sdk::EventBuilder::new(nostr_sdk::Kind::TextNote, "hello").build(keys.public_key());

    assert!(filter.can_sign(&event));
    assert!(filter.can_encrypt("hello", &keys.public_key(), &keys.public_key()));
}
