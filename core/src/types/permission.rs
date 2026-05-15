use crate::custom_permissions::{
    allowed_kinds::AllowedKinds, content_filter::ContentFilter, encrypt_to_self::EncryptToSelf,
};
use crate::traits::CustomPermission;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{from_row::FromRow, row::Row};
use sqlx_sqlite::SqliteRow;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PermissionError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Unknown permission type: {0}")]
    UnknownPermission(String),
    #[error("Invalid permission configuration: {0}")]
    InvalidConfig(String),
}

/// A permission is database representation of a CustomPermission trait
#[derive(Debug, Serialize, Deserialize)]
pub struct Permission {
    /// The id of the permission
    pub id: u32,
    /// The identifier of the permission
    pub identifier: String,
    /// The configuration of the permission
    pub config: serde_json::Value,
    /// The date and time the permission was created
    pub created_at: DateTime<chrono::Utc>,
    /// The date and time the permission was last updated
    pub updated_at: DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, SqliteRow> for Permission {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            identifier: row.try_get("identifier")?,
            config: row.try_get("config")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl Permission {
    pub fn validate_config(
        identifier: &str,
        config: &serde_json::Value,
    ) -> Result<(), PermissionError> {
        let permission = Permission {
            id: 0,
            identifier: identifier.to_string(),
            config: config.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        permission.to_custom_permission().map(|_| ())
    }

    /// Convert this database permission into a CustomPermission implementation
    pub fn to_custom_permission(&self) -> Result<Box<dyn CustomPermission>, PermissionError> {
        match self.identifier.as_str() {
            "allowed_kinds" => AllowedKinds::from_permission(self),
            "content_filter" => ContentFilter::from_permission(self),
            "encrypt_to_self" => EncryptToSelf::from_permission(self),
            _ => Err(PermissionError::UnknownPermission(self.identifier.clone())),
        }
    }
}

/// A policy permission is a join table between a policy and a permission
#[derive(Debug, Serialize, Deserialize)]
pub struct PolicyPermission {
    /// The id of the policy permission
    pub id: u32,
    /// The id of the policy
    pub policy_id: u32,
    /// The id of the permission
    pub permission_id: u32,
    /// The date and time the policy permission was created
    pub created_at: DateTime<chrono::Utc>,
    /// The date and time the policy permission was last updated
    pub updated_at: DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, SqliteRow> for PolicyPermission {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            policy_id: row.try_get("policy_id")?,
            permission_id: row.try_get("permission_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_config_rejects_unknown_permission_identifiers() {
        assert!(matches!(
            Permission::validate_config("missing", &serde_json::json!({})),
            Err(PermissionError::UnknownPermission(identifier)) if identifier == "missing"
        ));
    }

    #[test]
    fn validate_config_rejects_malformed_allowed_kinds() {
        assert!(matches!(
            Permission::validate_config(
                "allowed_kinds",
                &serde_json::json!({"sign": [1], "encrypt": null, "decrypt": null})
            ),
            Err(PermissionError::InvalidConfig(_))
        ));
    }

    #[test]
    fn validate_config_accepts_known_permission_shapes() {
        Permission::validate_config(
            "allowed_kinds",
            &serde_json::json!({"allowed_kinds": [1, 7]}),
        )
        .unwrap();
        Permission::validate_config(
            "content_filter",
            &serde_json::json!({"blocked_words": ["secret"]}),
        )
        .unwrap();
        Permission::validate_config("encrypt_to_self", &serde_json::json!({})).unwrap();
    }
}
