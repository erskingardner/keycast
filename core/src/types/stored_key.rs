use crate::encryption::KeyManagerError;
use crate::types::user::UserError;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use sqlx::{from_row::FromRow, row::Row};
use sqlx_sqlite::SqliteRow;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KeyError {
    #[error("Nostr Key Error: {0}")]
    NostrKey(#[from] nostr_sdk::key::Error),

    #[error("NIP-49 Error: {0}")]
    Nip49(#[from] nostr_sdk::nips::nip49::Error),

    #[error("NIP-19 Error: {0}")]
    Nip19(#[from] nostr_sdk::nips::nip19::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("User is not an admin of the team")]
    NotAdmin(#[from] UserError),

    #[error("Key manager error: {0}")]
    KeyManager(#[from] KeyManagerError),
}

/// A stored key is a key that has been stored in the database for a team
#[derive(Debug, Serialize, Deserialize)]
pub struct StoredKey {
    /// The id of the stored key
    pub id: u32,
    /// The id of the team the key belongs to
    pub team_id: u32,
    /// The name of the key
    pub name: String,
    /// The public key of the key, in hex format
    pub public_key: String,
    /// The secret key of the key, in bytes, encrypted
    pub secret_key: Vec<u8>,
    /// The date and time the key was created
    pub created_at: DateTime<chrono::Utc>,
    /// The date and time the key was last updated
    pub updated_at: DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, SqliteRow> for StoredKey {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            team_id: row.try_get("team_id")?,
            name: row.try_get("name")?,
            public_key: row.try_get("public_key")?,
            secret_key: row.try_get("secret_key")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// A public representation of a stored key, without the secret key
#[derive(Debug, Serialize, Deserialize)]
pub struct PublicStoredKey {
    pub id: u32,
    pub team_id: u32,
    pub name: String,
    pub public_key: String,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
}

impl From<StoredKey> for PublicStoredKey {
    fn from(key: StoredKey) -> Self {
        Self {
            id: key.id,
            team_id: key.team_id,
            name: key.name,
            public_key: key.public_key,
            created_at: key.created_at,
            updated_at: key.updated_at,
        }
    }
}
