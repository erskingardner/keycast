pub mod routes;
pub mod teams;

use axum::{
    body::{to_bytes, Body, Bytes},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
pub use routes::*;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use nostr_sdk::prelude::*;
use serde::Serialize;
use std::env;
use thiserror::Error;

/// Common HTTP authentication header names
pub const AUTHORIZATION_HEADER: &str = "Authorization";
const AUTH_EVENT_MAX_AGE_SECONDS: i64 = 60;
const AUTH_EVENT_MAX_FUTURE_SKEW_SECONDS: i64 = 60;
const MAX_AUTH_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicConfig {
    pub allowed_pubkeys: Vec<String>,
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

pub async fn auth_middleware(request: Request<Body>, next: Next) -> Response {
    let (parts, body) = request.into_parts();

    let body_bytes = match to_bytes(body, MAX_AUTH_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::debug!("Failed to read request body for auth validation: {}", e);
            return response_with_status(StatusCode::PAYLOAD_TOO_LARGE, "Request body too large");
        }
    };

    let request = Request::from_parts(parts, Body::from(body_bytes.clone()));

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
    let event = match validate_token(auth_str, &request, &body_bytes) {
        Ok(event) => event,
        Err(e) => {
            tracing::debug!("Token validation failed: {}", e);
            return response_with_status(StatusCode::UNAUTHORIZED, "Invalid credentials");
        }
    };

    if !is_allowed_pubkey(&event.pubkey) {
        tracing::debug!("Token validation failed: pubkey not allowed");
        return response_with_status(StatusCode::FORBIDDEN, "Forbidden");
    };

    next.run(request).await
}

pub async fn public_config() -> Json<PublicConfig> {
    Json(PublicConfig {
        allowed_pubkeys: configured_allowed_pubkeys(),
    })
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
) -> Result<Event, AuthenticationError> {
    // Check prefix
    if !token.starts_with("Nostr ") {
        return Err(AuthenticationError::InvalidEvent(
            "Invalid token prefix".to_string(),
        ));
    }

    let event = extract_auth_event_from_header(token)?;

    validate_auth_event(&event, request, body)?;
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
            tracing::debug!(
                "Token validation failed: Invalid NIP-98 event json: {:#?}",
                json_str
            );
            return Err(AuthenticationError::InvalidJson);
        }
    };

    Ok(event)
}

pub fn validate_auth_event(
    event: &Event,
    request: &Request<Body>,
    body: &[u8],
) -> Result<(), AuthenticationError> {
    if event.verify().is_err() {
        tracing::debug!("Token validation failed: Event verification failed");
        return Err(AuthenticationError::InvalidEvent(
            "Event verification failed".to_string(),
        ));
    }

    if event.kind != Kind::HttpAuth {
        tracing::debug!("Token validation failed: Invalid event kind");
        return Err(AuthenticationError::InvalidEvent(
            "Event kind is not HttpAuth".to_string(),
        ));
    }

    let now = chrono::Utc::now().timestamp();
    let created_at = event.created_at.as_secs() as i64;
    if created_at < now - AUTH_EVENT_MAX_AGE_SECONDS {
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

    let u_tag = required_tag_content(
        event,
        TagKind::SingleLetter(SingleLetterTag::lowercase(nostr_sdk::Alphabet::U)),
        "u",
    )?;
    let full_url = full_request_url(request);

    if u_tag != full_url {
        tracing::debug!("Token validation failed: Invalid u tag");
        return Err(AuthenticationError::InvalidEvent(
            "Invalid u tag".to_string(),
        ));
    }

    let method_tag = required_tag_content(event, TagKind::Method, "method")?;
    if method_tag != request.method().as_str() {
        tracing::debug!("Token validation failed: Invalid method tag");
        return Err(AuthenticationError::InvalidEvent(
            "Invalid method tag".to_string(),
        ));
    }

    validate_payload_tag(event, body)?;

    Ok(())
}

fn full_request_url(request: &Request<Body>) -> String {
    let host = request
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");

    let scheme = if request
        .headers()
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .map(|p| p == "https")
        .unwrap_or(false)
    {
        "https"
    } else {
        "http"
    };

    // The API router is nested under /api, so nested route handlers see stripped paths.
    let prefix = request
        .headers()
        .get("x-forwarded-prefix")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("/api");

    format!("{}://{}{}{}", scheme, host, prefix, request.uri())
}

fn required_tag_content<'a>(
    event: &'a Event,
    kind: TagKind<'static>,
    name: &str,
) -> Result<&'a str, AuthenticationError> {
    match tag_contents(event, kind).as_slice() {
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
    kind: TagKind<'static>,
    name: &str,
) -> Result<Option<&'a str>, AuthenticationError> {
    match tag_contents(event, kind).as_slice() {
        [value] => Ok(Some(*value)),
        [] => Ok(None),
        _ => Err(AuthenticationError::InvalidEvent(format!(
            "Duplicate {name} tags"
        ))),
    }
}

fn tag_contents<'a>(event: &'a Event, kind: TagKind<'static>) -> Vec<&'a str> {
    event
        .tags
        .iter()
        .filter(|tag| tag.kind() == kind)
        .filter_map(|tag| tag.content())
        .collect()
}

fn validate_payload_tag(event: &Event, body: &[u8]) -> Result<(), AuthenticationError> {
    let payload_tag = optional_tag_content(event, TagKind::Payload, "payload")?;
    if body.is_empty() && payload_tag.is_none() {
        return Ok(());
    }

    let expected = sha256::digest(body);
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
    let allowed_pubkeys = configured_allowed_pubkeys();
    if allowed_pubkeys.is_empty() {
        return true;
    }

    let pubkey_hex = pubkey.to_hex();
    allowed_pubkeys
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(&pubkey_hex))
}

fn configured_allowed_pubkeys() -> Vec<String> {
    let allowed_pubkeys = env::var("ALLOWED_PUBKEYS")
        .or_else(|_| env::var("VITE_ALLOWED_PUBKEYS"))
        .unwrap_or_default();

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
    use super::*;
    use axum::http::Method;
    use sqlx::{query::query, query_scalar::query_scalar, raw_sql::raw_sql};
    use sqlx_sqlite::{SqlitePool, SqlitePoolOptions};
    use std::sync::Mutex;
    use tower::ServiceExt;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn request(method: Method, uri: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "example.com")
            .header("x-forwarded-proto", "https")
            .body(Body::empty())
            .unwrap()
    }

    fn auth_event(tags: Vec<Tag>, created_at: i64) -> Event {
        EventBuilder::new(Kind::HttpAuth, "")
            .tags(tags)
            .custom_created_at(Timestamp::from(created_at as u64))
            .sign_with_keys(&Keys::generate())
            .unwrap()
    }

    fn standard_tags(method: &str, url: &str) -> Vec<Tag> {
        vec![
            Tag::custom(
                TagKind::SingleLetter(SingleLetterTag::lowercase(nostr_sdk::Alphabet::U)),
                [url],
            ),
            Tag::custom(TagKind::Method, [method]),
        ]
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
        raw_sql(include_str!(
            "../../../../database/migrations/0001_initial.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    fn auth_header(keys: &Keys, method: &str, url: &str, body: &[u8]) -> String {
        let mut tags = standard_tags(method, url);
        if !body.is_empty() {
            tags.push(Tag::custom(TagKind::Payload, [sha256::digest(body)]));
        }
        let event = EventBuilder::new(Kind::HttpAuth, "")
            .tags(tags)
            .sign_with_keys(keys)
            .unwrap();

        format!("Nostr {}", BASE64.encode(event.as_json()))
    }

    #[test]
    fn validates_required_url_and_method_tags() {
        let req = request(Method::GET, "/teams");
        let now = chrono::Utc::now().timestamp();
        let event = auth_event(standard_tags("GET", "https://example.com/api/teams"), now);

        assert!(validate_auth_event(&event, &req, &[]).is_ok());
    }

    #[test]
    fn rejects_missing_url_or_method_tags() {
        let req = request(Method::GET, "/teams");
        let now = chrono::Utc::now().timestamp();

        let missing_url = auth_event(vec![Tag::custom(TagKind::Method, ["GET"])], now);
        assert!(validate_auth_event(&missing_url, &req, &[]).is_err());

        let missing_method = auth_event(
            vec![Tag::custom(
                TagKind::SingleLetter(SingleLetterTag::lowercase(nostr_sdk::Alphabet::U)),
                ["https://example.com/api/teams"],
            )],
            now,
        );
        assert!(validate_auth_event(&missing_method, &req, &[]).is_err());
    }

    #[test]
    fn rejects_future_timestamps() {
        let req = request(Method::GET, "/teams");
        let future = chrono::Utc::now().timestamp() + AUTH_EVENT_MAX_FUTURE_SKEW_SECONDS + 1;
        let event = auth_event(
            standard_tags("GET", "https://example.com/api/teams"),
            future,
        );

        assert!(validate_auth_event(&event, &req, &[]).is_err());
    }

    #[test]
    fn validates_payload_hash_for_body_requests() {
        let req = request(Method::POST, "/teams");
        let body = br#"{"name":"ops"}"#;
        let mut tags = standard_tags("POST", "https://example.com/api/teams");
        let digest = sha256::digest(body);
        tags.push(Tag::custom(TagKind::Payload, [digest]));
        let event = auth_event(tags, chrono::Utc::now().timestamp());

        assert!(validate_auth_event(&event, &req, body).is_ok());
    }

    #[test]
    fn rejects_missing_or_mismatched_payload_hashes() {
        let req = request(Method::POST, "/teams");
        let body = br#"{"name":"ops"}"#;
        let now = chrono::Utc::now().timestamp();

        let missing_payload =
            auth_event(standard_tags("POST", "https://example.com/api/teams"), now);
        assert!(validate_auth_event(&missing_payload, &req, body).is_err());

        let mut tags = standard_tags("POST", "https://example.com/api/teams");
        tags.push(Tag::custom(TagKind::Payload, ["not-a-real-hash"]));
        let mismatched_payload = auth_event(tags, now);
        assert!(validate_auth_event(&mismatched_payload, &req, body).is_err());
    }

    #[test]
    fn configured_allowlist_trims_blank_entries() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("ALLOWED_PUBKEYS", " abc, def ,, ghi ");

        assert_eq!(configured_allowed_pubkeys(), vec!["abc", "def", "ghi"]);

        env::remove_var("ALLOWED_PUBKEYS");
    }

    #[tokio::test]
    async fn public_config_exposes_allowlist_without_auth() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pool = setup_route_test_db().await;
        env::set_var("ALLOWED_PUBKEYS", "abc,def");

        let response = crate::api::http::routes::routes(pool)
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/config")
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
        assert_eq!(config["allowed_pubkeys"], serde_json::json!(["abc", "def"]));
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

    #[tokio::test]
    async fn post_team_route_accepts_valid_nip98_payload() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pool = setup_route_test_db().await;
        let keys = Keys::generate();
        env::set_var("ALLOWED_PUBKEYS", keys.public_key().to_hex());

        let body = br#"{"name":"Ops"}"#;
        let response = crate::api::http::routes::routes(pool.clone())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/teams")
                    .header("host", "example.com")
                    .header("x-forwarded-proto", "https")
                    .header("content-type", "application/json")
                    .header(
                        AUTHORIZATION_HEADER,
                        auth_header(&keys, "POST", "https://example.com/api/teams", body),
                    )
                    .body(Body::from(&body[..]))
                    .unwrap(),
            )
            .await
            .unwrap();
        env::remove_var("ALLOWED_PUBKEYS");

        assert_eq!(response.status(), StatusCode::OK);
        let team_count: i64 = query_scalar("SELECT COUNT(*) FROM teams WHERE name = 'Ops'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(team_count, 1);
    }

    #[tokio::test]
    async fn post_team_route_rejects_tampered_payload() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pool = setup_route_test_db().await;
        let keys = Keys::generate();
        env::set_var("ALLOWED_PUBKEYS", keys.public_key().to_hex());

        let signed_body = br#"{"name":"Ops"}"#;
        let sent_body = br#"{"name":"Tampered"}"#;
        let response = crate::api::http::routes::routes(pool.clone())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/teams")
                    .header("host", "example.com")
                    .header("x-forwarded-proto", "https")
                    .header("content-type", "application/json")
                    .header(
                        AUTHORIZATION_HEADER,
                        auth_header(&keys, "POST", "https://example.com/api/teams", signed_body),
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
}
