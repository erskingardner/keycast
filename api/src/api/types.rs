use chrono::{DateTime, Utc};
use serde::Deserialize;

use keycast_core::types::user::TeamUserRole;

#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamRequest {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddTeammateRequest {
    pub user_public_key: String,
    pub role: TeamUserRole,
}

#[derive(Debug, Deserialize)]
pub struct AddKeyRequest {
    pub name: String,
    pub secret_key: String,
}

#[derive(Debug, Deserialize)]
pub struct PermissionParams {
    pub identifier: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct CreatePolicyRequest {
    pub name: String,
    pub permissions: Vec<PermissionParams>,
}

#[derive(Debug, Deserialize)]
pub struct AddAuthorizationRequest {
    pub policy_id: u32,
    pub relays: Vec<String>,
    pub max_uses: Option<i32>,
    #[serde(default)]
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub expires_at: Option<DateTime<Utc>>,
}
