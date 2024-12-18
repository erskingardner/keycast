use crate::permissions::traits::CustomPermission;
use nostr_sdk::{Event, PublicKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EncryptToSelfConfig {}

impl From<EncryptToSelfConfig> for serde_json::Value {
    fn from(config: EncryptToSelfConfig) -> Self {
        serde_json::to_value(config).unwrap()
    }
}

pub struct EncryptToSelf {
    config: EncryptToSelfConfig,
}

impl CustomPermission for EncryptToSelf {
    fn identifier(&self) -> &'static str {
        "encrypt_to_self"
    }

    fn config(&self) -> serde_json::Value {
        self.config.clone().into()
    }

    // This permission doesn't care about signing events
    fn can_sign(&self, _event: &Event) -> bool {
        true
    }

    fn can_encrypt(&self, event: &Event, recipient_pubkey: &PublicKey) -> bool {
        event.pubkey == *recipient_pubkey
    }

    fn can_decrypt(&self, event: &Event, sender_pubkey: &PublicKey) -> bool {
        event.pubkey == *sender_pubkey
    }
}
