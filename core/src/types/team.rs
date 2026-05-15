use crate::encryption::KeyManagerError;
use crate::types::authorization::{AuthorizationError, AuthorizationWithRelations};
use crate::types::permission::{Permission, PermissionError};
use crate::types::policy::{Policy, PolicyError, PolicyWithPermissions};
use crate::types::stored_key::{PublicStoredKey, StoredKey};
use crate::types::user::{TeamUser, UserError};
use chrono::DateTime;
use nostr_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::{from_row::FromRow, row::Row};
use sqlx_sqlite::SqlitePool;
use sqlx_sqlite::SqliteRow;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TeamError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("User is not authorized to perform this action")]
    NotAuthorized,

    #[error("User is not an admin of the team")]
    NotAdmin(#[from] UserError),

    #[error("User is already a member of the team")]
    UserAlreadyMember,

    #[error("Encryption error: {0}")]
    Encryption(#[from] KeyManagerError),

    #[error("Policy error: {0}")]
    Policy(#[from] PolicyError),

    #[error("Permission error: {0}")]
    Permission(#[from] PermissionError),

    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Authorization error: {0}")]
    Authorization(#[from] AuthorizationError),
}

/// A team is a collection of users, stored keys, policies, and permissions
#[derive(Debug, Serialize, Deserialize)]
pub struct Team {
    /// The id of the team
    pub id: u32,
    /// The name of the team
    pub name: String,
    /// The date and time the team was created
    pub created_at: DateTime<chrono::Utc>,
    /// The date and time the team was last updated
    pub updated_at: DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, SqliteRow> for Team {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamWithRelations {
    pub team: Team,
    pub team_users: Vec<TeamUser>, // Use team_user here so we get the role
    pub stored_keys: Vec<PublicStoredKey>,
    pub policies: Vec<PolicyWithPermissions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyWithRelations {
    pub team: Team,
    pub stored_key: PublicStoredKey,
    pub authorizations: Vec<AuthorizationWithRelations>,
}

impl Team {
    pub async fn find_with_relations(
        pool: &SqlitePool,
        team_id: u32,
    ) -> Result<TeamWithRelations, TeamError> {
        // Get team
        let team = sqlx::query_as::query_as::<_, Team>("SELECT * FROM teams WHERE id = ?1")
            .bind(team_id)
            .fetch_one(pool)
            .await?;

        // Get team_users for this team
        let team_users = sqlx::query_as::query_as::<_, TeamUser>(
            r#"
            SELECT tu.* 
            FROM team_users tu
            WHERE tu.team_id = ?1
            "#,
        )
        .bind(team_id)
        .fetch_all(pool)
        .await?;

        // Get stored keys for this team
        let stored_keys = sqlx::query_as::query_as::<_, StoredKey>(
            "SELECT * FROM stored_keys WHERE team_id = ?1",
        )
        .bind(team_id)
        .fetch_all(pool)
        .await?;

        let public_stored_keys: Vec<PublicStoredKey> = stored_keys
            .into_iter()
            .map(|k| k.into())
            .collect::<Vec<_>>();

        // Get policies for this team
        let policies = Team::get_policies_with_permissions(pool, team_id).await?;

        Ok(TeamWithRelations {
            team,
            team_users,
            stored_keys: public_stored_keys,
            policies,
        })
    }

    pub async fn get_policies_with_permissions(
        pool: &SqlitePool,
        team_id: u32,
    ) -> Result<Vec<PolicyWithPermissions>, TeamError> {
        // First fetch policies
        let policies =
            sqlx::query_as::query_as::<_, Policy>("SELECT * FROM policies WHERE team_id = ?1")
                .bind(team_id)
                .fetch_all(pool)
                .await?;

        // Then fetch permissions for each policy
        let mut policies_with_permissions = Vec::new();
        for policy in policies {
            let permissions = sqlx::query_as::query_as::<_, Permission>(
                "SELECT p.* FROM permissions p 
                 JOIN policy_permissions pp ON pp.permission_id = p.id 
                 WHERE pp.policy_id = ?1",
            )
            .bind(policy.id)
            .fetch_all(pool)
            .await?;

            policies_with_permissions.push(PolicyWithPermissions {
                policy,
                permissions,
            });
        }

        Ok(policies_with_permissions)
    }
}
