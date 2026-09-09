use crate::v2::secret::Secret;

/// Only this authenticated forwarding protocol is exposed on the API socket.
/// The body is zeroizing because a browser key import travels through it.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlRequest {
    Http {
        method: String,
        path: String,
        authorization: Option<String>,
        body: Secret,
    },
    Status,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpReply {
    pub status: u16,
    pub body: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedReply {
    pub encrypted_response: String,
    pub public_key: String,
}

use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub enum LifecycleRequest {
    SealStoredKey {
        team_id: i64,
        actor_public_key: String,
        name: String,
        secret_key: Secret,
    },
    CreateGrant {
        team_id: i64,
        actor_public_key: String,
        stored_key_id: i64,
        policy_id: i64,
        name: String,
        expires_at: Option<i64>,
        invitation_expires_at: i64,
    },
    CreateInvitation {
        grant_id: i64,
        actor_public_key: String,
        expires_at: i64,
    },
    RevokeGrant {
        grant_id: i64,
        actor_public_key: String,
    },
    RevokeInvitation {
        invitation_id: i64,
        actor_public_key: String,
    },
    Reload,
    Status,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlResponse {
    Http {
        reply: HttpReply,
    },
    StoredKey {
        key: StoredKeySummary,
    },
    GrantCreated {
        grant: GrantSummary,
        bunker_uri: String,
    },
    InvitationCreated {
        invitation_id: i64,
        bunker_uri: String,
    },
    Status {
        status: SignerStatus,
    },
    Ok,
    Error {
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredKeySummary {
    pub id: i64,
    pub team_id: i64,
    pub name: String,
    pub public_key: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantSummary {
    pub id: i64,
    pub team_id: i64,
    pub stored_key_id: i64,
    pub policy_id: i64,
    pub name: String,
    pub remote_signer_public_key: String,
    pub expires_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignerStatus {
    pub resources: ResourceStatus,
    pub ready: bool,
    pub quarantined_grants: usize,
    pub integrity_ok: bool,
    pub recovery_pending: bool,
    pub schema_version: i64,
    pub envelope_version: i64,
    pub credential_key_id: Option<String>,
    /// Stable identity that encrypts management replies. Public, and shown so an
    /// operator can compare the browser's pinned value against the host CLI.
    pub management_reply_public_key: Option<String>,
    pub active_grants: i64,
    pub active_sessions: i64,
    pub claimable_invitations: i64,
    pub enabled_relays: i64,
    pub connected_relays: usize,
    pub last_processed_at: Option<i64>,
    pub ingress_rejections: u64,
    pub cached_retries: u64,
    pub retry_coalesced: u64,
    pub retry_throttled: u64,
    pub storage_rejections: u64,
    pub parse_errors: u64,
    pub denied_requests: u64,
    pub relay_failures: u64,
}

/// Redacted capacity and recovery indicators; no keys, payloads or invitation material.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceStatus {
    pub inbox_records: i64,
    pub inbox_bytes: i64,
    pub pending_inputs: i64,
    pub pending_responses: i64,
    pub oldest_response_age_seconds: i64,
    pub database_bytes: i64,
    pub wal_bytes: u64,
    pub last_backup_at: Option<i64>,
}
