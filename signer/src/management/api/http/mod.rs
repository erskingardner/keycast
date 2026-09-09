pub mod routes;
pub mod teams;

use axum::{
    body::{to_bytes, Body, Bytes},
    extract::{Query, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
pub use routes::*;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use nostr::prelude::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use thiserror::Error;

use crate::management::state::KeycastState;

/// Common HTTP authentication header names
pub const AUTHORIZATION_HEADER: &str = "Authorization";
const AUTH_EVENT_MAX_AGE_SECONDS: i64 = 60;
const APPROVAL_MAX_AGE_SECONDS: i64 = 300;
const AUTH_EVENT_MAX_FUTURE_SKEW_SECONDS: i64 = 60;
const MAX_AUTH_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicConfig {
    pub pubkey_allowed: bool,
    pub instance_id: String,
    pub authority_revision: i64,
    /// Stable identity that encrypts management replies. A browser pins this and
    /// refuses a reply from any other sender, so the API cannot forge outcomes.
    pub management_reply_public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct PublicConfigQuery {
    pub pubkey: String,
}

#[derive(Debug, Error)]
pub enum AuthenticationError {
    #[error("Invalid base64")]
    InvalidBase64,
    #[error("Invalid utf8")]
    InvalidUtf8,
    #[error("Invalid json")]
    InvalidJson,
    #[error("Invalid event: {0}")]
    InvalidEvent(String),
}

pub async fn auth_middleware(
    State(state): State<KeycastState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let (parts, body) = request.into_parts();

    let body_bytes = match to_bytes(body, MAX_AUTH_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::debug!("Failed to read request body for auth validation: {}", e);
            return response_with_status(StatusCode::PAYLOAD_TOO_LARGE, "Request body too large");
        }
    };

    let mut request = Request::from_parts(parts, Body::from(body_bytes.clone()));

    // Get the authorization header
    let auth_header = match request.headers().get(AUTHORIZATION_HEADER) {
        Some(header) => header,
        None => {
            return response_with_status(StatusCode::UNAUTHORIZED, "Missing authentication header");
        }
    };

    // Convert header to string and validate
    let auth_str = match auth_header.to_str() {
        Ok(str) => str,
        Err(_) => {
            return response_with_status(StatusCode::UNAUTHORIZED, "Invalid authentication format");
        }
    };

    // Validate the token
    let event = match validate_token(auth_str, &request, &body_bytes, &state.public_url) {
        Ok(event) => event,
        Err(e) => {
            tracing::debug!("Token validation failed: {}", e);
            return response_with_status(StatusCode::UNAUTHORIZED, "Invalid credentials");
        }
    };

    if !is_allowed_pubkey(&event.pubkey) {
        tracing::debug!("Token validation failed: pubkey not allowed");
        return response_with_status(StatusCode::FORBIDDEN, "Forbidden");
    }

    let write = !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    );
    if !write {
        let instance: Result<String, _> = sqlx::query_scalar::query_scalar(
            "SELECT instance_id FROM instance_settings WHERE singleton=1",
        )
        .fetch_one(&state.db)
        .await;
        let Ok(instance) = instance else {
            return response_with_status(StatusCode::SERVICE_UNAVAILABLE, "Authority unavailable");
        };
        if keycast_core::v2::management::exact_tag(&event, "instance") != Some(instance.as_str()) {
            return response_with_status(StatusCode::UNAUTHORIZED, "Invalid instance");
        }
        request.extensions_mut().insert(event);
        return next.run(request).await;
    }
    let body_text = std::str::from_utf8(&body_bytes).unwrap_or("");
    let described = keycast_core::v2::management::description(
        request.method().as_str(),
        request.uri().path(),
        body_text,
    );
    // Fail closed before matching. An approval event is displayed by, stored in,
    // and for NIP-46 signers transported to an external key store, so private
    // material must never reach one even if a route or the frontend regresses.
    if described
        .as_deref()
        .is_some_and(keycast_core::v2::management::contains_secret_marker)
        || keycast_core::v2::management::contains_secret_marker(&event.content)
    {
        return response_with_status(
            StatusCode::BAD_REQUEST,
            "Approval content must not contain private key material",
        );
    }
    if described.as_deref() != Some(event.content.as_str()) {
        return response_with_status(
            StatusCode::UNAUTHORIZED,
            "Approval contents do not match the command",
        );
    }
    // Serializes original approval, ACL checks and mutation with NIP-46 admission.
    let _authority = state.signer.runtime.store.authority.lock().await;
    let context: Result<(String, i64), _> = sqlx::query_as::query_as(
        "SELECT instance_id, authority_revision FROM instance_settings WHERE singleton=1",
    )
    .fetch_one(&state.db)
    .await;
    let Ok((instance, revision)) = context else {
        return response_with_status(StatusCode::SERVICE_UNAVAILABLE, "Authority unavailable");
    };
    let Some(recipient) =
        keycast_core::v2::management::approval_context(&event, &instance, revision)
    else {
        return response_with_status(
            StatusCode::CONFLICT,
            "A fresh external-signer management approval is required",
        );
    };
    let nonce = keycast_core::v2::management::exact_tag(&event, "nonce").unwrap();
    let claim = async {
        let mut tx = state.db.begin().await?;
        sqlx::query::query("DELETE FROM management_nonces WHERE accepted_at < unixepoch()-600")
            .execute(&mut *tx)
            .await?;
        sqlx::query::query("INSERT INTO management_nonces(nonce,actor_public_key) VALUES (?,?)")
            .bind(nonce)
            .bind(event.pubkey.to_hex())
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    }
    .await;
    if claim.is_err() {
        return response_with_status(
            StatusCode::CONFLICT,
            "Approval already used or authority unavailable",
        );
    }
    request.extensions_mut().insert(event);
    let response = next.run(request).await;
    let status = response.status().as_u16();
    let bytes = match to_bytes(response.into_body(), MAX_AUTH_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return response_with_status(StatusCode::INTERNAL_SERVER_ERROR, "Response unavailable")
        }
    };
    let reply = keycast_core::v2::control::HttpReply {
        status,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    };
    // A fresh key per reply left NIP-44 with nothing to authenticate: anyone
    // holding the public `response` pubkey could forge a reply. The stable
    // root-derived identity makes NIP-44 authenticate the sender.
    let Ok(reply_keys) = state.signer.runtime.store.cipher.management_reply_keys() else {
        return response_with_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Management reply identity unavailable",
        );
    };
    let encrypted = nostr::nips::nip44::encrypt(
        reply_keys.secret_key(),
        &recipient,
        serde_json::to_string(&reply).unwrap(),
        nostr::nips::nip44::Version::default(),
    );
    use axum::response::IntoResponse;
    match encrypted {
        Ok(encrypted_response) => Json(keycast_core::v2::control::EncryptedReply {
            encrypted_response,
            public_key: reply_keys.public_key().to_hex(),
        })
        .into_response(),
        Err(_) => response_with_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Response encryption failed",
        ),
    }
}

pub async fn public_config(
    State(state): State<KeycastState>,
    Query(query): Query<PublicConfigQuery>,
) -> Result<Json<PublicConfig>, StatusCode> {
    let (instance_id, authority_revision): (String, i64) = sqlx::query_as::query_as(
        "SELECT instance_id, authority_revision FROM instance_settings WHERE singleton=1",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let management_reply_public_key = state
        .signer
        .runtime
        .store
        .cipher
        .management_reply_public_key()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(PublicConfig {
        pubkey_allowed: is_allowed_pubkey_hex(&query.pubkey),
        instance_id,
        authority_revision,
        management_reply_public_key,
    }))
}

fn response_with_status(status: StatusCode, message: &'static str) -> Response {
    Response::builder()
        .status(status)
        .body(message.into())
        .unwrap()
}

fn validate_token(
    token: &str,
    request: &Request<Body>,
    body: &Bytes,
    public_url: &str,
) -> Result<Event, AuthenticationError> {
    // Check prefix
    if !token.starts_with("Nostr ") {
        return Err(AuthenticationError::InvalidEvent(
            "Invalid token prefix".to_string(),
        ));
    }

    let event = extract_auth_event_from_header(token)?;

    validate_auth_event(&event, request, body, public_url)?;
    tracing::debug!("Token validation successful");
    Ok(event)
}

pub fn extract_auth_event_from_header(token: &str) -> Result<Event, AuthenticationError> {
    // Get the base64 part
    let base64_str = token
        .strip_prefix("Nostr ")
        .ok_or_else(|| AuthenticationError::InvalidEvent("Invalid token prefix".to_string()))?
        .trim();

    // Decode base64
    let decoded = match BASE64.decode(base64_str) {
        Ok(bytes) => bytes,
        Err(_) => {
            tracing::debug!("Token validation failed: Invalid base64");
            return Err(AuthenticationError::InvalidBase64);
        }
    };

    // Convert bytes to string
    let json_str = match String::from_utf8(decoded) {
        Ok(s) => s,
        Err(_) => {
            tracing::debug!("Token validation failed: Invalid utf8");
            return Err(AuthenticationError::InvalidUtf8);
        }
    };

    // Parse JSON into Event
    let event: Event = match Event::from_json(&json_str) {
        Ok(evt) => evt,
        Err(_) => {
            tracing::debug!("Token validation failed: Invalid NIP-98 event json");
            return Err(AuthenticationError::InvalidJson);
        }
    };

    Ok(event)
}

pub fn validate_auth_event(
    event: &Event,
    request: &Request<Body>,
    body: &[u8],
    public_url: &str,
) -> Result<(), AuthenticationError> {
    if event.verify().is_err() {
        tracing::debug!("Token validation failed: Event verification failed");
        return Err(AuthenticationError::InvalidEvent(
            "Event verification failed".to_string(),
        ));
    }

    let write = !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    );
    let expected = if write {
        keycast_core::v2::management::MANAGEMENT_KIND
    } else {
        keycast_core::v2::management::MANAGEMENT_READ_KIND
    };
    if event.kind.as_u16() != expected {
        tracing::debug!("Token validation failed: Invalid event kind");
        return Err(AuthenticationError::InvalidEvent(
            "Invalid management event kind".to_string(),
        ));
    }

    let now = chrono::Utc::now().timestamp();
    let created_at = event.created_at.as_secs() as i64;
    if created_at
        < now
            - if write {
                APPROVAL_MAX_AGE_SECONDS
            } else {
                AUTH_EVENT_MAX_AGE_SECONDS
            }
    {
        tracing::debug!("Token validation failed: Event too old");
        return Err(AuthenticationError::InvalidEvent(
            "Event too old".to_string(),
        ));
    }

    if created_at > now + AUTH_EVENT_MAX_FUTURE_SKEW_SECONDS {
        tracing::debug!("Token validation failed: Event too far in the future");
        return Err(AuthenticationError::InvalidEvent(
            "Event too far in the future".to_string(),
        ));
    }

    let u_tag = required_tag_content(event, "u")?;
    let full_url = full_request_url(request, public_url);

    if u_tag != full_url {
        tracing::debug!("Token validation failed: Invalid u tag");
        return Err(AuthenticationError::InvalidEvent(
            "Invalid u tag".to_string(),
        ));
    }

    let method_tag = required_tag_content(event, "method")?;
    if method_tag != request.method().as_str() {
        tracing::debug!("Token validation failed: Invalid method tag");
        return Err(AuthenticationError::InvalidEvent(
            "Invalid method tag".to_string(),
        ));
    }

    validate_payload_tag(event, body)?;

    Ok(())
}

fn full_request_url(request: &Request<Body>, public_url: &str) -> String {
    format!("{}{}", public_url.trim_end_matches('/'), request.uri())
}

fn required_tag_content<'a>(event: &'a Event, name: &str) -> Result<&'a str, AuthenticationError> {
    match tag_contents(event, name).as_slice() {
        [value] => Ok(*value),
        [] => Err(AuthenticationError::InvalidEvent(format!(
            "Missing {name} tag"
        ))),
        _ => Err(AuthenticationError::InvalidEvent(format!(
            "Duplicate {name} tags"
        ))),
    }
}

fn optional_tag_content<'a>(
    event: &'a Event,
    name: &str,
) -> Result<Option<&'a str>, AuthenticationError> {
    match tag_contents(event, name).as_slice() {
        [value] => Ok(Some(*value)),
        [] => Ok(None),
        _ => Err(AuthenticationError::InvalidEvent(format!(
            "Duplicate {name} tags"
        ))),
    }
}

fn tag_contents<'a>(event: &'a Event, kind: &str) -> Vec<&'a str> {
    event
        .tags
        .iter()
        .filter(|tag| tag.kind() == kind)
        .map(|tag| {
            if tag.as_slice().len() == 2 {
                tag.content().unwrap_or("")
            } else {
                ""
            }
        })
        .collect()
}

fn hex_digest(body: &[u8]) -> String {
    Sha256::digest(body)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_payload_tag(event: &Event, body: &[u8]) -> Result<(), AuthenticationError> {
    let payload_tag = optional_tag_content(event, "payload")?;
    if body.is_empty() && payload_tag.is_none() {
        return Ok(());
    }

    let expected = hex_digest(body);
    match payload_tag {
        Some(payload) if payload.eq_ignore_ascii_case(&expected) => Ok(()),
        Some(_) => Err(AuthenticationError::InvalidEvent(
            "Invalid payload tag".to_string(),
        )),
        None => Err(AuthenticationError::InvalidEvent(
            "Missing payload tag".to_string(),
        )),
    }
}

fn is_allowed_pubkey(pubkey: &PublicKey) -> bool {
    is_allowed_pubkey_hex(&pubkey.to_hex())
}

fn is_allowed_pubkey_hex(pubkey_hex: &str) -> bool {
    let allowed_pubkeys = configured_allowed_pubkeys();
    if allowed_pubkeys.is_empty() {
        return open_registration_enabled();
    }

    allowed_pubkeys
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(pubkey_hex))
}

fn open_registration_enabled() -> bool {
    std::env::var("KEYCAST_ALLOW_OPEN_REGISTRATION")
        .ok()
        .is_some_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true"))
}

fn configured_allowed_pubkeys() -> Vec<String> {
    let allowed_pubkeys = env::var("ALLOWED_PUBKEYS").unwrap_or_default();

    parse_allowed_pubkeys(&allowed_pubkeys)
}

fn parse_allowed_pubkeys(raw_allowlist: &str) -> Vec<String> {
    raw_allowlist
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::await_holding_lock)]

    use super::*;
    use axum::http::Method;
    use sqlx::{query::query, query_as::query_as, query_scalar::query_scalar, raw_sql::raw_sql};
    use sqlx_sqlite::{SqlitePool, SqlitePoolOptions};
    use std::collections::BTreeSet;

    use std::sync::Mutex;
    use tower::ServiceExt;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn request(method: Method, uri: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "example.com")
            .body(Body::empty())
            .unwrap()
    }

    fn auth_event(tags: Vec<Tag>, created_at: i64) -> Event {
        EventBuilder::new(
            if tags
                .iter()
                .any(|t| t.kind() == "method" && t.content() != Some("GET"))
            {
                Kind::Custom(keycast_core::v2::management::MANAGEMENT_KIND)
            } else {
                Kind::Custom(keycast_core::v2::management::MANAGEMENT_READ_KIND)
            },
            "",
        )
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at as u64))
        .finalize(&Keys::generate())
        .unwrap()
    }

    fn standard_tags(method: &str, url: &str) -> Vec<Tag> {
        vec![
            Tag::parse(["u", url]).unwrap(),
            Tag::parse(["method", method]).unwrap(),
        ]
    }

    fn state(pool: SqlitePool) -> crate::management::state::KeycastState {
        crate::management::state::KeycastState {
            public_url: "http://example.com/api".into(),
            db: pool.clone(),
            signer: crate::management::state::SignerClient {
                runtime: crate::runtime::RuntimeState::new(crate::store::Store::new(
                    pool,
                    keycast_core::v2::envelope::EnvelopeCipher::from_key(zeroize::Zeroizing::new(
                        [7; 32],
                    )),
                )),
            },
        }
    }

    async fn setup_route_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        query("PRAGMA foreign_keys=ON")
            .execute(&pool)
            .await
            .unwrap();
        raw_sql(concat!(
            include_str!("../../../../../database/migrations/0001_initial.sql"),
            "\n",
            include_str!("../../../../../database/migrations/0004_key_relay_discovery.sql")
        ))
        .execute(&pool)
        .await
        .unwrap();
        raw_sql(include_str!(
            "../../../../../database/migrations/0002_team_slugs.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        raw_sql(include_str!(
            "../../../../../database/migrations/0003_relay_reliability.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        query("UPDATE instance_settings SET instance_id='test-instance'")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    fn auth_header(keys: &Keys, method: &str, url: &str, body: &[u8]) -> String {
        let mut tags = standard_tags(method, url);
        if !body.is_empty() {
            tags.push(Tag::parse(["payload", &hex_digest(body)]).unwrap());
        }
        let kind = if method == "GET" {
            tags.push(Tag::parse(["instance", "test-instance"]).unwrap());
            Kind::Custom(keycast_core::v2::management::MANAGEMENT_READ_KIND)
        } else {
            tags.extend([
                Tag::parse(["instance", "test-instance"]).unwrap(),
                Tag::parse(["revision", "0"]).unwrap(),
                Tag::parse(["nonce", &Keys::generate().public_key().to_hex()]).unwrap(),
                Tag::parse(["response", &keys.public_key().to_hex()]).unwrap(),
            ]);
            Kind::Custom(keycast_core::v2::management::MANAGEMENT_KIND)
        };
        let description = if method == "GET" {
            String::new()
        } else {
            keycast_core::v2::management::description(
                method,
                url,
                std::str::from_utf8(body).unwrap(),
            )
            .unwrap()
        };
        let event = EventBuilder::new(kind, description)
            .tags(tags)
            .finalize(keys)
            .unwrap();

        format!("Nostr {}", BASE64.encode(event.as_json()))
    }

    async fn decrypt_reply(
        response: Response,
        keys: &Keys,
    ) -> keycast_core::v2::control::HttpReply {
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), MAX_AUTH_BODY_BYTES)
            .await
            .unwrap();
        let reply: keycast_core::v2::control::EncryptedReply =
            serde_json::from_slice(&bytes).unwrap();
        let plain = nostr::nips::nip44::decrypt(
            keys.secret_key(),
            &PublicKey::from_hex(&reply.public_key).unwrap(),
            reply.encrypted_response,
        )
        .unwrap();
        serde_json::from_str(&plain).unwrap()
    }

    #[test]
    fn validates_required_url_and_method_tags() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("KEYCAST_PUBLIC_URL");
        let req = request(Method::GET, "/teams");
        let now = chrono::Utc::now().timestamp();
        let event = auth_event(standard_tags("GET", "http://example.com/api/teams"), now);

        assert!(validate_auth_event(&event, &req, &[], "http://example.com/api").is_ok());
    }

    #[test]
    fn rejects_missing_url_or_method_tags() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("KEYCAST_PUBLIC_URL");
        let req = request(Method::GET, "/teams");
        let now = chrono::Utc::now().timestamp();

        let missing_url = auth_event(vec![Tag::parse(["method", "GET"]).unwrap()], now);
        assert!(validate_auth_event(&missing_url, &req, &[], "http://example.com/api").is_err());

        let missing_method = auth_event(
            vec![Tag::parse(["u", "http://example.com/api/teams"]).unwrap()],
            now,
        );
        assert!(validate_auth_event(&missing_method, &req, &[], "http://example.com/api").is_err());
    }

    #[test]
    fn rejects_future_timestamps() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("KEYCAST_PUBLIC_URL");
        let req = request(Method::GET, "/teams");
        let future = chrono::Utc::now().timestamp() + AUTH_EVENT_MAX_FUTURE_SKEW_SECONDS + 60;
        let event = auth_event(standard_tags("GET", "http://example.com/api/teams"), future);

        assert!(validate_auth_event(&event, &req, &[], "http://example.com/api").is_err());
    }

    #[test]
    fn validates_payload_hash_for_body_requests() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("KEYCAST_PUBLIC_URL");
        let req = request(Method::POST, "/teams");
        let body = br#"{"name":"ops"}"#;
        let mut tags = standard_tags("POST", "http://example.com/api/teams");
        let digest = hex_digest(body);
        tags.push(Tag::parse(["payload", &digest]).unwrap());
        let event = auth_event(tags, chrono::Utc::now().timestamp());

        assert!(validate_auth_event(&event, &req, body, "http://example.com/api").is_ok());
    }

    #[test]
    fn rejects_missing_or_mismatched_payload_hashes() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("KEYCAST_PUBLIC_URL");
        let req = request(Method::POST, "/teams");
        let body = br#"{"name":"ops"}"#;
        let now = chrono::Utc::now().timestamp();

        let missing_payload =
            auth_event(standard_tags("POST", "http://example.com/api/teams"), now);
        assert!(
            validate_auth_event(&missing_payload, &req, body, "http://example.com/api").is_err()
        );

        let mut tags = standard_tags("POST", "http://example.com/api/teams");
        tags.push(Tag::parse(["payload", "not-a-real-hash"]).unwrap());
        let mismatched_payload = auth_event(tags, now);
        assert!(
            validate_auth_event(&mismatched_payload, &req, body, "http://example.com/api").is_err()
        );
    }

    #[test]
    fn configured_allowlist_trims_blank_entries() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("ALLOWED_PUBKEYS", " abc, def ,, ghi ");

        assert_eq!(configured_allowed_pubkeys(), vec!["abc", "def", "ghi"]);

        env::remove_var("ALLOWED_PUBKEYS");
    }

    #[test]
    fn configured_public_url_wins_and_forwarded_headers_are_ignored() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("KEYCAST_PUBLIC_URL", "https://keycast.example/api/");
        let req = Request::builder()
            .method(Method::GET)
            .uri("/teams")
            .header("host", "attacker.example")
            .header("x-forwarded-proto", "http")
            .header("x-forwarded-prefix", "/evil")
            .body(Body::empty())
            .unwrap();

        assert_eq!(
            full_request_url(&req, "https://keycast.example/api/"),
            "https://keycast.example/api/teams"
        );
        env::remove_var("KEYCAST_PUBLIC_URL");
    }

    #[tokio::test]
    async fn public_config_returns_only_requested_pubkey_status_without_auth() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pool = setup_route_test_db().await;
        env::set_var("ALLOWED_PUBKEYS", "abc,def");

        let response = crate::management::api::http::routes::routes(state(pool.clone()))
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/config?pubkey=ABC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), MAX_AUTH_BODY_BYTES)
            .await
            .unwrap();
        let config: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(config["pubkey_allowed"], true);
        assert_eq!(config["instance_id"], "test-instance");

        let response = crate::management::api::http::routes::routes(state(pool))
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/config?pubkey=abcdef")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        env::remove_var("ALLOWED_PUBKEYS");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), MAX_AUTH_BODY_BYTES)
            .await
            .unwrap();
        let config: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(config["pubkey_allowed"], false);
    }

    #[test]
    fn allowlist_uses_exact_pubkey_matches() {
        let _guard = ENV_LOCK.lock().unwrap();
        let keys = Keys::generate();
        let pubkey = keys.public_key();
        let similar = format!("{}ff", pubkey.to_hex());

        env::set_var("ALLOWED_PUBKEYS", similar);
        assert!(!is_allowed_pubkey(&pubkey));

        env::set_var("ALLOWED_PUBKEYS", pubkey.to_hex());
        assert!(is_allowed_pubkey(&pubkey));

        env::remove_var("ALLOWED_PUBKEYS");
    }

    #[test]
    fn empty_allowlist_fails_closed_unless_open_registration_is_explicit() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("ALLOWED_PUBKEYS");
        env::remove_var("KEYCAST_ALLOW_OPEN_REGISTRATION");
        assert!(!is_allowed_pubkey_hex("abc"));

        env::set_var("KEYCAST_ALLOW_OPEN_REGISTRATION", "true");
        assert!(is_allowed_pubkey_hex("abc"));
        env::remove_var("KEYCAST_ALLOW_OPEN_REGISTRATION");
    }

    #[tokio::test]
    async fn post_team_requires_external_management_approval_and_encrypts_reply() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pool = setup_route_test_db().await;
        let keys = Keys::generate();
        env::set_var("ALLOWED_PUBKEYS", keys.public_key().to_hex());

        let body = br#"{"name":"Ops"}"#;
        let header = auth_header(&keys, "POST", "http://example.com/api/teams", body);
        let response = crate::management::api::http::routes::routes(state(pool.clone()))
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/teams")
                    .header("host", "example.com")
                    .header("content-type", "application/json")
                    .header(AUTHORIZATION_HEADER, &header)
                    .body(Body::from(&body[..]))
                    .unwrap(),
            )
            .await
            .unwrap();
        let reply = decrypt_reply(response, &keys).await;
        assert_eq!(reply.status, StatusCode::CREATED.as_u16());
        let replay = crate::management::api::http::routes::routes(state(pool.clone()))
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/teams")
                    .header("host", "example.com")
                    .header("content-type", "application/json")
                    .header(AUTHORIZATION_HEADER, header)
                    .body(Body::from(&body[..]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::CONFLICT);
        let team_count: i64 = query_scalar("SELECT COUNT(*) FROM teams WHERE name = 'Ops'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(team_count, 1);
        env::remove_var("ALLOWED_PUBKEYS");
    }

    #[tokio::test]
    async fn post_team_route_rejects_tampered_payload() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pool = setup_route_test_db().await;
        let keys = Keys::generate();
        env::set_var("ALLOWED_PUBKEYS", keys.public_key().to_hex());

        let signed_body = br#"{"name":"Ops"}"#;
        let sent_body = br#"{"name":"Tampered"}"#;
        let response = crate::management::api::http::routes::routes(state(pool.clone()))
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/teams")
                    .header("host", "example.com")
                    .header("content-type", "application/json")
                    .header(
                        AUTHORIZATION_HEADER,
                        auth_header(&keys, "POST", "http://example.com/api/teams", signed_body),
                    )
                    .body(Body::from(&sent_body[..]))
                    .unwrap(),
            )
            .await
            .unwrap();
        env::remove_var("ALLOWED_PUBKEYS");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let team_count: i64 = query_scalar("SELECT COUNT(*) FROM teams")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(team_count, 0);
    }

    #[tokio::test]
    async fn team_member_cannot_add_a_private_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pool = setup_route_test_db().await;
        let member = Keys::generate();
        env::set_var("ALLOWED_PUBKEYS", member.public_key().to_hex());
        query("INSERT INTO users(public_key) VALUES (?)")
            .bind(member.public_key().to_hex())
            .execute(&pool)
            .await
            .unwrap();
        let team_id: i64 = query_scalar("INSERT INTO teams(name) VALUES ('Family') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
        query("INSERT INTO team_members(team_id, user_public_key, role) VALUES (?, ?, 'member')")
            .bind(team_id)
            .bind(member.public_key().to_hex())
            .execute(&pool)
            .await
            .unwrap();

        let body = br#"{"name":"Sensitive","secret_key":"not-reached"}"#;
        let endpoint = format!("/teams/{team_id}/keys");
        let response = crate::management::api::http::routes::routes(state(pool.clone()))
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&endpoint)
                    .header("host", "example.com")
                    .header("content-type", "application/json")
                    .header(
                        AUTHORIZATION_HEADER,
                        auth_header(
                            &member,
                            "POST",
                            &format!("http://example.com/api{endpoint}"),
                            body,
                        ),
                    )
                    .body(Body::from(&body[..]))
                    .unwrap(),
            )
            .await
            .unwrap();
        env::remove_var("ALLOWED_PUBKEYS");

        let reply = decrypt_reply(response, &member).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN.as_u16());
        let keys: i64 = query_scalar("SELECT count(*) FROM stored_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(keys, 0);
    }
    #[tokio::test]
    async fn delegated_http_auth_cannot_mutate_and_nonoperator_cannot_change_relays() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pool = setup_route_test_db().await;
        let keys = Keys::generate();
        env::set_var("ALLOWED_PUBKEYS", keys.public_key().to_hex());
        env::remove_var("KEYCAST_OPERATOR_PUBKEYS");
        env::remove_var("KEYCAST_PUBLIC_URL");
        let body = br#"{"name":"Escalate"}"#;
        let event = EventBuilder::new(Kind::HttpAuth, "")
            .tags([
                Tag::parse(["u", "http://example.com/api/teams"]).unwrap(),
                Tag::parse(["method", "POST"]).unwrap(),
                Tag::parse(["payload", &hex_digest(body)]).unwrap(),
            ])
            .finalize(&keys)
            .unwrap();
        let header = format!("Nostr {}", BASE64.encode(event.as_json()));
        let response = routes::routes(state(pool.clone()))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/teams")
                    .header("host", "example.com")
                    .header("content-type", "application/json")
                    .header(AUTHORIZATION_HEADER, header)
                    .body(Body::from(&body[..]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body=br#"{"minimum_connected_relays":1,"relays":[{"url":"wss://example.com","enabled":true}]}"#;
        let header = auth_header(&keys, "PUT", "http://example.com/api/relays", body);
        let response = routes::routes(state(pool.clone()))
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/relays")
                    .header("host", "example.com")
                    .header("content-type", "application/json")
                    .header(AUTHORIZATION_HEADER, header)
                    .body(Body::from(&body[..]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(decrypt_reply(response, &keys).await.status, 403);
        env::remove_var("ALLOWED_PUBKEYS");
    }
    include!("hardening_tests.rs");
}
