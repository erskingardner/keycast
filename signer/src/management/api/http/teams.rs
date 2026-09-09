#![allow(clippy::type_complexity)]

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use keycast_core::v2::control::{ControlResponse, LifecycleRequest};
use keycast_core::v2::policy::PolicyDocument;
use nostr::prelude::PublicKey;
use sqlx::{query::query, query_as::query_as, query_scalar::query_scalar};

use crate::management::api::error::{ApiError, ApiResult};
use crate::management::api::extractors::AuthEvent;
use crate::management::api::types::*;
use crate::management::state::KeycastState;

pub async fn list_teams(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
) -> ApiResult<Json<Vec<TeamWithRelations>>> {
    let ids: Vec<i64> = query_scalar(
        "SELECT t.id FROM teams t
         JOIN team_members m ON m.team_id = t.id
         WHERE m.user_public_key = ? ORDER BY lower(t.name), t.id",
    )
    .bind(event.pubkey.to_hex())
    .fetch_all(&state.db)
    .await?;
    let mut teams = Vec::with_capacity(ids.len());
    for id in ids {
        teams.push(team_with_relations(&state.db, id).await?);
    }
    Ok(Json(teams))
}

pub async fn create_team(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Json(request): Json<CreateTeamRequest>,
) -> ApiResult<(StatusCode, Json<TeamWithRelations>)> {
    validate_name(&request.name)?;
    let actor = event.pubkey.to_hex();
    let mut transaction = state.db.begin().await?;
    query("INSERT OR IGNORE INTO users(public_key) VALUES (?)")
        .bind(&actor)
        .execute(&mut *transaction)
        .await?;
    let team_id: i64 = query_scalar("INSERT INTO teams(name) VALUES (?) RETURNING id")
        .bind(request.name.trim())
        .fetch_one(&mut *transaction)
        .await?;
    keycast_core::v2::team_slug::assign_slug(&mut transaction, team_id, request.name.trim())
        .await?;
    query("INSERT INTO team_members(team_id, user_public_key, role) VALUES (?, ?, 'admin')")
        .bind(team_id)
        .bind(&actor)
        .execute(&mut *transaction)
        .await?;
    audit_control(
        &mut transaction,
        team_id,
        &actor,
        "team.create",
        "succeeded",
    )
    .await?;
    transaction.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(team_with_relations(&state.db, team_id).await?),
    ))
}

pub async fn get_team(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path(id): Path<i64>,
) -> ApiResult<Json<TeamWithRelations>> {
    require_member(&state.db, id, &event.pubkey.to_hex()).await?;
    Ok(Json(team_with_relations(&state.db, id).await?))
}

pub async fn update_team(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path(id): Path<i64>,
    Json(request): Json<UpdateTeamRequest>,
) -> ApiResult<Json<Team>> {
    let actor = event.pubkey.to_hex();
    require_admin(&state.db, id, &actor).await?;
    validate_name(&request.name)?;
    let mut transaction = state.db.begin().await?;
    let result = query("UPDATE teams SET name = ?, updated_at = unixepoch() WHERE id = ?")
        .bind(request.name.trim())
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    audit_control(&mut transaction, id, &actor, "team.update", "succeeded").await?;
    transaction.commit().await?;
    Ok(Json(team(&state.db, id).await?))
}

pub async fn delete_team(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    let actor = event.pubkey.to_hex();
    require_admin(&state.db, id, &actor).await?;
    let mut transaction = state.db.begin_with("BEGIN IMMEDIATE").await?;
    audit_control(&mut transaction, id, &actor, "team.delete", "succeeded").await?;
    // `grants` references `policies` with ON DELETE RESTRICT while `policies`
    // cascades from `teams`, so deleting the team aborts unless its grants go
    // first. Revocation is a tombstone rather than a row delete, so even a fully
    // revoked team would otherwise be undeletable. Deleting the grants cascades
    // their invitations, sessions and durable requests; audit rows survive with
    // their team and grant links cleared.
    query("DELETE FROM grants WHERE team_id = ?")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    let result = query("DELETE FROM teams WHERE id = ?")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    transaction.commit().await?;
    state.signer.request(LifecycleRequest::Reload).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_user(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path(id): Path<i64>,
    Json(request): Json<AddTeammateRequest>,
) -> ApiResult<(StatusCode, Json<TeamMember>)> {
    let actor = event.pubkey.to_hex();
    require_admin(&state.db, id, &actor).await?;
    let user_public_key = PublicKey::from_hex(&request.user_public_key)
        .map_err(|_| ApiError::bad_request("invalid user public key"))?
        .to_hex();
    let mut transaction = state.db.begin().await?;
    query("INSERT OR IGNORE INTO users(public_key) VALUES (?)")
        .bind(&user_public_key)
        .execute(&mut *transaction)
        .await?;
    query("INSERT INTO team_members(team_id, user_public_key, role) VALUES (?, ?, ?)")
        .bind(id)
        .bind(&user_public_key)
        .bind(request.role.as_str())
        .execute(&mut *transaction)
        .await?;
    audit_control(
        &mut transaction,
        id,
        &actor,
        "team_member.create",
        "succeeded",
    )
    .await?;
    transaction.commit().await?;
    let member = team_member(&state.db, id, &user_public_key).await?;
    Ok((StatusCode::CREATED, Json(member)))
}

pub async fn remove_user(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path((id, user_public_key)): Path<(i64, String)>,
) -> ApiResult<StatusCode> {
    let actor = event.pubkey.to_hex();
    require_admin(&state.db, id, &actor).await?;
    let user_public_key = PublicKey::from_hex(&user_public_key)
        .map_err(|_| ApiError::bad_request("invalid user public key"))?
        .to_hex();
    let mut transaction = state.db.begin_with("BEGIN IMMEDIATE").await?;
    let role: Option<String> =
        query_scalar("SELECT role FROM team_members WHERE team_id = ? AND user_public_key = ?")
            .bind(id)
            .bind(&user_public_key)
            .fetch_optional(&mut *transaction)
            .await?;
    let Some(role) = role else {
        return Err(ApiError::NotFound);
    };
    if role == "admin" {
        let admin_count: i64 =
            query_scalar("SELECT count(*) FROM team_members WHERE team_id = ? AND role = 'admin'")
                .bind(id)
                .fetch_one(&mut *transaction)
                .await?;
        if admin_count <= 1 {
            return Err(ApiError::bad_request("a team must retain an admin"));
        }
    }
    query("DELETE FROM team_members WHERE team_id = ? AND user_public_key = ?")
        .bind(id)
        .bind(user_public_key)
        .execute(&mut *transaction)
        .await?;
    audit_control(
        &mut transaction,
        id,
        &actor,
        "team_member.remove",
        "succeeded",
    )
    .await?;
    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_key(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path(id): Path<i64>,
    Json(request): Json<AddKeyRequest>,
) -> ApiResult<(
    StatusCode,
    Json<keycast_core::v2::control::StoredKeySummary>,
)> {
    require_admin(&state.db, id, &event.pubkey.to_hex()).await?;
    let response = state
        .signer
        .request(LifecycleRequest::SealStoredKey {
            team_id: id,
            actor_public_key: event.pubkey.to_hex(),
            name: request.name,
            secret_key: request.secret_key,
        })
        .await?;
    match response {
        ControlResponse::StoredKey { key } => Ok((StatusCode::CREATED, Json(key))),
        _ => Err(ApiError::Internal),
    }
}

pub async fn remove_key(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path((id, pubkey)): Path<(i64, String)>,
) -> ApiResult<StatusCode> {
    let actor = event.pubkey.to_hex();
    require_admin(&state.db, id, &actor).await?;
    let pubkey = PublicKey::from_hex(&pubkey)
        .map_err(|_| ApiError::bad_request("invalid key public key"))?
        .to_hex();
    let mut transaction = state.db.begin().await?;
    let result = query("DELETE FROM stored_keys WHERE team_id = ? AND public_key = ?")
        .bind(id)
        .bind(pubkey)
        .execute(&mut *transaction)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    audit_control(
        &mut transaction,
        id,
        &actor,
        "stored_key.remove",
        "succeeded",
    )
    .await?;
    transaction.commit().await?;
    state.signer.request(LifecycleRequest::Reload).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_key(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path((id, pubkey)): Path<(i64, String)>,
) -> ApiResult<Json<KeyWithRelations>> {
    require_admin(&state.db, id, &event.pubkey.to_hex()).await?;
    let pubkey = PublicKey::from_hex(&pubkey)
        .map_err(|_| ApiError::bad_request("invalid key public key"))?
        .to_hex();
    let stored_key = stored_key_by_pubkey(&state.db, id, &pubkey).await?;
    let rows: Vec<(
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
        i64,
        i64,
    )> = query_as(
        "SELECT g.id, g.team_id, g.stored_key_id, g.policy_id, g.name,
                g.remote_signer_public_key, g.expires_at, g.revoked_at,
                g.created_at, g.updated_at,
                (SELECT count(*) FROM sessions s WHERE s.grant_id = g.id AND s.ended_at IS NULL
                    AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at > unixepoch())),
                (SELECT count(*) FROM invitations i WHERE i.grant_id = g.id
                    AND i.consumed_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > unixepoch()
                    AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at > unixepoch()))
         FROM grants g WHERE g.stored_key_id = ? ORDER BY g.created_at DESC",
    )
    .bind(stored_key.id)
    .fetch_all(&state.db)
    .await?;
    let rows_invitations: Vec<(i64,i64,i64)> = query_as("SELECT i.grant_id,i.id,i.expires_at FROM invitations i JOIN grants g ON g.id=i.grant_id WHERE g.stored_key_id=? AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at>unixepoch()) AND i.consumed_at IS NULL AND i.revoked_at IS NULL AND i.expires_at>unixepoch() ORDER BY i.id")
        .bind(stored_key.id).fetch_all(&state.db).await?;
    let mut invitations_by_grant = std::collections::HashMap::<_, Vec<_>>::new();
    for (grant_id, id, expires_at) in rows_invitations {
        invitations_by_grant
            .entry(grant_id)
            .or_default()
            .push(crate::management::api::types::InvitationStatus { id, expires_at });
    }
    let mut grants = Vec::new();
    for row in rows {
        grants.push(GrantWithStatus {
            grant: keycast_core::v2::control::GrantSummary {
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
            },
            active_sessions: row.10,
            claimable_invitations: row.11,
            invitations: invitations_by_grant.remove(&row.0).unwrap_or_default(),
        });
    }
    let relay_discovery = state
        .signer
        .runtime
        .store
        .key_relay_info(stored_key.id)
        .await
        .map_err(|_| ApiError::Internal)?;
    Ok(Json(KeyWithRelations {
        relay_discovery,
        team: team(&state.db, id).await?,
        stored_key,
        grants,
    }))
}

pub async fn add_policy(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path(id): Path<i64>,
    Json(request): Json<CreatePolicyRequest>,
) -> ApiResult<(StatusCode, Json<PolicySummary>)> {
    let actor = event.pubkey.to_hex();
    require_admin(&state.db, id, &actor).await?;
    validate_name(&request.name)?;
    request
        .document
        .validate()
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let document = serde_json::to_string(&request.document)
        .map_err(|_| ApiError::bad_request("invalid policy"))?;
    let mut transaction = state.db.begin().await?;
    let policy_id: i64 =
        query_scalar("INSERT INTO policies(team_id, name, document) VALUES (?, ?, ?) RETURNING id")
            .bind(id)
            .bind(request.name.trim())
            .bind(document)
            .fetch_one(&mut *transaction)
            .await?;
    audit_control(&mut transaction, id, &actor, "policy.create", "succeeded").await?;
    transaction.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(policy(&state.db, id, policy_id).await?),
    ))
}

pub async fn update_policy(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path((id, policy_id)): Path<(i64, i64)>,
    Json(request): Json<CreatePolicyRequest>,
) -> ApiResult<Json<PolicySummary>> {
    let actor = event.pubkey.to_hex();
    require_admin(&state.db, id, &actor).await?;
    validate_name(&request.name)?;
    request
        .document
        .validate()
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let document = serde_json::to_string(&request.document)
        .map_err(|_| ApiError::bad_request("invalid policy"))?;
    let mut transaction = state.db.begin().await?;
    let result = query(
        "UPDATE policies SET name = ?, document = ?, revision = revision + 1,
                updated_at = unixepoch() WHERE id = ? AND team_id = ? AND deleted_at IS NULL",
    )
    .bind(request.name.trim())
    .bind(document)
    .bind(policy_id)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    audit_control(&mut transaction, id, &actor, "policy.update", "succeeded").await?;
    transaction.commit().await?;
    state.signer.request(LifecycleRequest::Reload).await?;
    Ok(Json(policy(&state.db, id, policy_id).await?))
}

pub async fn remove_policy(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path((id, policy_id)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    let actor = event.pubkey.to_hex();
    require_admin(&state.db, id, &actor).await?;
    let mut transaction = state.db.begin_with("BEGIN IMMEDIATE").await?;
    let in_use: i64 =
        query_scalar("SELECT count(*) FROM grants WHERE policy_id = ? AND team_id = ? AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>unixepoch())")
            .bind(policy_id)
            .bind(id)
            .fetch_one(&mut *transaction)
            .await?;
    if in_use > 0 {
        return Err(ApiError::bad_request(
            "revoke active grants using this policy before deleting it",
        ));
    }
    let result = query("UPDATE policies SET deleted_at=unixepoch(),updated_at=unixepoch() WHERE id = ? AND team_id = ? AND deleted_at IS NULL")
        .bind(policy_id)
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    audit_control(&mut transaction, id, &actor, "policy.remove", "succeeded").await?;
    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_grant(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path((id, pubkey)): Path<(i64, String)>,
    Json(request): Json<CreateGrantRequest>,
) -> ApiResult<(StatusCode, Json<GrantCreationResponse>)> {
    require_admin(&state.db, id, &event.pubkey.to_hex()).await?;
    let pubkey = PublicKey::from_hex(&pubkey)
        .map_err(|_| ApiError::bad_request("invalid key public key"))?
        .to_hex();
    let stored_key = stored_key_by_pubkey(&state.db, id, &pubkey).await?;
    let response = state
        .signer
        .request(LifecycleRequest::CreateGrant {
            team_id: id,
            actor_public_key: event.pubkey.to_hex(),
            stored_key_id: stored_key.id,
            policy_id: request.policy_id,
            name: request.name,
            expires_at: request.expires_at,
            invitation_expires_at: request.invitation_expires_at,
        })
        .await?;
    match response {
        ControlResponse::GrantCreated { grant, bunker_uri } => Ok((
            StatusCode::CREATED,
            Json(GrantCreationResponse { grant, bunker_uri }),
        )),
        _ => Err(ApiError::Internal),
    }
}

pub async fn revoke_grant(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path((id, pubkey, grant_id)): Path<(i64, String, i64)>,
) -> ApiResult<StatusCode> {
    require_admin(&state.db, id, &event.pubkey.to_hex()).await?;
    let pubkey = PublicKey::from_hex(&pubkey)
        .map_err(|_| ApiError::bad_request("invalid key public key"))?
        .to_hex();
    require_grant_for_key(&state.db, id, &pubkey, grant_id).await?;
    match state
        .signer
        .request(LifecycleRequest::RevokeGrant {
            grant_id,
            actor_public_key: event.pubkey.to_hex(),
        })
        .await?
    {
        ControlResponse::Ok => Ok(StatusCode::NO_CONTENT),
        _ => Err(ApiError::Internal),
    }
}

pub async fn create_invitation(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path((id, grant_id)): Path<(i64, i64)>,
    Json(request): Json<CreateInvitationRequest>,
) -> ApiResult<(StatusCode, Json<InvitationCreationResponse>)> {
    require_admin(&state.db, id, &event.pubkey.to_hex()).await?;
    require_grant_for_team(&state.db, id, grant_id).await?;
    match state
        .signer
        .request(LifecycleRequest::CreateInvitation {
            grant_id,
            actor_public_key: event.pubkey.to_hex(),
            expires_at: request.expires_at,
        })
        .await?
    {
        ControlResponse::InvitationCreated {
            invitation_id,
            bunker_uri,
        } => Ok((
            StatusCode::CREATED,
            Json(InvitationCreationResponse {
                invitation_id,
                bunker_uri,
            }),
        )),
        _ => Err(ApiError::Internal),
    }
}

pub async fn revoke_invitation(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path((id, invitation_id)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    require_admin(&state.db, id, &event.pubkey.to_hex()).await?;
    let found: Option<i64> = query_scalar(
        "SELECT i.id FROM invitations i JOIN grants g ON g.id = i.grant_id
         WHERE i.id = ? AND g.team_id = ?",
    )
    .bind(invitation_id)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    if found.is_none() {
        return Err(ApiError::NotFound);
    }
    match state
        .signer
        .request(LifecycleRequest::RevokeInvitation {
            invitation_id,
            actor_public_key: event.pubkey.to_hex(),
        })
        .await?
    {
        ControlResponse::Ok => Ok(StatusCode::NO_CONTENT),
        _ => Err(ApiError::Internal),
    }
}

pub async fn list_audit(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Path(id): Path<i64>,
) -> ApiResult<Json<Vec<AuditEvent>>> {
    require_member(&state.db, id, &event.pubkey.to_hex()).await?;
    let rows: Vec<(
        i64,
        i64,
        Option<i64>,
        Option<i64>,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = query_as(
        "SELECT id, occurred_at, grant_id, session_id, actor_public_key, action,
                outcome, reason_code, request_event_id
         FROM audit_events WHERE team_id = ? ORDER BY occurred_at DESC, id DESC LIMIT 200",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| AuditEvent {
                id: row.0,
                occurred_at: row.1,
                grant_id: row.2,
                session_id: row.3,
                actor_public_key: row.4,
                action: row.5,
                outcome: row.6,
                reason_code: row.7,
                request_event_id: row.8,
            })
            .collect(),
    ))
}

pub async fn status(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
) -> ApiResult<Json<StatusResponse>> {
    require_operator(&event.pubkey.to_hex())?;
    let database_ok = state
        .signer
        .runtime
        .integrity_ok
        .load(std::sync::atomic::Ordering::Relaxed);
    let relay_rows: Vec<(
        i64,
        String,
        i64,
        i64,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        i64,
        Option<String>,
    )> = query_as(
        "SELECT r.id, r.url, r.enabled, r.sort_order, c.last_connected_at,
                c.last_received_at, c.last_published_at,
                coalesce(c.consecutive_failures, 0), c.last_error
         FROM relays r LEFT JOIN relay_checkpoints c ON c.relay_id = r.id
         ORDER BY r.sort_order, r.id",
    )
    .fetch_all(&state.db)
    .await?;
    let discovered_ids: Vec<i64> = query_scalar("SELECT id FROM relays WHERE discovered=1")
        .fetch_all(&state.db)
        .await?;
    let auto_activate_relays =
        query_scalar("SELECT auto_activate FROM relay_discovery_policy WHERE singleton=1")
            .fetch_one(&state.db)
            .await?;
    let diagnostics = state.signer.runtime.relay_diagnostics.read().await.clone();
    let mut reliability = state.signer.runtime.store.relay_reliability().await?;
    let relays = relay_rows
        .into_iter()
        .map(|row| RelayStatus {
            diagnostics: diagnostics.get(row.1.trim_end_matches('/')).cloned(),
            reliability: reliability.remove(&row.0),
            discovered: discovered_ids.contains(&row.0),
            id: row.0,
            url: row.1,
            enabled: row.2 == 1,
            sort_order: row.3,
            last_connected_at: row.4,
            last_received_at: row.5,
            last_published_at: row.6,
            consecutive_failures: row.7,
            last_error: row.8,
        })
        .collect();
    let minimum_connected_relays: i64 =
        query_scalar("SELECT minimum_connected_relays FROM instance_settings WHERE singleton = 1")
            .fetch_one(&state.db)
            .await?;
    match state.signer.request(LifecycleRequest::Status).await? {
        ControlResponse::Status { status } => Ok(Json(StatusResponse {
            signer: status,
            database_ok,
            auto_activate_relays,
            minimum_connected_relays,
            relays,
        })),
        _ => Err(ApiError::Internal),
    }
}

pub async fn update_relays(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Json(request): Json<UpdateRelaysRequest>,
) -> ApiResult<Json<Vec<RelayStatus>>> {
    require_operator(&event.pubkey.to_hex())?;
    if request.relays.is_empty() || request.relays.len() > 20 {
        return Err(ApiError::bad_request("configure between 1 and 20 relays"));
    }

    let mut normalized = Vec::with_capacity(request.relays.len());
    let mut unique = std::collections::BTreeSet::new();
    for relay in request.relays {
        let parsed = nostr::types::RelayUrl::parse(relay.url.trim())
            .map_err(|_| ApiError::bad_request("invalid relay URL"))?;
        let url = parsed.to_string();
        let parsed_url =
            url::Url::parse(&url).map_err(|_| ApiError::bad_request("invalid relay URL"))?;
        let is_secure = parsed_url.scheme() == "wss";
        let is_local = matches!(
            parsed_url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]")
        );
        if !parsed_url.username().is_empty()
            || parsed_url.password().is_some()
            || parsed_url.fragment().is_some()
        {
            return Err(ApiError::bad_request(
                "relay credentials and fragments are forbidden",
            ));
        }
        if !is_secure && !is_local {
            return Err(ApiError::bad_request(
                "relay URLs must use wss, except for loopback relays",
            ));
        }
        if !unique.insert(url.clone()) {
            return Err(ApiError::bad_request("relay URLs must be unique"));
        }
        normalized.push((url, relay.enabled));
    }

    let enabled = normalized.iter().filter(|(_, enabled)| *enabled).count() as i64;
    if request.minimum_connected_relays < 1 || request.minimum_connected_relays > enabled {
        return Err(ApiError::bad_request(
            "minimum connected relays must be between 1 and the enabled relay count",
        ));
    }

    let mut transaction = state.db.begin().await?;
    let urls = serde_json::to_string(&normalized.iter().map(|(url, _)| url).collect::<Vec<_>>())
        .map_err(|_| ApiError::Internal)?;
    query("DELETE FROM relays WHERE discovered=0 AND url NOT IN (SELECT value FROM json_each(?))")
        .bind(urls)
        .execute(&mut *transaction)
        .await?;
    for (index, (url, enabled)) in normalized.into_iter().enumerate() {
        query(
            "INSERT INTO relays(url, enabled, sort_order) VALUES (?, ?, ?)
             ON CONFLICT(url) DO UPDATE SET enabled = excluded.enabled,
                 sort_order = excluded.sort_order, discovered=0, updated_at = unixepoch()",
        )
        .bind(url)
        .bind(enabled)
        .bind((index as i64 + 1) * 10)
        .execute(&mut *transaction)
        .await?;
    }
    query(
        "UPDATE instance_settings SET minimum_connected_relays = ?, updated_at = unixepoch()
         WHERE singleton = 1",
    )
    .bind(request.minimum_connected_relays)
    .execute(&mut *transaction)
    .await?;
    query("INSERT INTO audit_events(actor_public_key,action,outcome) VALUES (?,'relays.update','succeeded')").bind(event.pubkey.to_hex()).execute(&mut *transaction).await?;
    query("UPDATE instance_settings SET authority_revision=authority_revision+1 WHERE singleton=1")
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    state.signer.request(LifecycleRequest::Reload).await?;

    let rows: Vec<(
        i64,
        String,
        i64,
        i64,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        i64,
        Option<String>,
    )> = query_as(
        "SELECT r.id, r.url, r.enabled, r.sort_order, c.last_connected_at,
                c.last_received_at, c.last_published_at,
                coalesce(c.consecutive_failures, 0), c.last_error
         FROM relays r LEFT JOIN relay_checkpoints c ON c.relay_id = r.id
         ORDER BY r.sort_order, r.id",
    )
    .fetch_all(&state.db)
    .await?;
    let discovered_ids: Vec<i64> = query_scalar("SELECT id FROM relays WHERE discovered=1")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| RelayStatus {
                diagnostics: None,
                reliability: None,
                discovered: discovered_ids.contains(&row.0),
                id: row.0,
                url: row.1,
                enabled: row.2 == 1,
                sort_order: row.3,
                last_connected_at: row.4,
                last_received_at: row.5,
                last_published_at: row.6,
                consecutive_failures: row.7,
                last_error: row.8,
            })
            .collect(),
    ))
}

async fn require_member(
    pool: &sqlx_sqlite::SqlitePool,
    team_id: i64,
    actor: &str,
) -> ApiResult<()> {
    let exists: i64 = query_scalar(
        "SELECT EXISTS(SELECT 1 FROM team_members WHERE team_id = ? AND user_public_key = ?)",
    )
    .bind(team_id)
    .bind(actor)
    .fetch_one(pool)
    .await?;
    if exists == 1 {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

async fn require_admin(pool: &sqlx_sqlite::SqlitePool, team_id: i64, actor: &str) -> ApiResult<()> {
    let exists: i64 = query_scalar(
        "SELECT EXISTS(SELECT 1 FROM team_members
         WHERE team_id = ? AND user_public_key = ? AND role = 'admin')",
    )
    .bind(team_id)
    .bind(actor)
    .fetch_one(pool)
    .await?;
    if exists == 1 {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

async fn require_grant_for_team(
    pool: &sqlx_sqlite::SqlitePool,
    team_id: i64,
    grant_id: i64,
) -> ApiResult<()> {
    let exists: i64 =
        query_scalar("SELECT EXISTS(SELECT 1 FROM grants WHERE id = ? AND team_id = ?)")
            .bind(grant_id)
            .bind(team_id)
            .fetch_one(pool)
            .await?;
    if exists == 1 {
        Ok(())
    } else {
        Err(ApiError::NotFound)
    }
}

async fn require_grant_for_key(
    pool: &sqlx_sqlite::SqlitePool,
    team_id: i64,
    pubkey: &str,
    grant_id: i64,
) -> ApiResult<()> {
    let exists: i64 = query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM grants g JOIN stored_keys k ON k.id = g.stored_key_id
            WHERE g.id = ? AND g.team_id = ? AND k.public_key = ?
         )",
    )
    .bind(grant_id)
    .bind(team_id)
    .bind(pubkey)
    .fetch_one(pool)
    .await?;
    if exists == 1 {
        Ok(())
    } else {
        Err(ApiError::NotFound)
    }
}

async fn team(pool: &sqlx_sqlite::SqlitePool, id: i64) -> ApiResult<Team> {
    let row: (i64, String, i64, i64, Option<String>) =
        query_as("SELECT id, name, created_at, updated_at, slug FROM teams WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or(ApiError::NotFound)?;
    Ok(Team {
        id: row.0,
        name: row.1,
        created_at: row.2,
        updated_at: row.3,
        slug: row.4,
    })
}

async fn team_member(
    pool: &sqlx_sqlite::SqlitePool,
    team_id: i64,
    pubkey: &str,
) -> ApiResult<TeamMember> {
    let row: (String, String, i64) = query_as(
        "SELECT user_public_key, role, created_at FROM team_members
         WHERE team_id = ? AND user_public_key = ?",
    )
    .bind(team_id)
    .bind(pubkey)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(TeamMember {
        user_public_key: row.0,
        role: row.1,
        created_at: row.2,
    })
}

async fn stored_key_by_pubkey(
    pool: &sqlx_sqlite::SqlitePool,
    team_id: i64,
    pubkey: &str,
) -> ApiResult<keycast_core::v2::control::StoredKeySummary> {
    let row: (i64, i64, String, String, i64, i64) = query_as(
        "SELECT id, team_id, name, public_key, created_at, updated_at
         FROM stored_keys WHERE team_id = ? AND public_key = ?",
    )
    .bind(team_id)
    .bind(pubkey)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(keycast_core::v2::control::StoredKeySummary {
        id: row.0,
        team_id: row.1,
        name: row.2,
        public_key: row.3,
        created_at: row.4,
        updated_at: row.5,
    })
}

async fn policy(
    pool: &sqlx_sqlite::SqlitePool,
    team_id: i64,
    policy_id: i64,
) -> ApiResult<PolicySummary> {
    let row: (i64, i64, String, String, i64, i64, i64) = query_as(
        "SELECT id, team_id, name, document, revision, created_at, updated_at
         FROM policies WHERE team_id = ? AND id = ? AND deleted_at IS NULL",
    )
    .bind(team_id)
    .bind(policy_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(PolicySummary {
        id: row.0,
        team_id: row.1,
        name: row.2,
        document: PolicyDocument::parse(&row.3).map_err(|_| ApiError::Internal)?,
        revision: row.4,
        created_at: row.5,
        updated_at: row.6,
    })
}

async fn team_with_relations(
    pool: &sqlx_sqlite::SqlitePool,
    id: i64,
) -> ApiResult<TeamWithRelations> {
    let team = team(pool, id).await?;
    let member_rows: Vec<(String, String, i64)> = query_as(
        "SELECT user_public_key, role, created_at FROM team_members
         WHERE team_id = ? ORDER BY role, created_at",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let team_users = member_rows
        .into_iter()
        .map(|row| TeamMember {
            user_public_key: row.0,
            role: row.1,
            created_at: row.2,
        })
        .collect();
    let key_rows: Vec<(i64, i64, String, String, i64, i64)> = query_as(
        "SELECT id, team_id, name, public_key, created_at, updated_at
         FROM stored_keys WHERE team_id = ? ORDER BY lower(name), id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let stored_keys = key_rows
        .into_iter()
        .map(|row| keycast_core::v2::control::StoredKeySummary {
            id: row.0,
            team_id: row.1,
            name: row.2,
            public_key: row.3,
            created_at: row.4,
            updated_at: row.5,
        })
        .collect();
    let policy_ids: Vec<i64> = query_scalar(
        "SELECT id FROM policies WHERE team_id = ? AND deleted_at IS NULL ORDER BY lower(name), id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let mut policies = Vec::with_capacity(policy_ids.len());
    for policy_id in policy_ids {
        policies.push(policy(pool, id, policy_id).await?);
    }
    Ok(TeamWithRelations {
        team,
        team_users,
        stored_keys,
        policies,
    })
}

fn validate_name(name: &str) -> ApiResult<()> {
    if !(1..=120).contains(&name.trim().chars().count()) {
        return Err(ApiError::bad_request(
            "name must contain between 1 and 120 characters",
        ));
    }
    Ok(())
}

async fn audit_control(
    transaction: &mut sqlx::transaction::Transaction<'_, sqlx_sqlite::Sqlite>,
    team_id: i64,
    actor: &str,
    action: &str,
    outcome: &str,
) -> Result<(), sqlx::Error> {
    query(
        "INSERT INTO audit_events(team_id, actor_public_key, action, outcome, details)
         VALUES (?, ?, ?, ?, '{}')",
    )
    .bind(team_id)
    .bind(actor)
    .bind(action)
    .bind(outcome)
    .execute(&mut **transaction)
    .await?;
    query("UPDATE instance_settings SET authority_revision=authority_revision+1 WHERE singleton=1")
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

pub fn is_operator(actor: &str) -> bool {
    std::env::var("KEYCAST_OPERATOR_PUBKEYS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .any(|p| p.eq_ignore_ascii_case(actor))
}
pub fn require_operator(actor: &str) -> ApiResult<()> {
    if is_operator(actor) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub async fn update_discovery_policy(
    State(state): State<KeycastState>,
    AuthEvent(event): AuthEvent,
    Json(request): Json<RelayDiscoveryPolicy>,
) -> ApiResult<StatusCode> {
    require_operator(&event.pubkey.to_hex())?;
    let mut tx = state.db.begin().await?;
    query("UPDATE relay_discovery_policy SET auto_activate=? WHERE singleton=1")
        .bind(request.auto_activate)
        .execute(&mut *tx)
        .await?;
    query("UPDATE key_relay_lists SET next_fetch_at=0")
        .execute(&mut *tx)
        .await?;
    query("INSERT INTO audit_events(actor_public_key,action,outcome) VALUES(?,'relay_discovery.update','succeeded')").bind(event.pubkey.to_hex()).execute(&mut *tx).await?;
    query("UPDATE instance_settings SET authority_revision=authority_revision+1 WHERE singleton=1")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state.signer.request(LifecycleRequest::Reload).await?;
    Ok(StatusCode::NO_CONTENT)
}
