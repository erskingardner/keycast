use crate::api::types::*;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use nostr_sdk::prelude::*;

use sqlx_sqlite::SqlitePool;

use crate::api::error::{ApiError, ApiResult};
use crate::api::extractors::AuthEvent;
use crate::state::get_key_manager;
use keycast_core::custom_permissions::{allowed_kinds::AllowedKindsConfig, AVAILABLE_PERMISSIONS};
use keycast_core::types::authorization::{
    Authorization, AuthorizationWithRelations, UserAuthorization,
};
use keycast_core::types::permission::{Permission, PolicyPermission};
use keycast_core::types::policy::{Policy, PolicyWithPermissions};
use keycast_core::types::stored_key::{PublicStoredKey, StoredKey};
use keycast_core::types::team::{KeyWithRelations, Team, TeamWithRelations};
use keycast_core::types::user::{TeamUser, User};

pub async fn list_teams(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
) -> ApiResult<Json<Vec<TeamWithRelations>>> {
    let user = match User::find_by_pubkey(&pool, &event.pubkey).await {
        Ok(user) => user,
        Err(_) => {
            return Err(ApiError::not_found("User not found"));
        }
    };

    let teams_with_relations = user.teams(&pool).await?;

    Ok(Json(teams_with_relations))
}

pub async fn create_team(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Json(request): Json<CreateTeamRequest>,
) -> ApiResult<Json<TeamWithRelations>> {
    let mut tx = pool.begin().await?;

    // First, try to insert the user if they don't exist
    sqlx::query::query(
        r#"
            INSERT OR IGNORE INTO users (public_key, created_at, updated_at)
            VALUES (?1, datetime('now'), datetime('now'))
            "#,
    )
    .bind(event.pubkey.to_hex())
    .execute(&mut *tx)
    .await?;

    // Then, insert the team
    let team = sqlx::query_as::query_as::<_, Team>(
        r#"
            INSERT INTO teams (name, created_at, updated_at)
            VALUES (?1, datetime('now'), datetime('now'))
            RETURNING *
            "#,
    )
    .bind(request.name)
    .fetch_one(&mut *tx)
    .await?;

    // Then, create the team_user relationship with admin role
    let team_user = sqlx::query_as::query_as::<_, TeamUser>(
        r#"
            INSERT INTO team_users (team_id, user_public_key, role, created_at, updated_at)
            VALUES (?1, ?2, 'admin', datetime('now'), datetime('now'))
            RETURNING *
            "#,
    )
    .bind(team.id)
    .bind(event.pubkey.to_hex())
    .fetch_one(&mut *tx)
    .await?;

    // Finally, create the default policy, default permission (all permissions allowed), and join them
    let policy = sqlx::query_as::query_as::<_, Policy>(
        r#"
            INSERT INTO policies (team_id, name, created_at, updated_at)
            VALUES (?1, 'All Access', datetime('now'), datetime('now'))
            RETURNING *
            "#,
    )
    .bind(team.id)
    .fetch_one(&mut *tx)
    .await?;

    let allowed_kinds_config = serde_json::to_value(AllowedKindsConfig::default())
        .map_err(|_| ApiError::bad_request("Couldn't serialize allowed kinds config"))?;

    let permission = sqlx::query_as::query_as::<_, Permission>(
        r#"
            INSERT INTO permissions (identifier, config, created_at, updated_at)
            VALUES ('allowed_kinds', ?1, datetime('now'), datetime('now'))
            RETURNING *
            "#,
    )
    .bind(allowed_kinds_config)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query_as::query_as::<_, PolicyPermission>(
        r#"
            INSERT INTO policy_permissions (policy_id, permission_id, created_at, updated_at)
            VALUES (?1, ?2, datetime('now'), datetime('now'))
            RETURNING *
            "#,
    )
    .bind(policy.id)
    .bind(permission.id)
    .fetch_one(&mut *tx)
    .await?;

    let policy_with_permissions = PolicyWithPermissions {
        policy,
        permissions: vec![permission],
    };

    // Commit the transaction
    tx.commit().await?;

    Ok(Json(TeamWithRelations {
        team,
        team_users: vec![team_user],
        stored_keys: vec![],
        policies: vec![policy_with_permissions],
    }))
}

pub async fn get_team(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Path(team_id): Path<u32>,
) -> ApiResult<Json<TeamWithRelations>> {
    verify_teammate(&pool, &event.pubkey, team_id).await?;

    let team_with_relations = Team::find_with_relations(&pool, team_id).await?;

    Ok(Json(team_with_relations))
}

pub async fn update_team(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Json(request): Json<UpdateTeamRequest>,
) -> ApiResult<Json<Team>> {
    verify_admin(&pool, &event.pubkey, request.id).await?;

    let mut tx = pool.begin().await?;

    let team = sqlx::query_as::query_as::<_, Team>(
        r#"
        UPDATE teams SET name = ?1 WHERE id = ?2
        RETURNING *
        "#,
    )
    .bind(request.name)
    .bind(request.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(team))
}

pub async fn delete_team(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Path(team_id): Path<u32>,
) -> ApiResult<StatusCode> {
    verify_admin(&pool, &event.pubkey, team_id).await?;

    let mut tx = pool.begin().await?;

    // Delete order is important to avoid foreign key constraints

    // Delete user_authorizations for all authorizations linked to stored keys in this team
    sqlx::query::query(
        r#"
            DELETE FROM user_authorizations 
            WHERE authorization_id IN (
                SELECT a.id 
                FROM authorizations a
                JOIN stored_keys sk ON a.stored_key_id = sk.id
                WHERE sk.team_id = ?1
            )
            "#,
    )
    .bind(team_id)
    .execute(&mut *tx)
    .await?;

    // Delete authorizations for all stored keys in this team
    sqlx::query::query(
        r#"
            DELETE FROM authorizations 
            WHERE stored_key_id IN (
                SELECT id FROM stored_keys WHERE team_id = ?1
            )
            "#,
    )
    .bind(team_id)
    .execute(&mut *tx)
    .await?;

    // Delete stored keys for this team
    sqlx::query::query("DELETE FROM stored_keys WHERE team_id = ?1")
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

    let permission_ids = sqlx::query_scalar::query_scalar::<_, u32>(
        r#"
            SELECT pp.permission_id
            FROM policy_permissions pp
            JOIN policies p ON p.id = pp.policy_id
            WHERE p.team_id = ?1
        "#,
    )
    .bind(team_id)
    .fetch_all(&mut *tx)
    .await?;

    // Delete policy_permissions for all policies in this team
    sqlx::query::query(
        r#"
            DELETE FROM policy_permissions
            WHERE policy_id IN (
                SELECT id FROM policies WHERE team_id = ?1
            )
            "#,
    )
    .bind(team_id)
    .execute(&mut *tx)
    .await?;

    for permission_id in permission_ids {
        sqlx::query::query(
            r#"
                DELETE FROM permissions
                WHERE id = ?1
                  AND NOT EXISTS (
                    SELECT 1 FROM policy_permissions WHERE permission_id = ?1
                  )
                "#,
        )
        .bind(permission_id)
        .execute(&mut *tx)
        .await?;
    }

    // Delete policies for this team
    sqlx::query::query("DELETE FROM policies WHERE team_id = ?1")
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

    // Delete team_users
    sqlx::query::query("DELETE FROM team_users WHERE team_id = ?1")
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

    // Finally delete the team
    sqlx::query::query("DELETE FROM teams WHERE id = ?1")
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

    // Commit the transaction
    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_user(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Path(team_id): Path<u32>,
    Json(request): Json<AddTeammateRequest>,
) -> ApiResult<Json<TeamUser>> {
    verify_admin(&pool, &event.pubkey, team_id).await?;

    let mut tx = pool.begin().await?;

    let new_user_public_key = PublicKey::from_hex(&request.user_public_key)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    // Verify the user isn't already a member of the team
    if sqlx::query_as::query_as::<_, TeamUser>(
        r#"
        SELECT * FROM team_users WHERE team_id = ?1 AND user_public_key = ?2
        "#,
    )
    .bind(team_id)
    .bind(new_user_public_key.to_hex())
    .fetch_optional(&mut *tx)
    .await?
    .is_some()
    {
        return Err(ApiError::BadRequest(
            "User already a member of this team".to_string(),
        ));
    }

    // First, try to insert the user if they don't exist
    sqlx::query::query(
        r#"
        INSERT OR IGNORE INTO users (public_key, created_at, updated_at)
        VALUES (?1, datetime('now'), datetime('now'))
        "#,
    )
    .bind(new_user_public_key.to_hex())
    .execute(&mut *tx)
    .await?;

    // Then, insert the team_user relationship
    let team_user = sqlx::query_as::query_as::<_, TeamUser>(
        r#"
        INSERT INTO team_users (team_id, user_public_key, role, created_at, updated_at)
        VALUES (?1, ?2, ?3, datetime('now'), datetime('now'))
        RETURNING *
        "#,
    )
    .bind(team_id)
    .bind(new_user_public_key.to_hex())
    .bind(request.role.as_db_str())
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(team_user))
}

pub async fn remove_user(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Path((team_id, user_public_key)): Path<(u32, String)>,
) -> ApiResult<StatusCode> {
    verify_admin(&pool, &event.pubkey, team_id).await?;

    let mut tx = pool.begin().await?;

    let removed_user_public_key =
        PublicKey::from_hex(&user_public_key).map_err(|e| ApiError::bad_request(e.to_string()))?;

    // Check if the user is deleting themselves
    if event.pubkey == removed_user_public_key {
        // At least one admin has to remain in the team
        let remaining_admin_count: i64 = sqlx::query_scalar::query_scalar("SELECT COUNT(*) FROM team_users WHERE team_id = ?1 AND user_public_key != ?2 AND role = 'admin'")
            .bind(team_id)
            .bind(removed_user_public_key.to_hex())
            .fetch_one(&mut *tx)
            .await?;

        if remaining_admin_count == 0 {
            return Err(ApiError::forbidden(
                "Cannot delete the last admin from the team.",
            ));
        }
    }

    // Delete the team_user relationship
    sqlx::query::query("DELETE FROM team_users WHERE team_id = ?1 AND user_public_key = ?2")
        .bind(team_id)
        .bind(removed_user_public_key.to_hex())
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_key(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Path(team_id): Path<u32>,
    Json(request): Json<AddKeyRequest>,
) -> ApiResult<Json<PublicStoredKey>> {
    verify_admin(&pool, &event.pubkey, team_id).await?;

    let mut tx = pool.begin().await?;

    let keys =
        Keys::parse(&request.secret_key).map_err(|e| ApiError::bad_request(e.to_string()))?;

    // Encrypt the secret key
    let key_manager = get_key_manager().unwrap();
    let encrypted_secret = key_manager
        .encrypt(keys.secret_key().as_secret_bytes())
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    // Insert the key
    let key = sqlx::query_as::query_as::<_, StoredKey>(
        r#"
         INSERT INTO stored_keys (team_id, name, public_key, secret_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'))
         RETURNING *
         "#,
    )
    .bind(team_id)
    .bind(request.name)
    .bind(keys.public_key().to_hex())
    .bind(encrypted_secret)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    tx.commit().await?;

    Ok(Json(key.into()))
}

pub async fn remove_key(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Path((team_id, pubkey)): Path<(u32, String)>,
) -> ApiResult<StatusCode> {
    verify_admin(&pool, &event.pubkey, team_id).await?;

    let mut tx = pool.begin().await?;

    let removed_stored_key_public_key =
        PublicKey::from_hex(&pubkey).map_err(|e| ApiError::bad_request(e.to_string()))?;

    // First get the stored key ID
    let stored_key = sqlx::query_as::query_as::<_, StoredKey>(
        "SELECT * FROM stored_keys WHERE team_id = ?1 AND public_key = ?2",
    )
    .bind(team_id)
    .bind(removed_stored_key_public_key.to_hex())
    .fetch_one(&mut *tx)
    .await?;

    // Delete all user_authorizations for this key using the correct stored_key_id
    sqlx::query::query(
        "DELETE FROM user_authorizations WHERE authorization_id IN (SELECT id FROM authorizations WHERE stored_key_id = ?1)"
    )
    .bind(stored_key.id)  // Use stored_key.id instead of public_key
    .execute(&mut *tx)
    .await?;

    // Delete all authorizations for this key using the correct stored_key_id
    sqlx::query::query("DELETE FROM authorizations WHERE stored_key_id = ?1")
        .bind(stored_key.id) // Use stored_key.id instead of public_key
        .execute(&mut *tx)
        .await?;

    // Finally delete the key itself
    sqlx::query::query("DELETE FROM stored_keys WHERE team_id = ?1 AND public_key = ?2")
        .bind(team_id)
        .bind(removed_stored_key_public_key.to_hex())
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_key(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Path((team_id, pubkey)): Path<(u32, String)>,
) -> ApiResult<Json<KeyWithRelations>> {
    verify_admin(&pool, &event.pubkey, team_id).await?;

    let mut tx = pool.begin().await?;

    let stored_key_public_key =
        PublicKey::from_hex(&pubkey).map_err(|e| ApiError::bad_request(e.to_string()))?;

    let team = sqlx::query_as::query_as::<_, Team>(
        r#"
            SELECT * FROM teams WHERE id = ?1
            "#,
    )
    .bind(team_id)
    .fetch_one(&mut *tx)
    .await?;

    let stored_key = sqlx::query_as::query_as::<_, StoredKey>(
        r#"
            SELECT * FROM stored_keys WHERE team_id = ?1 AND public_key = ?2
            "#,
    )
    .bind(team_id)
    .bind(stored_key_public_key.to_hex())
    .fetch_one(&mut *tx)
    .await?;

    // First fetch authorizations with policies
    let authorizations = sqlx::query_as::query_as::<_, Authorization>(
        r#"
            SELECT *
            FROM authorizations
            WHERE stored_key_id = ?1
            "#,
    )
    .bind(stored_key.id)
    .fetch_all(&mut *tx)
    .await?;

    // Then fetch users for each authorization and combine
    let mut complete_authorizations = Vec::new();

    for auth in authorizations {
        let policy = sqlx::query_as::query_as::<_, Policy>(
            r#"
                SELECT *
                FROM policies
                WHERE id = ?1
                "#,
        )
        .bind(auth.policy_id)
        .fetch_one(&mut *tx)
        .await?;

        let users = sqlx::query_as::query_as::<_, UserAuthorization>(
            r#"
                SELECT user_public_key, created_at, updated_at
                FROM user_authorizations
                WHERE authorization_id = ?1
                "#,
        )
        .bind(auth.id)
        .fetch_all(&mut *tx)
        .await?;

        complete_authorizations.push(AuthorizationWithRelations {
            authorization: auth.clone(),
            policy,
            users,
            bunker_connection_string: auth
                .bunker_connection_string()
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }

    Ok(Json(KeyWithRelations {
        team,
        stored_key: stored_key.into(),
        authorizations: complete_authorizations,
    }))
}

pub async fn add_authorization(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Path((team_id, pubkey)): Path<(u32, String)>,
    Json(request): Json<AddAuthorizationRequest>,
) -> ApiResult<Json<Authorization>> {
    verify_admin(&pool, &event.pubkey, team_id).await?;

    let stored_key_public_key =
        PublicKey::from_hex(&pubkey).map_err(|e| ApiError::bad_request(e.to_string()))?;

    let mut tx = pool.begin().await?;

    let stored_key = sqlx::query_as::query_as::<_, StoredKey>(
        r#"
            SELECT * FROM stored_keys WHERE team_id = ?1 AND public_key = ?2
            "#,
    )
    .bind(team_id)
    .bind(stored_key_public_key.to_hex())
    .fetch_one(&mut *tx)
    .await?;

    // Verify policy exists and belongs to the same team as the key.
    let policy_exists = sqlx::query_scalar::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM policies WHERE id = ?1 AND team_id = ?2)",
    )
    .bind(request.policy_id)
    .bind(team_id)
    .fetch_one(&mut *tx)
    .await?;

    if !policy_exists {
        return Err(ApiError::not_found("Policy not found"));
    }

    // Create bunker keys for this authorization
    let bunker_keys = Keys::generate();

    // Encrypt the secret key
    let key_manager = get_key_manager().unwrap();
    let encrypted_bunker_secret = key_manager
        .encrypt(bunker_keys.secret_key().as_secret_bytes())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    // create a secret uuid for the authorization connection string
    let secret = uuid::Uuid::new_v4().to_string();

    let relays =
        serde_json::to_value(&request.relays).map_err(|e| ApiError::bad_request(e.to_string()))?;

    // Create authorization
    let authorization = sqlx::query_as::query_as::<_, Authorization>(
            r#"
            INSERT INTO authorizations (stored_key_id, policy_id, secret, bunker_public_key, bunker_secret, relays, max_uses, expires_at, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now'))
            RETURNING *
            "#,
        )
        .bind(stored_key.id)
        .bind(request.policy_id)
        .bind(secret)
        .bind(bunker_keys.public_key().to_hex())
        .bind(encrypted_bunker_secret)
        .bind(relays)
        .bind(request.max_uses)
        .bind(request.expires_at)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Json(authorization))
}

pub async fn add_policy(
    State(pool): State<SqlitePool>,
    AuthEvent(event): AuthEvent,
    Path(team_id): Path<u32>,
    Json(request): Json<CreatePolicyRequest>,
) -> ApiResult<Json<PolicyWithPermissions>> {
    verify_admin(&pool, &event.pubkey, team_id).await?;

    if request.permissions.is_empty() {
        return Err(ApiError::bad_request(
            "Policy must contain at least one permission",
        ));
    }

    let mut tx = pool.begin().await?;

    // Create the permissions
    let mut permissions = Vec::new();
    for permission in request.permissions {
        if !AVAILABLE_PERMISSIONS.contains(&permission.identifier.as_str()) {
            return Err(ApiError::bad_request(format!(
                "Unknown permission identifier: {}",
                permission.identifier
            )));
        }

        Permission::validate_config(&permission.identifier, &permission.config)
            .map_err(|e| ApiError::bad_request(e.to_string()))?;

        let permission = sqlx::query_as::query_as::<_, Permission>(
            "INSERT INTO permissions (identifier, config, created_at, updated_at) VALUES (?1, ?2, datetime('now'), datetime('now')) RETURNING *",
        )
        .bind(permission.identifier)
        .bind(permission.config)
        .fetch_one(&mut *tx)
        .await?;

        permissions.push(permission);
    }

    // Create the policy
    let policy = sqlx::query_as::query_as::<_, Policy>(
        "INSERT INTO policies (team_id, name, created_at, updated_at) VALUES (?1, ?2, datetime('now'), datetime('now')) RETURNING *",
    )
    .bind(team_id)
    .bind(request.name)
    .fetch_one(&mut *tx)
    .await?;

    // create the policy permissions
    for permission in &permissions {
        sqlx::query::query(
            "INSERT INTO policy_permissions (policy_id, permission_id, created_at, updated_at) VALUES (?1, ?2, datetime('now'), datetime('now'))",
        )
        .bind(policy.id)
        .bind(permission.id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Json(PolicyWithPermissions {
        policy,
        permissions,
    }))
}

pub async fn verify_admin<'a>(
    pool: &'a SqlitePool,
    pubkey: &'a PublicKey,
    team_id: u32,
) -> ApiResult<()> {
    match User::is_team_admin(pool, pubkey, team_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(ApiError::forbidden(
            "You are not authorized to access this team",
        )),
        Err(_) => Err(ApiError::auth("Failed to verify admin status")),
    }
}

pub async fn verify_teammate<'a>(
    pool: &'a SqlitePool,
    pubkey: &'a PublicKey,
    team_id: u32,
) -> ApiResult<()> {
    match User::is_team_teammate(pool, pubkey, team_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(ApiError::forbidden(
            "You are not authorized to access this team",
        )),
        Err(_) => Err(ApiError::auth("Failed to verify team membership")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Path;
    use keycast_core::types::user::TeamUserRole;
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
            "../../../../database/migrations/0001_initial.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    fn auth_event(keys: &Keys) -> Event {
        EventBuilder::new(Kind::TextNote, "")
            .sign_with_keys(keys)
            .unwrap()
    }

    async fn create_team_for(pool: &SqlitePool, keys: &Keys, name: &str) -> TeamWithRelations {
        create_team(
            State(pool.clone()),
            AuthEvent(auth_event(keys)),
            Json(CreateTeamRequest {
                name: name.to_string(),
            }),
        )
        .await
        .unwrap()
        .0
    }

    async fn count_rows(pool: &SqlitePool, table: &str) -> i64 {
        sqlx::query_scalar::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn create_team_creates_admin_and_default_policy() {
        let pool = setup_test_db().await;
        let admin = Keys::generate();

        let response = create_team_for(&pool, &admin, "Ops").await;

        assert_eq!(response.team.name, "Ops");
        assert_eq!(response.team_users.len(), 1);
        assert!(
            User::is_team_admin(&pool, &admin.public_key(), response.team.id)
                .await
                .unwrap()
        );
        assert_eq!(response.policies.len(), 1);
        assert_eq!(response.policies[0].policy.name, "All Access");
        assert_eq!(response.policies[0].permissions.len(), 1);
        assert_eq!(
            response.policies[0].permissions[0].config,
            serde_json::json!({"allowed_kinds": null})
        );
    }

    #[tokio::test]
    async fn add_policy_rejects_empty_unknown_and_malformed_permissions() {
        let pool = setup_test_db().await;
        let admin = Keys::generate();
        let team = create_team_for(&pool, &admin, "Ops").await;

        let empty = add_policy(
            State(pool.clone()),
            AuthEvent(auth_event(&admin)),
            Path(team.team.id),
            Json(CreatePolicyRequest {
                name: "empty".to_string(),
                permissions: vec![],
            }),
        )
        .await
        .unwrap_err();
        assert!(matches!(empty, ApiError::BadRequest(_)));

        let unknown = add_policy(
            State(pool.clone()),
            AuthEvent(auth_event(&admin)),
            Path(team.team.id),
            Json(CreatePolicyRequest {
                name: "unknown".to_string(),
                permissions: vec![PermissionParams {
                    identifier: "missing".to_string(),
                    config: serde_json::json!({}),
                }],
            }),
        )
        .await
        .unwrap_err();
        assert!(matches!(unknown, ApiError::BadRequest(_)));

        let malformed = add_policy(
            State(pool.clone()),
            AuthEvent(auth_event(&admin)),
            Path(team.team.id),
            Json(CreatePolicyRequest {
                name: "malformed".to_string(),
                permissions: vec![PermissionParams {
                    identifier: "allowed_kinds".to_string(),
                    config: serde_json::json!({"sign": [1], "encrypt": null, "decrypt": null}),
                }],
            }),
        )
        .await
        .unwrap_err();
        assert!(matches!(malformed, ApiError::BadRequest(_)));
    }

    #[tokio::test]
    async fn add_policy_accepts_multiple_valid_permissions() {
        let pool = setup_test_db().await;
        let admin = Keys::generate();
        let team = create_team_for(&pool, &admin, "Ops").await;

        let policy = add_policy(
            State(pool.clone()),
            AuthEvent(auth_event(&admin)),
            Path(team.team.id),
            Json(CreatePolicyRequest {
                name: "filtered".to_string(),
                permissions: vec![
                    PermissionParams {
                        identifier: "allowed_kinds".to_string(),
                        config: serde_json::json!({"allowed_kinds": [1]}),
                    },
                    PermissionParams {
                        identifier: "content_filter".to_string(),
                        config: serde_json::json!({"blocked_words": ["secret"]}),
                    },
                ],
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(policy.policy.team_id, team.team.id);
        assert_eq!(policy.permissions.len(), 2);
    }

    #[tokio::test]
    async fn team_members_can_read_team_but_not_admin_routes() {
        let pool = setup_test_db().await;
        let admin = Keys::generate();
        let member = Keys::generate();
        let team = create_team_for(&pool, &admin, "Ops").await;

        let _ = add_user(
            State(pool.clone()),
            AuthEvent(auth_event(&admin)),
            Path(team.team.id),
            Json(AddTeammateRequest {
                user_public_key: member.public_key().to_hex(),
                role: TeamUserRole::Member,
            }),
        )
        .await
        .unwrap();

        let team_response = get_team(
            State(pool.clone()),
            AuthEvent(auth_event(&member)),
            Path(team.team.id),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(team_response.team.id, team.team.id);
        assert_eq!(team_response.team_users.len(), 2);

        let err = add_key(
            State(pool.clone()),
            AuthEvent(auth_event(&member)),
            Path(team.team.id),
            Json(AddKeyRequest {
                name: "member key".to_string(),
                secret_key: Keys::generate().secret_key().to_secret_hex(),
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, ApiError::Forbidden(_)));
    }

    #[tokio::test]
    async fn add_authorization_rejects_policy_from_another_team() {
        let pool = setup_test_db().await;
        let admin = Keys::generate();
        let first_team = create_team_for(&pool, &admin, "First").await;
        let second_team = create_team_for(&pool, &admin, "Second").await;
        let stored_key = Keys::generate();

        sqlx::query::query(
            "INSERT INTO stored_keys (name, team_id, public_key, secret_key, created_at, updated_at)
             VALUES ('key', ?1, ?2, ?3, datetime('now'), datetime('now'))",
        )
        .bind(first_team.team.id)
        .bind(stored_key.public_key().to_hex())
        .bind(vec![1_u8, 2, 3])
        .execute(&pool)
        .await
        .unwrap();

        let err = add_authorization(
            State(pool.clone()),
            AuthEvent(auth_event(&admin)),
            Path((first_team.team.id, stored_key.public_key().to_hex())),
            Json(AddAuthorizationRequest {
                policy_id: second_team.policies[0].policy.id,
                relays: vec!["wss://relay.example".to_string()],
                max_uses: None,
                expires_at: None,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, ApiError::NotFound(_)));
        assert_eq!(count_rows(&pool, "authorizations").await, 0);
    }

    #[tokio::test]
    async fn delete_team_removes_related_rows_without_orphaning_permissions() {
        let pool = setup_test_db().await;
        let admin = Keys::generate();
        let team = create_team_for(&pool, &admin, "Ops").await;
        let stored_key = Keys::generate();
        let auth_user = Keys::generate();

        let stored_key_id: i64 = sqlx::query_scalar::query_scalar(
            "INSERT INTO stored_keys (name, team_id, public_key, secret_key, created_at, updated_at)
             VALUES ('key', ?1, ?2, ?3, datetime('now'), datetime('now'))
             RETURNING id",
        )
        .bind(team.team.id)
        .bind(stored_key.public_key().to_hex())
        .bind(vec![1_u8, 2, 3])
        .fetch_one(&pool)
        .await
        .unwrap();

        let authorization_id: i64 = sqlx::query_scalar::query_scalar(
            "INSERT INTO authorizations
             (stored_key_id, secret, bunker_public_key, bunker_secret, relays, policy_id, created_at, updated_at)
             VALUES (?1, 'secret', ?2, ?3, ?4, ?5, datetime('now'), datetime('now'))
             RETURNING id",
        )
        .bind(stored_key_id)
        .bind(Keys::generate().public_key().to_hex())
        .bind(vec![4_u8, 5, 6])
        .bind(serde_json::json!(["wss://relay.example"]))
        .bind(team.policies[0].policy.id)
        .fetch_one(&pool)
        .await
        .unwrap();

        sqlx::query::query(
            "INSERT INTO users (public_key, created_at, updated_at)
             VALUES (?1, datetime('now'), datetime('now'))",
        )
        .bind(auth_user.public_key().to_hex())
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query::query(
            "INSERT INTO user_authorizations (user_public_key, authorization_id, created_at, updated_at)
             VALUES (?1, ?2, datetime('now'), datetime('now'))",
        )
        .bind(auth_user.public_key().to_hex())
        .bind(authorization_id)
        .execute(&pool)
        .await
        .unwrap();

        delete_team(
            State(pool.clone()),
            AuthEvent(auth_event(&admin)),
            Path(team.team.id),
        )
        .await
        .unwrap();

        assert_eq!(count_rows(&pool, "teams").await, 0);
        assert_eq!(count_rows(&pool, "team_users").await, 0);
        assert_eq!(count_rows(&pool, "stored_keys").await, 0);
        assert_eq!(count_rows(&pool, "authorizations").await, 0);
        assert_eq!(count_rows(&pool, "user_authorizations").await, 0);
        assert_eq!(count_rows(&pool, "policies").await, 0);
        assert_eq!(count_rows(&pool, "policy_permissions").await, 0);
        assert_eq!(count_rows(&pool, "permissions").await, 0);

        let fk_rows: Vec<(String, i64, String, i64)> =
            sqlx::query_as::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(fk_rows.is_empty());
    }

    #[tokio::test]
    async fn remove_user_prevents_deleting_the_last_admin() {
        let pool = setup_test_db().await;
        let admin = Keys::generate();
        let team = create_team_for(&pool, &admin, "Ops").await;

        let err = remove_user(
            State(pool.clone()),
            AuthEvent(auth_event(&admin)),
            Path((team.team.id, admin.public_key().to_hex())),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ApiError::Forbidden(_)));

        let second_admin = Keys::generate();
        let _ = add_user(
            State(pool.clone()),
            AuthEvent(auth_event(&admin)),
            Path(team.team.id),
            Json(AddTeammateRequest {
                user_public_key: second_admin.public_key().to_hex(),
                role: TeamUserRole::Admin,
            }),
        )
        .await
        .unwrap();

        remove_user(
            State(pool.clone()),
            AuthEvent(auth_event(&admin)),
            Path((team.team.id, admin.public_key().to_hex())),
        )
        .await
        .unwrap();

        assert!(
            !User::is_team_teammate(&pool, &admin.public_key(), team.team.id)
                .await
                .unwrap()
        );
        assert!(
            User::is_team_admin(&pool, &second_admin.public_key(), team.team.id)
                .await
                .unwrap()
        );
    }
}
