#![allow(clippy::type_complexity)]

use std::collections::BTreeSet;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use constant_time_eq::constant_time_eq;
use getrandom::fill;
use keycast_core::v2::control::{GrantSummary, StoredKeySummary};
use keycast_core::v2::envelope::{EnvelopeCipher, EnvelopeContext};
use keycast_core::v2::policy::{PolicyDocument, RequestedCapabilities};
use keycast_core::v2::ENVELOPE_VERSION;
use nostr::prelude::{Keys, PublicKey};
use sha2::{Digest, Sha256};
use sqlx::{query::query, query_as::query_as, query_scalar::query_scalar};
use sqlx_sqlite::SqlitePool;
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Store {
    pub pool: SqlitePool,
    pub cipher: EnvelopeCipher,
    pub authority: std::sync::Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug, Clone)]
pub struct RuntimeGrant {
    pub id: i64,
    pub team_id: i64,
    pub stored_key_id: i64,
    pub remote_signer_public_key: String,
    pub remote_signer_secret_envelope: Vec<u8>,
    pub remote_envelope_version: i64,
    pub remote_key_id: String,
    pub stored_public_key: String,
    pub stored_secret_envelope: Vec<u8>,
    pub stored_envelope_version: i64,
    pub stored_key_id_tag: String,
    pub policy_document: String,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub id: i64,
    pub requested_capabilities: Option<String>,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("cryptographic error: {0}")]
    Envelope(#[from] keycast_core::v2::envelope::EnvelopeError),
    #[error("invalid Nostr key: {0}")]
    Nostr(#[from] nostr::error::Error),
    #[error("invalid policy: {0}")]
    Policy(#[from] keycast_core::v2::policy::PolicyError),
    #[error("random number generation failed")]
    Random,
    #[error("not found")]
    NotFound,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("invitation is not claimable")]
    InvitationNotClaimable,
    #[error("stored public key does not match decrypted secret")]
    PublicKeyMismatch,
    #[error("unsupported encrypted envelope")]
    UnsupportedEnvelope,
}

impl Store {
    pub fn new(pool: SqlitePool, cipher: EnvelopeCipher) -> Self {
        Self {
            pool,
            cipher,
            authority: std::sync::Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub async fn seal_stored_key(
        &self,
        team_id: i64,
        actor_public_key: &str,
        name: String,
        secret_key: Zeroizing<String>,
    ) -> Result<StoredKeySummary, StoreError> {
        validate_name(&name)?;
        let actor_public_key = PublicKey::from_hex(actor_public_key)?.to_hex();
        let keys = Keys::parse(secret_key.as_str())?;
        let public_key = keys.public_key().to_hex();
        let secret = Zeroizing::new(keys.secret_key().secret_bytes());

        let mut transaction = self.pool.begin().await?;
        let id: i64 = query_scalar(
            "INSERT INTO stored_keys(
                team_id, name, public_key, secret_envelope, envelope_version, key_encryption_key_id
             ) VALUES (?, ?, ?, x'00', ?, ?) RETURNING id",
        )
        .bind(team_id)
        .bind(name.trim())
        .bind(&public_key)
        .bind(ENVELOPE_VERSION)
        .bind(self.cipher.key_id())
        .fetch_one(&mut *transaction)
        .await?;

        let envelope = self.cipher.seal(
            secret.as_ref(),
            &EnvelopeContext {
                team_id,
                record_id: id,
                public_key: &public_key,
                purpose: "stored-key",
            },
        )?;
        query("UPDATE stored_keys SET secret_envelope = ? WHERE id = ?")
            .bind(envelope)
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        audit(
            &mut transaction,
            Some(team_id),
            Some(id),
            None,
            None,
            Some(&actor_public_key),
            "stored_key.create",
            "succeeded",
            None,
        )
        .await?;
        transaction.commit().await?;
        self.stored_key_summary(id).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_grant(
        &self,
        team_id: i64,
        actor_public_key: &str,
        stored_key_id: i64,
        policy_id: i64,
        name: String,
        expires_at: Option<i64>,
        invitation_expires_at: i64,
    ) -> Result<(GrantSummary, i64, String), StoreError> {
        validate_name(&name)?;
        let actor_public_key = PublicKey::from_hex(actor_public_key)?.to_hex();
        let now = chrono::Utc::now().timestamp();
        if expires_at.is_some_and(|expiry| expiry <= now) {
            return Err(StoreError::InvalidInput(
                "grant expiration must be in the future".to_string(),
            ));
        }
        if invitation_expires_at <= now
            || expires_at.is_some_and(|grant_expiry| invitation_expires_at > grant_expiry)
        {
            return Err(StoreError::InvalidInput(
                "invitation expiration must be in the grant lifetime".to_string(),
            ));
        }

        let (stored_team_id, _): (i64, String) =
            query_as("SELECT team_id, public_key FROM stored_keys WHERE id = ?")
                .bind(stored_key_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(StoreError::NotFound)?;
        let (policy_team_id, policy_json): (i64, String) =
            query_as("SELECT team_id, document FROM policies WHERE id = ? AND deleted_at IS NULL")
                .bind(policy_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(StoreError::NotFound)?;
        if stored_team_id != team_id || policy_team_id != team_id {
            return Err(StoreError::NotFound);
        }
        PolicyDocument::parse(&policy_json)?;

        let remote_keys = Keys::generate();
        let remote_public_key = remote_keys.public_key().to_hex();
        let remote_secret = Zeroizing::new(remote_keys.secret_key().secret_bytes());
        let (invitation_secret, invitation_hash) = generate_invitation_secret()?;
        let bunker_uri = self
            .bunker_uri(&remote_public_key, &invitation_secret)
            .await?;

        let mut transaction = self.pool.begin().await?;
        let grant_id: i64 = query_scalar(
            "INSERT INTO grants(
                team_id, stored_key_id, policy_id, name, remote_signer_public_key,
                remote_signer_secret_envelope, envelope_version, key_encryption_key_id, expires_at
             ) VALUES (?, ?, ?, ?, ?, x'00', ?, ?, ?) RETURNING id",
        )
        .bind(team_id)
        .bind(stored_key_id)
        .bind(policy_id)
        .bind(name.trim())
        .bind(&remote_public_key)
        .bind(ENVELOPE_VERSION)
        .bind(self.cipher.key_id())
        .bind(expires_at)
        .fetch_one(&mut *transaction)
        .await?;

        let remote_envelope = self.cipher.seal(
            remote_secret.as_ref(),
            &EnvelopeContext {
                team_id,
                record_id: grant_id,
                public_key: &remote_public_key,
                purpose: "remote-signer-key",
            },
        )?;
        query("UPDATE grants SET remote_signer_secret_envelope = ? WHERE id = ?")
            .bind(remote_envelope)
            .bind(grant_id)
            .execute(&mut *transaction)
            .await?;
        let invitation_id: i64 = query_scalar(
            "INSERT INTO invitations(grant_id, secret_hash, expires_at)
             VALUES (?, ?, ?) RETURNING id",
        )
        .bind(grant_id)
        .bind(invitation_hash)
        .bind(invitation_expires_at)
        .fetch_one(&mut *transaction)
        .await?;
        audit(
            &mut transaction,
            Some(team_id),
            Some(stored_key_id),
            Some(grant_id),
            None,
            Some(&actor_public_key),
            "grant.create",
            "succeeded",
            None,
        )
        .await?;
        transaction.commit().await?;

        let grant = self.grant_summary(grant_id).await?;
        Ok((grant, invitation_id, bunker_uri))
    }

    pub async fn create_invitation(
        &self,
        grant_id: i64,
        actor_public_key: &str,
        expires_at: i64,
    ) -> Result<(i64, String), StoreError> {
        let now = chrono::Utc::now().timestamp();
        let actor_public_key = PublicKey::from_hex(actor_public_key)?.to_hex();
        let (remote_public_key, grant_expiry, team_id, stored_key_id):
            (String, Option<i64>, i64, i64) = query_as(
            "SELECT remote_signer_public_key, expires_at, team_id, stored_key_id FROM grants
             WHERE id = ? AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > unixepoch())",
        )
        .bind(grant_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        if expires_at <= now || grant_expiry.is_some_and(|expiry| expires_at > expiry) {
            return Err(StoreError::InvalidInput(
                "invitation expiration must be in the grant lifetime".to_string(),
            ));
        }
        let (secret, hash) = generate_invitation_secret()?;
        let bunker_uri = self.bunker_uri(&remote_public_key, &secret).await?;
        let mut transaction = self.pool.begin().await?;
        let id: i64 = query_scalar(
            "INSERT INTO invitations(grant_id, secret_hash, expires_at)
             VALUES (?, ?, ?) RETURNING id",
        )
        .bind(grant_id)
        .bind(hash)
        .bind(expires_at)
        .fetch_one(&mut *transaction)
        .await?;
        audit(
            &mut transaction,
            Some(team_id),
            Some(stored_key_id),
            Some(grant_id),
            None,
            Some(&actor_public_key),
            "invitation.create",
            "succeeded",
            None,
        )
        .await?;
        transaction.commit().await?;
        Ok((id, bunker_uri))
    }

    pub async fn revoke_grant(
        &self,
        grant_id: i64,
        actor_public_key: &str,
    ) -> Result<(), StoreError> {
        let actor_public_key = PublicKey::from_hex(actor_public_key)?.to_hex();
        let mut transaction = self.pool.begin().await?;
        let revoked: Option<(i64, i64)> = query_as(
            "UPDATE grants SET revoked_at = unixepoch(), updated_at = unixepoch()
             WHERE id = ? AND revoked_at IS NULL RETURNING team_id, stored_key_id",
        )
        .bind(grant_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let (team_id, stored_key_id) = revoked.ok_or(StoreError::NotFound)?;
        query(
            "UPDATE invitations SET revoked_at = unixepoch()
             WHERE grant_id = ? AND consumed_at IS NULL AND revoked_at IS NULL",
        )
        .bind(grant_id)
        .execute(&mut *transaction)
        .await?;
        query(
            "UPDATE sessions SET ended_at = unixepoch(), end_reason = 'revoked'
             WHERE grant_id = ? AND ended_at IS NULL",
        )
        .bind(grant_id)
        .execute(&mut *transaction)
        .await?;
        audit(
            &mut transaction,
            Some(team_id),
            Some(stored_key_id),
            Some(grant_id),
            None,
            Some(&actor_public_key),
            "grant.revoke",
            "succeeded",
            None,
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn revoke_invitation(
        &self,
        invitation_id: i64,
        actor_public_key: &str,
    ) -> Result<(), StoreError> {
        let actor_public_key = PublicKey::from_hex(actor_public_key)?.to_hex();
        let mut transaction = self.pool.begin().await?;
        let grant_id: Option<i64> = query_scalar(
            "UPDATE invitations SET revoked_at = unixepoch()
             WHERE id = ? AND consumed_at IS NULL AND revoked_at IS NULL RETURNING grant_id",
        )
        .bind(invitation_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let grant_id = grant_id.ok_or(StoreError::NotFound)?;
        let (team_id, stored_key_id): (i64, i64) =
            query_as("SELECT team_id, stored_key_id FROM grants WHERE id = ?")
                .bind(grant_id)
                .fetch_one(&mut *transaction)
                .await?;
        audit(
            &mut transaction,
            Some(team_id),
            Some(stored_key_id),
            Some(grant_id),
            None,
            Some(&actor_public_key),
            "invitation.revoke",
            "succeeded",
            None,
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn active_grants(&self) -> Result<Vec<RuntimeGrant>, StoreError> {
        self.query_grants(None).await
    }
    pub async fn grant_for_recipient(
        &self,
        recipient: &str,
    ) -> Result<Option<RuntimeGrant>, StoreError> {
        Ok(self.query_grants(Some(recipient)).await?.pop())
    }
    async fn query_grants(&self, recipient: Option<&str>) -> Result<Vec<RuntimeGrant>, StoreError> {
        let rows: Vec<(
            i64,
            i64,
            i64,
            String,
            Vec<u8>,
            i64,
            String,
            String,
            Vec<u8>,
            i64,
            String,
            String,
            Option<i64>,
        )> = query_as(
            if recipient.is_some() {"SELECT g.id, g.team_id, g.stored_key_id, g.remote_signer_public_key,
                    g.remote_signer_secret_envelope, g.envelope_version,
                    g.key_encryption_key_id, k.public_key, k.secret_envelope,
                    k.envelope_version, k.key_encryption_key_id, p.document, g.expires_at
             FROM grants g
             JOIN stored_keys k ON k.id = g.stored_key_id AND k.team_id = g.team_id
             JOIN policies p ON p.id = g.policy_id AND p.team_id = g.team_id AND p.deleted_at IS NULL
             WHERE (SELECT recovery_pending FROM instance_settings WHERE singleton=1)=0 AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at > unixepoch()) AND g.remote_signer_public_key = ?"} else {"SELECT g.id, g.team_id, g.stored_key_id, g.remote_signer_public_key,
                    g.remote_signer_secret_envelope, g.envelope_version,
                    g.key_encryption_key_id, k.public_key, k.secret_envelope,
                    k.envelope_version, k.key_encryption_key_id, p.document, g.expires_at
             FROM grants g
             JOIN stored_keys k ON k.id = g.stored_key_id AND k.team_id = g.team_id
             JOIN policies p ON p.id = g.policy_id AND p.team_id = g.team_id AND p.deleted_at IS NULL
             WHERE (SELECT recovery_pending FROM instance_settings WHERE singleton=1)=0 AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at > unixepoch()) AND ? IS NULL"},
        ).bind(recipient)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| RuntimeGrant {
                id: row.0,
                team_id: row.1,
                stored_key_id: row.2,
                remote_signer_public_key: row.3,
                remote_signer_secret_envelope: row.4,
                remote_envelope_version: row.5,
                remote_key_id: row.6,
                stored_public_key: row.7,
                stored_secret_envelope: row.8,
                stored_envelope_version: row.9,
                stored_key_id_tag: row.10,
                policy_document: row.11,
                expires_at: row.12,
            })
            .collect())
    }

    pub async fn enabled_relays(&self) -> Result<Vec<String>, StoreError> {
        Ok(
            query_scalar("SELECT url FROM relays WHERE enabled = 1 ORDER BY sort_order, id")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn mark_relay_subscription(
        &self,
        url: &str,
        error: Option<&str>,
    ) -> Result<(), StoreError> {
        query("INSERT INTO relay_checkpoints(relay_id,last_connected_at,consecutive_failures,last_error) SELECT id,CASE WHEN ? IS NULL THEN unixepoch() ELSE NULL END,CASE WHEN ? IS NULL THEN 0 ELSE 1 END,? FROM relays WHERE url=? ON CONFLICT(relay_id) DO UPDATE SET last_connected_at=coalesce(excluded.last_connected_at,relay_checkpoints.last_connected_at),consecutive_failures=CASE WHEN excluded.last_error IS NULL THEN 0 ELSE relay_checkpoints.consecutive_failures+1 END,last_error=excluded.last_error,updated_at=unixepoch()")
            .bind(error).bind(error).bind(error).bind(url).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn mark_relay_received(
        &self,
        relay_url: &str,
        event_id: &str,
        created_at: i64,
    ) -> Result<(), StoreError> {
        query(
            "INSERT INTO relay_checkpoints(
                relay_id, last_event_created_at, last_event_id, last_received_at,
                consecutive_failures, last_error
             ) SELECT id, ?, ?, unixepoch(), 0, NULL FROM relays WHERE url = ?
             ON CONFLICT(relay_id) DO UPDATE SET
                last_event_created_at = excluded.last_event_created_at,
                last_event_id = excluded.last_event_id,
                last_received_at = unixepoch(), consecutive_failures = 0,
                last_error = NULL, updated_at = unixepoch()",
        )
        .bind(created_at)
        .bind(event_id)
        .bind(relay_url)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_relay_published(&self, relay_url: &str) -> Result<(), StoreError> {
        query(
            "INSERT INTO relay_checkpoints(
                relay_id, last_published_at, consecutive_failures, last_error
             ) SELECT id, unixepoch(), 0, NULL FROM relays WHERE url = ?
             ON CONFLICT(relay_id) DO UPDATE SET
                last_published_at = unixepoch(), consecutive_failures = 0,
                last_error = NULL, updated_at = unixepoch()",
        )
        .bind(relay_url)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub fn decrypt_remote_keys(&self, grant: &RuntimeGrant) -> Result<Keys, StoreError> {
        self.check_envelope(grant.remote_envelope_version, &grant.remote_key_id)?;
        let secret = self.cipher.open(
            &grant.remote_signer_secret_envelope,
            &EnvelopeContext {
                team_id: grant.team_id,
                record_id: grant.id,
                public_key: &grant.remote_signer_public_key,
                purpose: "remote-signer-key",
            },
        )?;
        let secret = nostr::prelude::SecretKey::from_slice(&secret)?;
        let keys = Keys::new(secret);
        if keys.public_key().to_hex() != grant.remote_signer_public_key {
            return Err(StoreError::PublicKeyMismatch);
        }
        Ok(keys)
    }

    pub fn decrypt_stored_keys(&self, grant: &RuntimeGrant) -> Result<Keys, StoreError> {
        // Durable inbox I/O can span the expiry boundary after initial admission.
        if grant
            .expires_at
            .is_some_and(|deadline| deadline <= chrono::Utc::now().timestamp())
        {
            return Err(StoreError::NotFound);
        }
        self.check_envelope(grant.stored_envelope_version, &grant.stored_key_id_tag)?;
        let secret = self.cipher.open(
            &grant.stored_secret_envelope,
            &EnvelopeContext {
                team_id: grant.team_id,
                record_id: grant.stored_key_id,
                public_key: &grant.stored_public_key,
                purpose: "stored-key",
            },
        )?;
        let secret = nostr::prelude::SecretKey::from_slice(&secret)?;
        let keys = Keys::new(secret);
        if keys.public_key().to_hex() != grant.stored_public_key {
            return Err(StoreError::PublicKeyMismatch);
        }
        Ok(keys)
    }

    pub fn validate_runtime_grant(&self, grant: &RuntimeGrant) -> Result<(), StoreError> {
        self.decrypt_remote_keys(grant)?;
        self.decrypt_stored_keys(grant)?;
        PolicyDocument::parse(&grant.policy_document)?;
        Ok(())
    }

    pub async fn active_session(
        &self,
        grant_id: i64,
        client: &PublicKey,
    ) -> Result<Option<ActiveSession>, StoreError> {
        let row: Option<(i64, Option<String>)> = query_as(
            "SELECT s.id, s.requested_capabilities FROM sessions s
             JOIN grants g ON g.id = s.grant_id
             WHERE s.grant_id = ? AND s.client_public_key = ? AND s.ended_at IS NULL
               AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at > unixepoch())",
        )
        .bind(grant_id)
        .bind(client.to_hex())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id, requested_capabilities)| ActiveSession {
            id,
            requested_capabilities,
        }))
    }

    /// Cheap pre-admission check prevents unknown clients filling the durable inbox.
    pub async fn can_connect(
        &self,
        grant: i64,
        client: &PublicKey,
        secret: &str,
    ) -> Result<bool, StoreError> {
        let hash = Sha256::digest(secret.as_bytes());
        Ok(query_scalar("SELECT EXISTS(SELECT 1 FROM invitations i WHERE i.grant_id=? AND i.secret_hash=? AND ((i.consumed_at IS NULL AND i.revoked_at IS NULL AND i.expires_at>unixepoch()) OR EXISTS(SELECT 1 FROM sessions s WHERE s.invitation_id=i.id AND s.client_public_key=? AND s.ended_at IS NULL)))")
            .bind(grant).bind(hash.as_slice()).bind(client.to_hex()).fetch_one(&self.pool).await?)
    }

    pub async fn claim_invitation(
        &self,
        grant: &RuntimeGrant,
        client: &PublicKey,
        secret: &str,
        requested: &RequestedCapabilities,
        metadata: Option<&str>,
    ) -> Result<i64, StoreError> {
        let supplied_hash = Sha256::digest(secret.as_bytes());
        let requested_json = requested
            .as_json()
            .map_err(|e| StoreError::InvalidInput(e.to_string()))?;
        let metadata = validate_metadata(metadata)?;

        let mut connection = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let result = async {
            let live:i64=query_scalar("SELECT count(*) FROM grants WHERE id=? AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>unixepoch())")
                .bind(grant.id).fetch_one(&mut *connection).await?;
            if live!=1 {return Err(StoreError::InvitationNotClaimable);}
            let candidates: Vec<(i64, Vec<u8>)> = query_as(
                "SELECT id, secret_hash FROM invitations
                 WHERE grant_id = ? AND consumed_at IS NULL AND revoked_at IS NULL
                   AND expires_at > unixepoch()",
            )
            .bind(grant.id)
            .fetch_all(&mut *connection)
            .await?;

            let invitation_id = candidates
                .iter()
                .find(|(_, hash)| constant_time_eq(hash, supplied_hash.as_slice()))
                .map(|(id, _)| *id);

            let Some(invitation_id) = invitation_id else {
                // A process may die after committing the claim but before caching/publishing the
                // response. Repeating the same connect for the same client is safe and lets that
                // request recover without making the invitation reusable by another client.
                let existing: Vec<(i64, Vec<u8>)> = query_as(
                    "SELECT s.id, i.secret_hash FROM sessions s
                     JOIN invitations i ON i.id = s.invitation_id AND i.grant_id = s.grant_id
                     WHERE s.grant_id = ? AND s.client_public_key = ? AND s.ended_at IS NULL
                       AND i.consumed_at IS NOT NULL",
                )
                .bind(grant.id)
                .bind(client.to_hex())
                .fetch_all(&mut *connection)
                .await?;
                if let Some((session_id, _)) = existing
                    .iter()
                    .find(|(_, hash)| constant_time_eq(hash, supplied_hash.as_slice()))
                {
                    return Ok(*session_id);
                }
                return Err(StoreError::InvitationNotClaimable);
            };

            query(
                "UPDATE sessions SET ended_at = unixepoch(), end_reason = 'replaced'
                 WHERE grant_id = ? AND client_public_key = ? AND ended_at IS NULL",
            )
            .bind(grant.id)
            .bind(client.to_hex())
            .execute(&mut *connection)
            .await?;
            let consumed = query(
                "UPDATE invitations SET consumed_at = unixepoch()
                 WHERE id = ? AND consumed_at IS NULL AND revoked_at IS NULL
                   AND expires_at > unixepoch()",
            )
            .bind(invitation_id)
            .execute(&mut *connection)
            .await?;
            if consumed.rows_affected() != 1 {
                return Err(StoreError::InvitationNotClaimable);
            }
            let session_id: i64 = query_scalar(
                "INSERT INTO sessions(
                    grant_id, invitation_id, client_public_key, requested_capabilities, client_metadata
                 ) VALUES (?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(grant.id)
            .bind(invitation_id)
            .bind(client.to_hex())
            .bind(requested_json)
            .bind(metadata)
            .fetch_one(&mut *connection)
            .await?;
            query(
                "INSERT INTO audit_events(
                    team_id, stored_key_id, grant_id, session_id, actor_public_key,
                    action, outcome, details
                 ) VALUES (?, ?, ?, ?, ?, 'session.connect', 'succeeded', '{}')",
            )
            .bind(grant.team_id)
            .bind(grant.stored_key_id)
            .bind(grant.id)
            .bind(session_id)
            .bind(client.to_hex())
            .execute(&mut *connection)
            .await?;
            Ok::<i64, StoreError>(session_id)
        }
        .await;

        match result {
            Ok(session_id) => {
                connection.commit().await?;
                Ok(session_id)
            }
            Err(error) => {
                let _ = connection.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn end_session(&self, session_id: i64) -> Result<(), StoreError> {
        query(
            "UPDATE sessions SET ended_at = unixepoch(), end_reason = 'logout'
             WHERE id = ? AND ended_at IS NULL",
        )
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn touch_session(&self, session_id: i64) -> Result<(), StoreError> {
        // A recent heartbeat may skip its write, but an ended/missing session must fail closed.
        let live: bool = query_scalar("SELECT EXISTS(SELECT 1 FROM sessions s JOIN grants g ON g.id=s.grant_id WHERE s.id=? AND s.ended_at IS NULL AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at>unixepoch()))")
            .bind(session_id).fetch_one(&self.pool).await?;
        if !live {
            return Err(StoreError::NotFound);
        }
        query("UPDATE sessions SET last_seen_at = unixepoch() WHERE id = ? AND ended_at IS NULL AND last_seen_at < unixepoch()-60")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn cached_response(&self, event_id: &str) -> Result<Option<String>, StoreError> {
        Ok(query_scalar(
            "SELECT response_event_json FROM processed_requests
             WHERE event_id = ? AND response_event_json IS NOT NULL",
        )
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?
        .flatten())
    }

    pub async fn begin_request(
        &self,
        event_id: &str,
        grant: &RuntimeGrant,
        session_id: Option<i64>,
        client: &PublicKey,
        request_id: &str,
        method: &str,
        request_json: &str,
    ) -> Result<bool, StoreError> {
        let result = query(
            "INSERT INTO processed_requests(
                event_id, grant_id, session_id, client_public_key, request_id, method, status, request_event_json
             ) VALUES (?, ?, ?, ?, ?, ?, 'processing', ?) ON CONFLICT(event_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(grant.id)
        .bind(session_id)
        .bind(client.to_hex())
        .bind(request_id)
        .bind(method)
        .bind(request_json)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 1 {
            return Ok(true);
        }
        let reclaimed = query(
            "UPDATE processed_requests
             SET session_id = ?, status = 'processing', reason_code = NULL
             WHERE event_id = ? AND response_event_json IS NULL AND status = 'processing'
",
        )
        .bind(session_id)
        .bind(event_id)
        .execute(&self.pool)
        .await?;
        Ok(reclaimed.rows_affected() == 1)
    }

    pub async fn cache_response(
        &self,
        event_id: &str,
        session_id: Option<i64>,
        response_json: &str,
        approved: bool,
        reason: Option<&str>,
        logout: bool,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        query(
            "UPDATE processed_requests SET session_id = ?, status = ?, reason_code = ?,
                    response_event_json = ?, completed_at = unixepoch()
             WHERE event_id = ?",
        )
        .bind(session_id)
        .bind(if approved { "approved" } else { "denied" })
        .bind(reason)
        .bind(response_json)
        .bind(event_id)
        .execute(&mut *tx)
        .await?;
        query("INSERT INTO audit_events(team_id,stored_key_id,grant_id,session_id,actor_public_key,action,outcome,reason_code,request_event_id)
            SELECT g.team_id,g.stored_key_id,g.id,r.session_id,r.client_public_key,'nip46.'||r.method,?,r.reason_code,r.event_id FROM processed_requests r JOIN grants g ON g.id=r.grant_id WHERE r.event_id=?")
            .bind(if approved {"allowed"} else {"denied"}).bind(event_id).execute(&mut *tx).await?;
        if logout {
            query("UPDATE sessions SET ended_at=unixepoch(),end_reason='logout' WHERE id=? AND ended_at IS NULL").bind(session_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_published(&self, event_id: &str, success: bool) -> Result<(), StoreError> {
        query("UPDATE processed_requests SET status = ?, publish_attempts=publish_attempts+1, next_attempt_at=unixepoch()+min(60, 1 << min(publish_attempts,6)) WHERE event_id = ?")
            .bind(if success { "completed" } else { "failed" })
            .bind(event_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_audit(
        &self,
        grant: &RuntimeGrant,
        session_id: Option<i64>,
        client: &PublicKey,
        method: &str,
        allowed: bool,
        reason: Option<&str>,
        event_id: &str,
    ) -> Result<(), StoreError> {
        query(
            "INSERT INTO audit_events(
                team_id, stored_key_id, grant_id, session_id, actor_public_key,
                action, outcome, reason_code, request_event_id, details
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, '{}')",
        )
        .bind(grant.team_id)
        .bind(grant.stored_key_id)
        .bind(grant.id)
        .bind(session_id)
        .bind(client.to_hex())
        .bind(format!("nip46.{method}"))
        .bind(if allowed { "allowed" } else { "denied" })
        .bind(reason)
        .bind(event_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn counts(&self) -> Result<(i64, i64, i64, i64, Option<i64>), StoreError> {
        let grants = query_scalar(
            "SELECT count(*) FROM grants WHERE revoked_at IS NULL
             AND (expires_at IS NULL OR expires_at > unixepoch())",
        )
        .fetch_one(&self.pool)
        .await?;
        let sessions = query_scalar(
            "SELECT count(*) FROM sessions s JOIN grants g ON g.id = s.grant_id
             WHERE s.ended_at IS NULL AND g.revoked_at IS NULL
               AND (g.expires_at IS NULL OR g.expires_at > unixepoch())",
        )
        .fetch_one(&self.pool)
        .await?;
        let invitations = query_scalar(
            "SELECT count(*) FROM invitations i JOIN grants g ON g.id = i.grant_id
             WHERE i.consumed_at IS NULL AND i.revoked_at IS NULL
               AND i.expires_at > unixepoch() AND g.revoked_at IS NULL
               AND (g.expires_at IS NULL OR g.expires_at > unixepoch())",
        )
        .fetch_one(&self.pool)
        .await?;
        let relays = query_scalar("SELECT count(*) FROM relays WHERE enabled = 1")
            .fetch_one(&self.pool)
            .await?;
        let last = query_scalar(
            "SELECT max(completed_at) FROM processed_requests WHERE status = 'completed'",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok((grants, sessions, invitations, relays, last))
    }

    pub async fn resources(&self) -> Result<keycast_core::v2::control::ResourceStatus, StoreError> {
        let (inbox_records, inbox_bytes): (i64, i64) =
            query_as("SELECT records,bytes FROM request_storage_budget WHERE singleton=1")
                .fetch_one(&self.pool)
                .await?;
        let (pending_inputs,pending_responses,oldest_response_age_seconds):(i64,i64,i64)=query_as("SELECT coalesce(sum(status='processing'),0),coalesce(sum(response_event_json IS NOT NULL AND status IN ('approved','denied','failed')),0),coalesce(max(CASE WHEN response_event_json IS NOT NULL AND status IN ('approved','denied','failed') THEN max(0,unixepoch()-received_at) ELSE 0 END),0) FROM processed_requests").fetch_one(&self.pool).await?;
        let pages: i64 = query_scalar("PRAGMA page_count")
            .fetch_one(&self.pool)
            .await?;
        let page_size: i64 = query_scalar("PRAGMA page_size")
            .fetch_one(&self.pool)
            .await?;
        let files: Vec<(i64, String, String)> = query_as("PRAGMA database_list")
            .fetch_all(&self.pool)
            .await?;
        let wal_bytes = files
            .iter()
            .find(|(_, name, _)| name == "main")
            .filter(|(_, _, path)| !path.is_empty())
            .and_then(|(_, _, path)| std::fs::metadata(format!("{path}-wal")).ok())
            .map_or(0, |m| m.len());
        let last_backup_at =
            query_scalar("SELECT last_backup_at FROM instance_settings WHERE singleton=1")
                .fetch_one(&self.pool)
                .await?;
        Ok(keycast_core::v2::control::ResourceStatus {
            inbox_records,
            inbox_bytes,
            pending_inputs,
            pending_responses,
            oldest_response_age_seconds,
            database_bytes: pages * page_size,
            wal_bytes,
            last_backup_at,
        })
    }

    pub async fn prune_requests(&self) -> Result<(), StoreError> {
        query("DELETE FROM processed_requests WHERE event_id IN (SELECT event_id FROM processed_requests WHERE received_at<unixepoch()-600 OR (status='processing' AND received_at<unixepoch()-300) ORDER BY received_at LIMIT 500)").execute(&self.pool).await?;
        Ok(())
    }
    pub async fn maintenance(&self) -> Result<(), StoreError> {
        query("UPDATE sessions SET ended_at=unixepoch(),end_reason='expired' WHERE ended_at IS NULL AND grant_id IN (SELECT id FROM grants WHERE expires_at<=unixepoch())").execute(&self.pool).await?;

        query("DELETE FROM audit_events WHERE id IN (SELECT id FROM audit_events WHERE occurred_at<unixepoch()-2592000 ORDER BY occurred_at LIMIT 500)").execute(&self.pool).await?;
        query("DELETE FROM management_nonces WHERE accepted_at<unixepoch()-600")
            .execute(&self.pool)
            .await?;
        query("DELETE FROM invitations WHERE id IN (SELECT id FROM invitations WHERE expires_at<unixepoch()-2592000 AND NOT EXISTS(SELECT 1 FROM sessions WHERE invitation_id=invitations.id) LIMIT 500)").execute(&self.pool).await?;
        query("DELETE FROM sessions WHERE id IN (SELECT id FROM sessions WHERE ended_at<unixepoch()-2592000 LIMIT 500)").execute(&self.pool).await?;
        Ok(())
    }
    pub async fn pending_inputs(&self) -> Result<Vec<String>, StoreError> {
        Ok(query_scalar("WITH candidates AS (SELECT request_event_json,received_at,event_id,ROW_NUMBER() OVER (PARTITION BY grant_id,client_public_key ORDER BY received_at,event_id) AS client_position,ROW_NUMBER() OVER (PARTITION BY grant_id ORDER BY received_at,event_id) AS grant_position FROM processed_requests WHERE status='processing' AND request_event_json IS NOT NULL AND received_at>=unixepoch()-300) SELECT request_event_json FROM candidates WHERE client_position<=2 AND grant_position<=8 ORDER BY grant_position,received_at,event_id LIMIT 32").fetch_all(&self.pool).await?)
    }
    pub async fn outbox(&self) -> Result<Vec<(String, String)>, StoreError> {
        Ok(query_as("WITH candidates AS (SELECT r.event_id,r.response_event_json,r.next_attempt_at,r.received_at,ROW_NUMBER() OVER (PARTITION BY r.grant_id ORDER BY r.next_attempt_at,r.received_at,r.event_id) AS position FROM processed_requests r JOIN grants g ON g.id=r.grant_id WHERE r.status IN ('approved','denied','failed') AND r.response_event_json IS NOT NULL AND r.next_attempt_at<=unixepoch() AND r.received_at>=unixepoch()-600 AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at>unixepoch())) SELECT event_id,response_event_json FROM candidates WHERE position<=4 ORDER BY position,next_attempt_at,received_at,event_id LIMIT 32").fetch_all(&self.pool).await?)
    }
    pub async fn retry_cached(&self, event_id: &str) -> Result<(), StoreError> {
        query("UPDATE processed_requests SET status='approved',next_attempt_at=0 WHERE event_id=? AND response_event_json IS NOT NULL AND status='completed'").bind(event_id).execute(&self.pool).await?;
        Ok(())
    }

    async fn bunker_uri(
        &self,
        remote_public_key: &str,
        secret: &str,
    ) -> Result<String, StoreError> {
        let relays = self.enabled_relays().await?;
        if relays.is_empty() {
            return Err(StoreError::InvalidInput(
                "at least one enabled relay is required".to_string(),
            ));
        }
        let relay_query = relays
            .iter()
            .map(|relay| format!("relay={}", urlencoding::encode(relay)))
            .collect::<Vec<_>>()
            .join("&");
        Ok(format!(
            "bunker://{remote_public_key}?{relay_query}&secret={}",
            urlencoding::encode(secret)
        ))
    }

    async fn stored_key_summary(&self, id: i64) -> Result<StoredKeySummary, StoreError> {
        let row: (i64, i64, String, String, i64, i64) = query_as(
            "SELECT id, team_id, name, public_key, created_at, updated_at
             FROM stored_keys WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        Ok(StoredKeySummary {
            id: row.0,
            team_id: row.1,
            name: row.2,
            public_key: row.3,
            created_at: row.4,
            updated_at: row.5,
        })
    }

    async fn grant_summary(&self, id: i64) -> Result<GrantSummary, StoreError> {
        let row: (
            i64,
            i64,
            i64,
            i64,
            String,
            String,
            Option<i64>,
            Option<i64>,
            i64,
            i64,
        ) = query_as(
            "SELECT id, team_id, stored_key_id, policy_id, name,
                        remote_signer_public_key, expires_at, revoked_at, created_at, updated_at
                 FROM grants WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        Ok(GrantSummary {
            id: row.0,
            team_id: row.1,
            stored_key_id: row.2,
            policy_id: row.3,
            name: row.4,
            remote_signer_public_key: row.5,
            expires_at: row.6,
            revoked_at: row.7,
            created_at: row.8,
            updated_at: row.9,
        })
    }

    fn check_envelope(&self, version: i64, key_id: &str) -> Result<(), StoreError> {
        if version != ENVELOPE_VERSION || key_id != self.cipher.key_id() {
            return Err(StoreError::UnsupportedEnvelope);
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<(), StoreError> {
    if !(1..=120).contains(&name.trim().chars().count()) {
        return Err(StoreError::InvalidInput(
            "name must contain between 1 and 120 characters".to_string(),
        ));
    }
    Ok(())
}

fn validate_metadata(metadata: Option<&str>) -> Result<Option<String>, StoreError> {
    let Some(metadata) = metadata.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    if metadata.len() > 16 * 1024 {
        return Err(StoreError::InvalidInput(
            "client metadata is too large".to_string(),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(metadata)
        .map_err(|_| StoreError::InvalidInput("invalid client metadata".to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| StoreError::InvalidInput("client metadata must be an object".to_string()))?;
    let allowed: BTreeSet<&str> = ["name", "url", "image"].into_iter().collect();
    if object.keys().any(|key| !allowed.contains(key.as_str()))
        || object.values().any(|value| !value.is_string())
    {
        return Err(StoreError::InvalidInput(
            "client metadata contains unsupported fields".to_string(),
        ));
    }
    serde_json::to_string(&value)
        .map(Some)
        .map_err(|e| StoreError::InvalidInput(e.to_string()))
}

fn generate_invitation_secret() -> Result<(Zeroizing<String>, Vec<u8>), StoreError> {
    let mut random = Zeroizing::new([0_u8; 32]);
    fill(&mut *random).map_err(|_| StoreError::Random)?;
    let secret = Zeroizing::new(URL_SAFE_NO_PAD.encode(*random));
    let hash = Sha256::digest(secret.as_bytes()).to_vec();
    Ok((secret, hash))
}

#[allow(clippy::too_many_arguments)]
async fn audit(
    transaction: &mut sqlx::transaction::Transaction<'_, sqlx_sqlite::Sqlite>,
    team_id: Option<i64>,
    stored_key_id: Option<i64>,
    grant_id: Option<i64>,
    session_id: Option<i64>,
    actor_public_key: Option<&str>,
    action: &str,
    outcome: &str,
    reason: Option<&str>,
) -> Result<(), sqlx::Error> {
    query(
        "INSERT INTO audit_events(
            team_id, stored_key_id, grant_id, session_id, actor_public_key,
            action, outcome, reason_code, details
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, '{}')",
    )
    .bind(team_id)
    .bind(stored_key_id)
    .bind(grant_id)
    .bind(session_id)
    .bind(actor_public_key)
    .bind(action)
    .bind(outcome)
    .bind(reason)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use keycast_core::v2::envelope::EnvelopeCipher;
    use sqlx::{query::query, query_scalar::query_scalar, raw_sql::raw_sql};
    use sqlx_sqlite::SqlitePoolOptions;

    async fn store_with_grant() -> (Store, RuntimeGrant, String) {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect("sqlite::memory:")
            .await
            .expect("connect test database");
        query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("enable foreign keys");
        raw_sql(include_str!("../../database/migrations/0001_initial.sql"))
            .execute(&pool)
            .await
            .expect("apply v2 schema");

        let team_id: i64 = query_scalar("INSERT INTO teams(name) VALUES ('Family') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("create team");
        let policy_id: i64 = query_scalar(
            "INSERT INTO policies(team_id, name, document)
             VALUES (?, 'Notes', '{\"version\":1,\"capabilities\":{\"sign_event\":{\"allowed_kinds\":[1]}}}')
             RETURNING id",
        )
        .bind(team_id)
        .fetch_one(&pool)
        .await
        .expect("create policy");

        let store = Store::new(pool, EnvelopeCipher::from_key(Zeroizing::new([42_u8; 32])));
        let user_keys = Keys::generate();
        let actor = user_keys.public_key().to_hex();
        let stored_key = store
            .seal_stored_key(
                team_id,
                &actor,
                "Family key".to_string(),
                Zeroizing::new(user_keys.secret_key().to_secret_hex()),
            )
            .await
            .expect("seal key");
        let (_grant, _invitation_id, bunker_uri) = store
            .create_grant(
                team_id,
                &actor,
                stored_key.id,
                policy_id,
                "Phone".to_string(),
                None,
                chrono::Utc::now().timestamp() + 300,
            )
            .await
            .expect("create grant");
        let grant = store
            .active_grants()
            .await
            .expect("load grant")
            .pop()
            .expect("active grant");
        let secret = bunker_uri
            .split("secret=")
            .nth(1)
            .expect("bunker URI secret")
            .to_string();
        (store, grant, secret)
    }

    #[tokio::test]
    async fn invitation_secret_is_not_consumed_on_mismatch_and_is_single_use() {
        let (store, grant, secret) = store_with_grant().await;
        let lifecycle_actors: Vec<String> = query_scalar(
            "SELECT actor_public_key FROM audit_events
             WHERE action IN ('stored_key.create', 'grant.create') ORDER BY id",
        )
        .fetch_all(&store.pool)
        .await
        .expect("load lifecycle audit actors");
        assert_eq!(
            lifecycle_actors,
            vec![
                grant.stored_public_key.clone(),
                grant.stored_public_key.clone()
            ]
        );
        let first_client = Keys::generate().public_key();
        let second_client = Keys::generate().public_key();
        let requested = RequestedCapabilities::unrestricted();

        assert!(matches!(
            store
                .claim_invitation(&grant, &first_client, "wrong-secret", &requested, None)
                .await,
            Err(StoreError::InvitationNotClaimable)
        ));
        let still_claimable: i64 = query_scalar(
            "SELECT count(*) FROM invitations WHERE consumed_at IS NULL AND revoked_at IS NULL",
        )
        .fetch_one(&store.pool)
        .await
        .expect("count claimable invitations");
        assert_eq!(still_claimable, 1);

        let session_id = store
            .claim_invitation(&grant, &first_client, &secret, &requested, None)
            .await
            .expect("claim matching invitation");
        assert!(store
            .active_session(grant.id, &first_client)
            .await
            .expect("load session")
            .is_some_and(|session| session.id == session_id));

        let repeated_session = store
            .claim_invitation(&grant, &first_client, &secret, &requested, None)
            .await
            .expect("same client can recover an interrupted connect response");
        assert_eq!(repeated_session, session_id);

        assert!(matches!(
            store
                .claim_invitation(&grant, &second_client, &secret, &requested, None)
                .await,
            Err(StoreError::InvitationNotClaimable)
        ));
        let sessions: i64 = query_scalar("SELECT count(*) FROM sessions")
            .fetch_one(&store.pool)
            .await
            .expect("count sessions");
        assert_eq!(sessions, 1);
    }

    #[tokio::test]
    async fn invitation_is_not_persisted_when_no_relay_can_be_advertised() {
        let (store, grant, _) = store_with_grant().await;
        query("UPDATE relays SET enabled = 0")
            .execute(&store.pool)
            .await
            .expect("disable relays");
        let before: i64 = query_scalar("SELECT count(*) FROM invitations")
            .fetch_one(&store.pool)
            .await
            .expect("count invitations before");

        assert!(matches!(
            store
                .create_invitation(
                    grant.id,
                    &grant.stored_public_key,
                    chrono::Utc::now().timestamp() + 300,
                )
                .await,
            Err(StoreError::InvalidInput(_))
        ));
        let after: i64 = query_scalar("SELECT count(*) FROM invitations")
            .fetch_one(&store.pool)
            .await
            .expect("count invitations after");
        assert_eq!(after, before);
    }

    #[tokio::test]
    async fn wrong_root_credential_is_detected_before_runtime_readiness() {
        let (store, grant, _) = store_with_grant().await;
        let wrong_cipher_store = Store::new(
            store.pool.clone(),
            EnvelopeCipher::from_key(Zeroizing::new([99_u8; 32])),
        );

        assert!(matches!(
            wrong_cipher_store.validate_runtime_grant(&grant),
            Err(StoreError::UnsupportedEnvelope | StoreError::Envelope(_))
        ));
    }
}
