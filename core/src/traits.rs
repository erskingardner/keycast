use crate::types::authorization::AuthorizationError;
use crate::types::permission::{Permission, PermissionError};
use async_trait::async_trait;
use nostr::nips::nip46::NostrConnectRequest;
use nostr_sdk::{PublicKey, UnsignedEvent};
use sqlx_sqlite::SqlitePool;

/// Provides methods for validating an authorization against it's permissions and other properties in the context of a request
pub trait AuthorizationValidations {
    /// Check the authorization's policy & permissions
    fn validate_policy(
        &self,
        pool: &SqlitePool,
        pubkey: &PublicKey,
        request: &NostrConnectRequest,
    ) -> Result<bool, AuthorizationError>;
}

/// A trait that represents a custom permission
#[async_trait]
pub trait CustomPermission: Send + Sync {
    /// Create a new instance of the permission from a database Permission
    fn from_permission(
        permission: &Permission,
    ) -> Result<Box<dyn CustomPermission>, PermissionError>
    where
        Self: Sized;

    fn identifier(&self) -> &'static str;

    /// A function that returns true if allowed to sign the event.
    fn can_sign(&self, event: &UnsignedEvent) -> bool;

    /// A function that returns true if allowed to encrypt the content for the recipient.
    /// Sender is the pubkey of the user requesting the encryption
    fn can_encrypt(
        &self,
        plaintext: &str,
        sender_pubkey: &PublicKey,
        recipient_pubkey: &PublicKey,
    ) -> bool;

    /// A function that returns true if allowed to decrypt the content from the sender.
    /// Recipient is the pubkey of the user requesting the decryption
    fn can_decrypt(
        &self,
        ciphertext: &str,
        sender_pubkey: &PublicKey,
        recipient_pubkey: &PublicKey,
    ) -> bool;
}
