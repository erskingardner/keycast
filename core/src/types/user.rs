use crate::types::stored_key::StoredKey;
use crate::types::team::{Team, TeamWithRelations};
use chrono::DateTime;
use nostr_sdk::PublicKey;
use serde::{Deserialize, Serialize};
use sqlx::{from_row::FromRow, row::Row};
use sqlx_sqlite::{SqlitePool, SqliteRow};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UserError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Couldn't fetch relations")]
    Relations,
    #[error("User not found")]
    NotFound,
}

/// A user is a representation of a Nostr user (based solely on a pubkey value)
#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    /// The user's public key, in hex format
    pub public_key: String,
    /// The date and time the user was created
    pub created_at: DateTime<chrono::Utc>,
    /// The date and time the user was last updated
    pub updated_at: DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, SqliteRow> for User {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            public_key: row.try_get("public_key")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// A team user is a representation of a user's membership in a team, this is a join table
#[derive(Debug, Serialize, Deserialize)]
pub struct TeamUser {
    /// The user's public key, in hex format
    pub user_public_key: String,
    /// The team id
    pub team_id: u32,
    /// The user's role in the team
    pub role: TeamUserRole,
    /// The date and time the user was created
    pub created_at: DateTime<chrono::Utc>,
    /// The date and time the user was last updated
    pub updated_at: DateTime<chrono::Utc>,
}

/// The role of a user in a team
#[derive(Debug, Serialize, Deserialize)]
pub enum TeamUserRole {
    Admin,
    Member,
}

impl TeamUserRole {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    fn from_db_str(role: String) -> Result<Self, sqlx::Error> {
        match role.as_str() {
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            _ => Err(sqlx::Error::ColumnDecode {
                index: "role".into(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown team user role: {role}"),
                )
                .into(),
            }),
        }
    }
}

impl<'r> FromRow<'r, SqliteRow> for TeamUser {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            user_public_key: row.try_get("user_public_key")?,
            team_id: row.try_get("team_id")?,
            role: TeamUserRole::from_db_str(row.try_get("role")?)?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl User {
    pub async fn find_by_pubkey(pool: &SqlitePool, pubkey: &PublicKey) -> Result<Self, UserError> {
        match sqlx::query_as::query_as::<_, User>("SELECT * FROM users WHERE public_key = ?1")
            .bind(pubkey.to_hex())
            .fetch_one(pool)
            .await
        {
            Ok(user) => Ok(user),
            Err(sqlx::Error::RowNotFound) => Err(UserError::NotFound),
            Err(e) => {
                println!("Error fetching user: {:?}", e);
                Err(UserError::Database(e))
            }
        }
    }

    pub async fn teams(&self, pool: &SqlitePool) -> Result<Vec<TeamWithRelations>, UserError> {
        let teams = sqlx::query_as::query_as::<_, Team>(
            "SELECT * FROM teams WHERE id IN (SELECT team_id FROM team_users WHERE user_public_key = ?1)",
        )
        .bind(self.public_key.clone())
        .fetch_all(pool)
        .await?;

        let mut teams_with_relations = Vec::new();

        for team in teams {
            // Get team_users for this team
            let team_users = sqlx::query_as::query_as::<_, TeamUser>(
                r#"
                SELECT tu.* 
                FROM team_users tu
                WHERE tu.team_id = ?1
                "#,
            )
            .bind(team.id)
            .fetch_all(pool)
            .await?;

            // Get stored keys for this team
            let stored_keys = sqlx::query_as::query_as::<_, StoredKey>(
                "SELECT * FROM stored_keys WHERE team_id = ?1",
            )
            .bind(team.id)
            .fetch_all(pool)
            .await?;

            // Get policies for this team
            let policies = Team::get_policies_with_permissions(pool, team.id)
                .await
                .map_err(|_| UserError::Relations)?;

            teams_with_relations.push(TeamWithRelations {
                team,
                team_users,
                stored_keys: stored_keys
                    .into_iter()
                    .map(|k| k.into())
                    .collect::<Vec<_>>(),
                policies,
            });
        }

        Ok(teams_with_relations)
    }

    /// Check if a user is an admin of a team
    pub async fn is_team_admin(
        pool: &SqlitePool,
        pubkey: &PublicKey,
        team_id: u32,
    ) -> Result<bool, UserError> {
        let query = "SELECT COUNT(*) FROM team_users WHERE user_public_key = ?1 AND team_id = ?2 AND role = 'admin'";
        let count = sqlx::query_scalar::query_scalar::<_, i64>(query)
            .bind(pubkey.to_hex())
            .bind(team_id)
            .fetch_one(pool)
            .await?;
        Ok(count > 0)
    }

    /// Check if a user is a member of a team
    pub async fn is_team_member(
        pool: &SqlitePool,
        pubkey: &PublicKey,
        team_id: u32,
    ) -> Result<bool, UserError> {
        let query = "SELECT COUNT(*) FROM team_users WHERE user_public_key = ?1 AND team_id = ?2 AND role = 'member'";
        let count = sqlx::query_scalar::query_scalar::<_, i64>(query)
            .bind(pubkey.to_hex())
            .bind(team_id)
            .fetch_one(pool)
            .await?;
        Ok(count > 0)
    }

    /// Check if a user is part of a team (admin or member)
    pub async fn is_team_teammate(
        pool: &SqlitePool,
        pubkey: &PublicKey,
        team_id: u32,
    ) -> Result<bool, UserError> {
        let query = "SELECT COUNT(*) FROM team_users WHERE user_public_key = ?1 AND team_id = ?2";
        let count = sqlx::query_scalar::query_scalar::<_, i64>(query)
            .bind(pubkey.to_hex())
            .bind(team_id)
            .fetch_one(pool)
            .await?;
        Ok(count > 0)
    }
}
