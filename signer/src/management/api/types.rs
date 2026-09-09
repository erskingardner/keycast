use keycast_core::v2::control::{GrantSummary, SignerStatus, StoredKeySummary};
use keycast_core::v2::policy::PolicyDocument;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTeamRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateTeamRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddTeammateRequest {
    pub user_public_key: String,
    pub role: TeamRole,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TeamRole {
    Admin,
    Member,
}

impl TeamRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }
}

/// `Secret` keeps the imported private key out of `Debug` output and erases its
/// buffer on drop. Intermediate copies inside the HTTP and JSON layers cannot be
/// erased from here, so the trusted CLI remains the stronger import path.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddKeyRequest {
    pub name: String,
    pub secret_key: keycast_core::v2::secret::Secret,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePolicyRequest {
    pub name: String,
    pub document: PolicyDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateGrantRequest {
    pub name: String,
    pub policy_id: i64,
    pub expires_at: Option<i64>,
    pub invitation_expires_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateInvitationRequest {
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub slug: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TeamMember {
    pub user_public_key: String,
    pub role: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicySummary {
    pub id: i64,
    pub team_id: i64,
    pub name: String,
    pub document: PolicyDocument,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct TeamWithRelations {
    pub team: Team,
    pub team_users: Vec<TeamMember>,
    pub stored_keys: Vec<StoredKeySummary>,
    pub policies: Vec<PolicySummary>,
}

#[derive(Debug, Serialize)]
pub struct InvitationStatus {
    pub id: i64,
    pub expires_at: i64,
}

#[derive(Debug, Serialize)]
pub struct GrantWithStatus {
    #[serde(flatten)]
    pub grant: GrantSummary,
    pub active_sessions: i64,
    pub claimable_invitations: i64,
    pub invitations: Vec<InvitationStatus>,
}

#[derive(Debug, Serialize)]
pub struct KeyWithRelations {
    pub team: Team,
    pub stored_key: StoredKeySummary,
    pub grants: Vec<GrantWithStatus>,
    pub relay_discovery: crate::discovery::KeyRelayInfo,
}

#[derive(Debug, Serialize)]
pub struct GrantCreationResponse {
    pub grant: GrantSummary,
    pub bunker_uri: String,
}

#[derive(Debug, Serialize)]
pub struct InvitationCreationResponse {
    pub invitation_id: i64,
    pub bunker_uri: String,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub signer: SignerStatus,
    pub database_ok: bool,
    pub auto_activate_relays: bool,
    pub minimum_connected_relays: i64,
    pub relays: Vec<RelayStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayInput {
    pub url: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateRelaysRequest {
    pub minimum_connected_relays: i64,
    pub relays: Vec<RelayInput>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelayStatus {
    pub discovered: bool,
    pub id: i64,
    pub url: String,
    pub enabled: bool,
    pub sort_order: i64,
    pub last_connected_at: Option<i64>,
    pub last_received_at: Option<i64>,
    pub last_published_at: Option<i64>,
    pub consecutive_failures: i64,
    pub last_error: Option<String>,
    pub diagnostics: Option<crate::relay_diagnostics::RelayDiagnostics>,
    pub reliability: Option<crate::relay_history::RelayReliability>,
}

#[derive(Debug, Serialize)]
pub struct AuditEvent {
    pub id: i64,
    pub occurred_at: i64,
    pub grant_id: Option<i64>,
    pub session_id: Option<i64>,
    pub actor_public_key: Option<String>,
    pub action: String,
    pub outcome: String,
    pub reason_code: Option<String>,
    pub request_event_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayDiscoveryPolicy {
    pub auto_activate: bool,
}
