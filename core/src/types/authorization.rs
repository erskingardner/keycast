use crate::encryption::KeyManagerError;
use crate::traits::AuthorizationValidations;
use crate::traits::CustomPermission;
use crate::types::permission::{Permission, PermissionError};
use crate::types::policy::Policy;
use crate::types::stored_key::StoredKey;
use chrono::DateTime;
use nostr::nips::nip46::NostrConnectRequest;
use nostr_sdk::PublicKey;
use serde::{Deserialize, Serialize};
use sqlx::{from_row::FromRow, row::Row};
use sqlx_sqlite::{SqlitePool, SqliteRow};
use thiserror::Error;
use urlencoding;

#[derive(Error, Debug)]
pub enum AuthorizationError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Encryption error: {0}")]
    Encryption(#[from] KeyManagerError),
    #[error("Invalid bunker secret key")]
    InvalidBunkerSecretKey,
    #[error("Authorization is expired")]
    Expired,
    #[error("Authorization is fully redeemed")]
    FullyRedeemed,
    #[error("Invalid secret")]
    InvalidSecret,
    #[error("Unauthorized by permission")]
    Unauthorized,
    #[error("Unsupported request")]
    UnsupportedRequest,
    #[error("Permission error: {0}")]
    Permission(#[from] PermissionError),
}

/// A list of relays, this is used to store the relays that signers will listen on for an authorization
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Relays(Vec<String>);

impl IntoIterator for Relays {
    type Item = String;
    type IntoIter = std::vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Relays {
    type Item = &'a String;
    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl TryFrom<String> for Relays {
    type Error = serde_json::Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(Relays(serde_json::from_str(&s)?))
    }
}

/// An authorization is a set of permissions that belong to a team and can be used to control access to a team's stored keys
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Authorization {
    /// The id of the authorization
    pub id: u32,
    /// The id of the stored key the authorization belongs to
    pub stored_key_id: u32,
    /// A human-readable label for remembering what the authorization is for
    pub name: Option<String>,
    /// The generated secret connection uuid
    pub secret: String,
    /// The public key of the bunker nostr secret key
    pub bunker_public_key: String,
    /// The encrypted bunker nostr secret key
    pub bunker_secret: Vec<u8>,
    /// The list of relays the authorization will listen on
    pub relays: Relays,
    /// The id of the policy the authorization belongs to
    pub policy_id: u32,
    /// The maximum number of uses for this authorization, None means unlimited
    pub max_uses: Option<u16>,
    /// The date and time at which this authorization expires, None means it never expires
    pub expires_at: Option<DateTime<chrono::Utc>>,
    /// The date and time the authorization was created
    pub created_at: DateTime<chrono::Utc>,
    /// The date and time the authorization was last updated
    pub updated_at: DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, SqliteRow> for Authorization {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        let relays_json: String = row.try_get("relays")?;
        let relays = Relays::try_from(relays_json).map_err(|e| sqlx::Error::ColumnDecode {
            index: "relays".into(),
            source: Box::new(e),
        })?;

        Ok(Self {
            id: row.try_get("id")?,
            stored_key_id: row.try_get("stored_key_id")?,
            name: row.try_get("name")?,
            secret: row.try_get("secret")?,
            bunker_public_key: row.try_get("bunker_public_key")?,
            bunker_secret: row.try_get("bunker_secret")?,
            relays,
            policy_id: row.try_get("policy_id")?,
            max_uses: row.try_get("max_uses")?,
            expires_at: row.try_get("expires_at")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthorizationWithRelations {
    pub authorization: Authorization,
    pub policy: Policy,
    pub users: Vec<UserAuthorization>,
    pub bunker_connection_string: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserAuthorization {
    pub user_public_key: String,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, SqliteRow> for UserAuthorization {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            user_public_key: row.try_get("user_public_key")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl Authorization {
    /// Get the number of redemptions used for this authorization
    /// This method is synchronous/blocking so that we can use it in the signing daemon
    pub fn redemptions_count_sync(&self, pool: &SqlitePool) -> Result<u16, AuthorizationError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let count = sqlx::query_scalar::query_scalar::<_, i64>(
                    r#"
                    SELECT COUNT(*) FROM user_authorizations WHERE authorization_id = ?
                    "#,
                )
                .bind(self.id)
                .fetch_one(pool)
                .await?;
                Ok(count as u16)
            })
        })
    }

    pub fn redemptions_pubkeys_sync(
        &self,
        pool: &SqlitePool,
    ) -> Result<Vec<PublicKey>, AuthorizationError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let pubkeys = sqlx::query_scalar::query_scalar::<_, String>(
                    r#"
                    SELECT user_public_key FROM user_authorizations WHERE authorization_id = ?
                    "#,
                )
                .bind(self.id)
                .fetch_all(pool)
                .await?;
                Ok(pubkeys
                    .iter()
                    .filter_map(|p| PublicKey::from_hex(p).ok())
                    .collect())
            })
        })
    }

    pub fn create_redemption_sync(
        &self,
        pool: &SqlitePool,
        pubkey: &PublicKey,
    ) -> Result<(), AuthorizationError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                // Check if the user exists
                let user = sqlx::query_scalar::query_scalar::<_, String>(
                    r#"
                    SELECT public_key FROM users WHERE public_key = ?
                    "#,
                )
                .bind(pubkey.to_hex())
                .fetch_optional(pool)
                .await?;

                // Create the user if needed
                if user.is_none() {
                    tracing::info!(target: "keycast_signer::signer_daemon", "Creating new user for pubkey: {:?}", pubkey);
                    sqlx::query::query(
                        r#"
                        INSERT INTO users (public_key, created_at, updated_at)
                        VALUES (?, ?, ?)
                        "#,
                    )
                    .bind(pubkey.to_hex())
                    .bind(chrono::Utc::now())
                    .bind(chrono::Utc::now())
                    .execute(pool)
                    .await?;
                }

                // Create the user authorization
                sqlx::query::query(
                    r#"
                    INSERT INTO user_authorizations (authorization_id, user_public_key, created_at, updated_at)
                    VALUES (?, ?, ?, ?)
                    "#,
                )
                .bind(self.id)
                .bind(pubkey.to_hex())
                .bind(chrono::Utc::now())
                .bind(chrono::Utc::now())
                .execute(pool)
                .await?;
                Ok(())
            })
        })
    }

    pub async fn find(pool: &SqlitePool, id: u32) -> Result<Self, AuthorizationError> {
        let authorization = sqlx::query_as::query_as::<_, Authorization>(
            r#"
            SELECT * FROM authorizations WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_one(pool)
        .await?;
        Ok(authorization)
    }

    pub async fn all_ids(pool: &SqlitePool) -> Result<Vec<u32>, AuthorizationError> {
        let authorizations = sqlx::query_scalar::query_scalar::<_, u32>(
            r#"
            SELECT id FROM authorizations
            "#,
        )
        .fetch_all(pool)
        .await?;
        Ok(authorizations)
    }

    /// Get the stored key for this authorization
    pub async fn stored_key(&self, pool: &SqlitePool) -> Result<StoredKey, AuthorizationError> {
        let stored_key = sqlx::query_as::query_as::<_, StoredKey>(
            r#"
            SELECT * FROM stored_keys WHERE id = ?
            "#,
        )
        .bind(self.stored_key_id)
        .fetch_one(pool)
        .await?;
        Ok(stored_key)
    }

    /// Get the permissions for this authorization
    /// This method is synchronous/blocking so that we can use it in the signing daemon
    pub fn permissions_sync(
        &self,
        pool: &SqlitePool,
    ) -> Result<Vec<Permission>, AuthorizationError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let permissions = sqlx::query_as::query_as::<_, Permission>(
                    r#"
                    SELECT p.* 
                    FROM permissions p
                    JOIN policy_permissions pp ON pp.permission_id = p.id
                    JOIN policies pol ON pol.id = pp.policy_id
                    WHERE pol.id = ?
                    "#,
                )
                .bind(self.policy_id)
                .fetch_all(pool)
                .await?;
                Ok(permissions)
            })
        })
    }

    /// Generate a connection string for the authorization
    /// bunker://<remote-signer-pubkey>?relay=<encoded-relay-1,encoded-relay-2>&secret=<encoded-secret>
    pub async fn bunker_connection_string(&self) -> Result<String, AuthorizationError> {
        let relay_params = self
            .relays
            .0
            .iter()
            .map(|r| format!("relay={}", urlencoding::encode(r)))
            .collect::<Vec<_>>()
            .join("&");

        Ok(format!(
            "bunker://{}?{}&secret={}",
            self.bunker_public_key,
            relay_params,
            urlencoding::encode(&self.secret),
        ))
    }

    fn expired(&self) -> Result<bool, AuthorizationError> {
        match self.expires_at {
            Some(expires_at) => Ok(expires_at < chrono::Utc::now()),
            None => Ok(false),
        }
    }

    fn fully_redeemed(&self, pool: &SqlitePool) -> Result<bool, AuthorizationError> {
        match self.max_uses {
            Some(max_uses) => {
                let redemptions = match self.redemptions_count_sync(pool) {
                    Ok(redemptions) => redemptions,
                    Err(e) => {
                        return Err(e);
                    }
                };
                Ok(redemptions >= max_uses)
            }
            None => Ok(false),
        }
    }
}

impl AuthorizationValidations for Authorization {
    fn validate_policy(
        &self,
        pool: &SqlitePool,
        pubkey: &PublicKey,
        request: &NostrConnectRequest,
    ) -> Result<bool, AuthorizationError> {
        // Before anything, check if the authorization is expired
        if self.expired()? {
            return Err(AuthorizationError::Expired);
        }

        // Approve straight away if it's just a ping request, for now?
        if *request == NostrConnectRequest::Ping {
            return Ok(true);
        }

        // Convert database permissions to custom permissions.
        let permissions = self.permissions_sync(pool)?;
        let custom_permissions: Result<Vec<Box<dyn CustomPermission>>, _> = permissions
            .iter()
            .map(|p| p.to_custom_permission())
            .collect();
        let custom_permissions = custom_permissions?;

        match request {
            NostrConnectRequest::Connect {
                remote_signer_public_key,
                secret,
            } => {
                tracing::info!(target: "keycast_signer::signer_daemon", "Connect request received");
                // Check the public key is the same as the bunker public key
                if remote_signer_public_key.to_hex() != self.bunker_public_key {
                    return Err(AuthorizationError::Unauthorized);
                }
                // Check that secret is correct
                match secret {
                    Some(ref s) if s != &self.secret => {
                        return Err(AuthorizationError::InvalidSecret)
                    }
                    _ => {}
                }

                let redeemed_pubkeys = self.redemptions_pubkeys_sync(pool)?;
                if redeemed_pubkeys.contains(pubkey) {
                    return Ok(true);
                }

                // Check if the authorization is fully redeemed before adding a new pubkey.
                if self.fully_redeemed(pool)? {
                    return Err(AuthorizationError::FullyRedeemed);
                }

                tracing::info!(target: "keycast_signer::signer_daemon", "Creating new user authorization for pubkey: {:?}", pubkey);
                self.create_redemption_sync(pool, pubkey)?;
                Ok(true)
            }
            NostrConnectRequest::GetPublicKey => {
                tracing::info!(target: "keycast_signer::signer_daemon", "Get public key request received");
                // Double check that the pubkey has connected to/redeemed this authorization
                Ok(self.redemptions_pubkeys_sync(pool)?.contains(pubkey))
            }
            NostrConnectRequest::SignEvent(event) => {
                tracing::info!(target: "keycast_signer::signer_daemon", "Sign event request received");
                if custom_permissions.is_empty() {
                    return Err(AuthorizationError::Unauthorized);
                }
                for permission in custom_permissions {
                    if !permission.can_sign(event) {
                        return Err(AuthorizationError::Unauthorized);
                    }
                }
                Ok(true)
            }
            NostrConnectRequest::Nip04Encrypt { public_key, text }
            | NostrConnectRequest::Nip44Encrypt { public_key, text } => {
                tracing::info!(target: "keycast_signer::signer_daemon", "NIP04 encrypt request received");
                if custom_permissions.is_empty() {
                    return Err(AuthorizationError::Unauthorized);
                }
                for permission in custom_permissions {
                    if !permission.can_encrypt(text, pubkey, public_key) {
                        return Err(AuthorizationError::Unauthorized);
                    }
                }
                Ok(true)
            }
            NostrConnectRequest::Nip04Decrypt {
                public_key,
                ciphertext,
            }
            | NostrConnectRequest::Nip44Decrypt {
                public_key,
                ciphertext,
            } => {
                tracing::info!(target: "keycast_signer::signer_daemon", "NIP04 decrypt request received");
                if custom_permissions.is_empty() {
                    return Err(AuthorizationError::Unauthorized);
                }
                for permission in custom_permissions {
                    if !permission.can_decrypt(ciphertext, public_key, pubkey) {
                        return Err(AuthorizationError::Unauthorized);
                    }
                }
                Ok(true)
            }
            // We check this earlier but to complete the match statement, we need to return true here
            NostrConnectRequest::Ping => {
                tracing::info!(target: "keycast_signer::signer_daemon", "Ping request received");
                Ok(true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use nostr::nips::nip46::NostrConnectRequest;
    use nostr_sdk::{Keys, PublicKey};
    use sqlx::raw_sql::raw_sql;
    use sqlx_sqlite::SqlitePoolOptions;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query::query("PRAGMA foreign_keys=ON")
            .execute(&pool)
            .await
            .unwrap();
        raw_sql(include_str!(
            "../../../database/migrations/0001_initial.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        raw_sql(include_str!(
            "../../../database/migrations/0003_add_authorization_name.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    async fn create_test_authorization(
        pool: &SqlitePool,
        max_uses: Option<u16>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Authorization {
        let team_id: i64 = sqlx::query_scalar::query_scalar(
            "INSERT INTO teams (name, created_at, updated_at)
             VALUES ('test team', datetime('now'), datetime('now'))
             RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();

        let policy_id: i64 = sqlx::query_scalar::query_scalar(
            "INSERT INTO policies (name, team_id, created_at, updated_at)
             VALUES ('test policy', ?1, datetime('now'), datetime('now'))
             RETURNING id",
        )
        .bind(team_id)
        .fetch_one(pool)
        .await
        .unwrap();

        let stored_key_id: i64 = sqlx::query_scalar::query_scalar(
            "INSERT INTO stored_keys (name, team_id, public_key, secret_key, created_at, updated_at)
             VALUES ('stored key', ?1, ?2, ?3, datetime('now'), datetime('now'))
             RETURNING id",
        )
        .bind(team_id)
        .bind(Keys::generate().public_key().to_hex())
        .bind(vec![1_u8, 2, 3])
        .fetch_one(pool)
        .await
        .unwrap();

        let keys = Keys::generate();
        sqlx::query_as::query_as::<_, Authorization>(
            r#"
            INSERT INTO authorizations
            (stored_key_id, name, secret, bunker_public_key, bunker_secret, relays, policy_id, max_uses, expires_at, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'), datetime('now'))
            RETURNING *
            "#,
        )
        .bind(stored_key_id)
        .bind(Option::<String>::None)
        .bind(format!("test_secret_{}", uuid::Uuid::new_v4()))
        .bind(keys.public_key().to_hex())
        .bind(keys.secret_key().to_secret_bytes().to_vec())
        .bind(serde_json::to_string(&vec!["wss://test.relay"]).unwrap())
        .bind(policy_id)
        .bind(max_uses)
        .bind(expires_at)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn add_permission(
        pool: &SqlitePool,
        policy_id: u32,
        identifier: &str,
        config: serde_json::Value,
    ) -> Permission {
        let permission = sqlx::query_as::query_as::<_, Permission>(
            r#"
            INSERT INTO permissions (identifier, config, created_at, updated_at)
            VALUES (?1, ?2, datetime('now'), datetime('now'))
            RETURNING *
            "#,
        )
        .bind(identifier)
        .bind(config)
        .fetch_one(pool)
        .await
        .unwrap();

        sqlx::query::query(
            r#"
            INSERT INTO policy_permissions (policy_id, permission_id, created_at, updated_at)
            VALUES (?1, ?2, datetime('now'), datetime('now'))
            "#,
        )
        .bind(policy_id)
        .bind(permission.id)
        .execute(pool)
        .await
        .unwrap();

        permission
    }

    async fn redemption_count(pool: &SqlitePool, authorization_id: u32) -> i64 {
        sqlx::query_scalar::query_scalar(
            "SELECT COUNT(*) FROM user_authorizations WHERE authorization_id = ?1",
        )
        .bind(authorization_id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_expired() {
        let pool = setup_test_db().await;

        let future_date = Utc::now() + Duration::hours(24);
        let future_auth = create_test_authorization(&pool, None, Some(future_date)).await;
        assert!(!future_auth.expired().unwrap());

        let past_date = Utc::now() - Duration::hours(24);
        let expired_auth = create_test_authorization(&pool, None, Some(past_date)).await;
        assert!(expired_auth.expired().unwrap());

        let never_expiring_auth = create_test_authorization(&pool, None, None).await;
        assert!(!never_expiring_auth.expired().unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_fully_redeemed() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, Some(2), None).await;
        let first_user = Keys::generate().public_key();
        let second_user = Keys::generate().public_key();

        assert!(!auth.fully_redeemed(&pool).unwrap());

        auth.create_redemption_sync(&pool, &first_user).unwrap();
        assert!(!auth.fully_redeemed(&pool).unwrap());

        auth.create_redemption_sync(&pool, &second_user).unwrap();
        assert!(auth.fully_redeemed(&pool).unwrap());

        let unlimited_auth = create_test_authorization(&pool, None, None).await;
        assert!(!unlimited_auth.fully_redeemed(&pool).unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_policy() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, None, None).await;
        let keys = Keys::generate();
        let pubkey = keys.public_key();
        let request = NostrConnectRequest::Connect {
            remote_signer_public_key: PublicKey::from_hex(&auth.bunker_public_key).unwrap(),
            secret: Some(auth.secret.clone()),
        };

        assert!(auth.validate_policy(&pool, &pubkey, &request).unwrap());
        assert_eq!(redemption_count(&pool, auth.id).await, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn connect_requires_matching_bunker_pubkey() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, None, None).await;
        let requester = Keys::generate().public_key();
        let wrong_remote_signer = Keys::generate().public_key();
        let request = NostrConnectRequest::Connect {
            remote_signer_public_key: wrong_remote_signer,
            secret: Some(auth.secret.clone()),
        };

        assert!(matches!(
            auth.validate_policy(&pool, &requester, &request),
            Err(AuthorizationError::Unauthorized)
        ));
        assert_eq!(redemption_count(&pool, auth.id).await, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn connect_with_wrong_secret_does_not_redeem() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, None, None).await;
        let requester = Keys::generate().public_key();
        let request = NostrConnectRequest::Connect {
            remote_signer_public_key: PublicKey::from_hex(&auth.bunker_public_key).unwrap(),
            secret: Some("wrong".to_string()),
        };

        assert!(matches!(
            auth.validate_policy(&pool, &requester, &request),
            Err(AuthorizationError::InvalidSecret)
        ));
        assert_eq!(redemption_count(&pool, auth.id).await, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_public_key_requires_prior_connect() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, None, None).await;
        let requester = Keys::generate().public_key();

        assert!(!auth
            .validate_policy(&pool, &requester, &NostrConnectRequest::GetPublicKey)
            .unwrap());

        let connect = NostrConnectRequest::Connect {
            remote_signer_public_key: PublicKey::from_hex(&auth.bunker_public_key).unwrap(),
            secret: Some(auth.secret.clone()),
        };
        assert!(auth.validate_policy(&pool, &requester, &connect).unwrap());
        assert!(auth
            .validate_policy(&pool, &requester, &NostrConnectRequest::GetPublicKey)
            .unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn connect_is_idempotent_for_same_pubkey_and_max_uses_blocks_new_pubkeys() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, Some(1), None).await;
        let requester = Keys::generate().public_key();
        let second_requester = Keys::generate().public_key();
        let connect = NostrConnectRequest::Connect {
            remote_signer_public_key: PublicKey::from_hex(&auth.bunker_public_key).unwrap(),
            secret: Some(auth.secret.clone()),
        };

        assert!(auth.validate_policy(&pool, &requester, &connect).unwrap());
        assert!(auth.validate_policy(&pool, &requester, &connect).unwrap());
        assert_eq!(redemption_count(&pool, auth.id).await, 1);

        assert!(matches!(
            auth.validate_policy(&pool, &second_requester, &connect),
            Err(AuthorizationError::FullyRedeemed)
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_empty_policy_denies_signing() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, None, None).await;
        let keys = Keys::generate();
        let request = NostrConnectRequest::SignEvent(
            nostr::EventBuilder::new(nostr::Kind::TextNote, "hello").build(keys.public_key()),
        );

        assert!(matches!(
            auth.validate_policy(&pool, &keys.public_key(), &request),
            Err(AuthorizationError::Unauthorized)
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_invalid_permission_config_denies_without_panic() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, None, None).await;
        add_permission(
            &pool,
            auth.policy_id,
            "allowed_kinds",
            serde_json::json!({"sign": [1], "encrypt": null, "decrypt": null}),
        )
        .await;
        let keys = Keys::generate();
        let request = NostrConnectRequest::SignEvent(
            nostr::EventBuilder::new(nostr::Kind::TextNote, "hello").build(keys.public_key()),
        );

        assert!(matches!(
            auth.validate_policy(&pool, &keys.public_key(), &request),
            Err(AuthorizationError::Permission(_))
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_allowed_kinds_denies_disallowed_event_kind() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, None, None).await;
        add_permission(
            &pool,
            auth.policy_id,
            "allowed_kinds",
            serde_json::json!({"allowed_kinds": [7]}),
        )
        .await;
        let keys = Keys::generate();
        let request = NostrConnectRequest::SignEvent(
            nostr::EventBuilder::new(nostr::Kind::TextNote, "hello").build(keys.public_key()),
        );

        assert!(matches!(
            auth.validate_policy(&pool, &keys.public_key(), &request),
            Err(AuthorizationError::Unauthorized)
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn allowed_kinds_allows_configured_event_kind() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, None, None).await;
        add_permission(
            &pool,
            auth.policy_id,
            "allowed_kinds",
            serde_json::json!({"allowed_kinds": [1]}),
        )
        .await;
        let keys = Keys::generate();
        let request = NostrConnectRequest::SignEvent(
            nostr::EventBuilder::new(nostr::Kind::TextNote, "hello").build(keys.public_key()),
        );

        assert!(auth
            .validate_policy(&pool, &keys.public_key(), &request)
            .unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn content_filter_denies_blocked_signing_and_encryption_content() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, None, None).await;
        add_permission(
            &pool,
            auth.policy_id,
            "content_filter",
            serde_json::json!({"blocked_words": ["secret"]}),
        )
        .await;
        let keys = Keys::generate();
        let sign_request = NostrConnectRequest::SignEvent(
            nostr::EventBuilder::new(nostr::Kind::TextNote, "a secret").build(keys.public_key()),
        );
        let encrypt_request = NostrConnectRequest::Nip04Encrypt {
            public_key: keys.public_key(),
            text: "a secret".to_string(),
        };

        assert!(matches!(
            auth.validate_policy(&pool, &keys.public_key(), &sign_request),
            Err(AuthorizationError::Unauthorized)
        ));
        assert!(matches!(
            auth.validate_policy(&pool, &keys.public_key(), &encrypt_request),
            Err(AuthorizationError::Unauthorized)
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn encrypt_to_self_denies_encryption_to_other_pubkeys() {
        let pool = setup_test_db().await;
        let auth = create_test_authorization(&pool, None, None).await;
        add_permission(
            &pool,
            auth.policy_id,
            "encrypt_to_self",
            serde_json::json!({}),
        )
        .await;
        let sender = Keys::generate().public_key();
        let other = Keys::generate().public_key();

        let denied = NostrConnectRequest::Nip04Encrypt {
            public_key: other,
            text: "hello".to_string(),
        };
        assert!(matches!(
            auth.validate_policy(&pool, &sender, &denied),
            Err(AuthorizationError::Unauthorized)
        ));

        let allowed = NostrConnectRequest::Nip04Encrypt {
            public_key: sender,
            text: "hello".to_string(),
        };
        assert!(auth.validate_policy(&pool, &sender, &allowed).unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bunker_connection_string_url_encodes_relays_and_secret() {
        let pool = setup_test_db().await;
        let mut auth = create_test_authorization(&pool, None, None).await;
        auth.secret = "secret with spaces & symbols".to_string();
        auth.relays = Relays(vec!["wss://relay.example/path?x=1&y=2".to_string()]);

        let connection_string = auth.bunker_connection_string().await.unwrap();

        assert!(
            connection_string.contains("relay=wss%3A%2F%2Frelay.example%2Fpath%3Fx%3D1%26y%3D2")
        );
        assert!(connection_string.contains("secret=secret%20with%20spaces%20%26%20symbols"));
    }
}
