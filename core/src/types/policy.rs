use crate::types::permission::Permission;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use sqlx::{from_row::FromRow, row::Row};
use sqlx_sqlite::SqliteRow;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PolicyError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Policy not found")]
    NotFound,
}

/// A policy is a set of permissions. Teams have many policies, and policies have many permissions.
#[derive(Debug, Serialize, Deserialize)]
pub struct Policy {
    /// The id of the policy
    pub id: u32,
    /// The name of the policy
    pub name: String,
    /// The id of the team the policy belongs to
    pub team_id: u32,
    /// The date and time the policy was created
    pub created_at: DateTime<chrono::Utc>,
    /// The date and time the policy was last updated
    pub updated_at: DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, SqliteRow> for Policy {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            team_id: row.try_get("team_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// A policy with its permissions, this is a join table between a policy and its permissions
#[derive(Debug, Serialize, Deserialize)]
pub struct PolicyWithPermissions {
    pub policy: Policy,
    pub permissions: Vec<Permission>,
}
