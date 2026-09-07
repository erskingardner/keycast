use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use keycast_core::v2::policy::{PolicyDocument, RequestedCapabilities};
use nostr::nips::{nip04, nip44};
use nostr::prelude::{
    Event, EventBuilder, FinalizeEvent, Keys, Kind, PublicKey, Tag, Timestamp, UnsignedEvent,
};
use nostr_sdk::prelude::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::store::{RuntimeGrant, Store, StoreError};

const MAX_EVENT_CONTENT_BYTES: usize = 256 * 1024;
const MAX_REQUEST_ID_BYTES: usize = 256;
const MAX_METHOD_BYTES: usize = 64;
const MAX_PARAMS: usize = 4;
const MAX_REQUEST_AGE_SECONDS: u64 = 300;
const MAX_FUTURE_SKEW_SECONDS: u64 = 60;

#[derive(Default)]
pub struct RuntimeMetrics {
    pub ingress_rejections: AtomicU64,
    pub parse_errors: AtomicU64,
    pub denied_requests: AtomicU64,
    pub relay_failures: AtomicU64,
}

#[derive(Clone)]
pub struct RequestProcessor {
    store: Store,
    metrics: Arc<RuntimeMetrics>,
    replications: Arc<std::sync::Mutex<tokio::task::JoinSet<()>>>,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("request is not a valid NIP-46 event")]
    InvalidEvent,
    #[error("request event is too large")]
    EventTooLarge,
    #[error("request has an invalid recipient")]
    InvalidRecipient,
    #[error("no active grant matches the recipient")]
    UnknownGrant,
    #[error("request could not be decrypted")]
    Decrypt,
    #[error("request is malformed")]
    Malformed,
    #[error("store operation failed: {0}")]
    Store(#[from] StoreError),
    #[error("request storage capacity exceeded")]
    Capacity(Box<Event>),
    #[error("failed to build response")]
    Response,
    #[error("response exceeds the event size limit")]
    ResponseTooLarge,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestWire {
    id: String,
    method: String,
    params: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ResponseWire<'a> {
    id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

struct Execution {
    result: Option<String>,
    error: Option<String>,
    approved: bool,
    reason: Option<&'static str>,
    session_id: Option<i64>,
    logout_after_publish: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignEventTemplateError {
    Invalid,
    PubkeyMismatch,
}

impl Execution {
    fn success(result: String, session_id: Option<i64>) -> Self {
        Self {
            result: Some(result),
            error: None,
            approved: true,
            reason: None,
            session_id,
            logout_after_publish: false,
        }
    }

    fn denied(error: &'static str, reason: &'static str, session_id: Option<i64>) -> Self {
        Self {
            result: None,
            error: Some(error.to_string()),
            approved: false,
            reason: Some(reason),
            session_id,
            logout_after_publish: false,
        }
    }
}

impl RequestProcessor {
    pub fn new(store: Store, metrics: Arc<RuntimeMetrics>) -> Self {
        Self {
            store,
            metrics,
            replications: Arc::default(),
        }
    }

    pub async fn process_and_publish(
        &self,
        client: &Client,
        event: &Event,
    ) -> Result<(), ProtocolError> {
        let response = match self.prepare_response(event).await {
            Err(ProtocolError::Capacity(response)) => {
                return self.publish_response(client, &response).await
            }
            result => result?,
        };
        if response.is_some() {
            let published = self
                .publish_cached_response(client, &event.id.to_hex())
                .await
                .is_ok();
            self.store
                .mark_published(&event.id.to_hex(), published)
                .await?;
            if !published {
                return Err(ProtocolError::Response);
            }
        }
        Ok(())
    }
    pub async fn prepare_response(&self, event: &Event) -> Result<Option<Event>, ProtocolError> {
        validate_outer_event(event)?;
        let remote_pubkey = recipient(event)?;
        let _authority = self.store.authority.lock().await;
        let grant = self
            .store
            .grant_for_recipient(&remote_pubkey.to_hex())
            .await?
            .ok_or(ProtocolError::UnknownGrant)?;
        let remote_keys = self.store.decrypt_remote_keys(&grant)?;
        let plaintext = nip44::decrypt(remote_keys.secret_key(), &event.pubkey, &event.content)
            .map_err(|_| ProtocolError::Decrypt)?;
        if plaintext.len() > MAX_EVENT_CONTENT_BYTES {
            return Err(ProtocolError::EventTooLarge);
        }
        let request: RequestWire = serde_json::from_str(&plaintext).map_err(|_| {
            self.metrics.parse_errors.fetch_add(1, Ordering::Relaxed);
            ProtocolError::Malformed
        })?;
        validate_request(&request)?;

        let current_session = if request.method == "connect" {
            None
        } else {
            self.store.active_session(grant.id, &event.pubkey).await?
        };

        if request.method == "connect"
            && (request.params.len() < 2
                || !self
                    .store
                    .can_connect(grant.id, &event.pubkey, &request.params[1])
                    .await?)
        {
            return Err(ProtocolError::Malformed);
        }

        if let Some(cached) = self
            .store
            .cached_session_response(&event.id.to_hex())
            .await?
        {
            self.store.retry_cached(&event.id.to_hex()).await?;
            return Ok(Some(
                Event::from_json(cached).map_err(|_| ProtocolError::Response)?,
            ));
        }

        if request.method != "connect" && current_session.is_none() {
            return Err(ProtocolError::UnknownGrant);
        }

        let admitted = self
            .store
            .begin_request(
                &event.id.to_hex(),
                &grant,
                current_session.as_ref().map(|session| session.id),
                &event.pubkey,
                &request.id,
                &request.method,
                &event.as_json(),
            )
            .await;
        match admitted {
            Ok(true) => {}
            Ok(false) => return Ok(None),
            Err(StoreError::Capacity) => {
                self.metrics
                    .ingress_rejections
                    .fetch_add(1, Ordering::Relaxed);
                return Err(ProtocolError::Capacity(Box::new(build_response(
                    &remote_keys,
                    &event.pubkey,
                    &request.id,
                    None,
                    Some("request capacity exceeded; retry later"),
                )?)));
            }
            Err(error) => return Err(error.into()),
        }

        let execution = self
            .execute(
                &grant,
                &remote_keys,
                &event.pubkey,
                &request,
                current_session,
            )
            .await;
        let mut execution = match execution {
            Ok(execution) => execution,
            Err(error @ StoreError::Database(_)) => return Err(error.into()),
            Err(error) => {
                tracing::warn!(
                    grant_id = grant.id,
                    method = %request.method,
                    error = %error,
                    "NIP-46 request failed safely"
                );
                Execution::denied("request failed", "operation_failed", None)
            }
        };

        let response = match build_response(
            &remote_keys,
            &event.pubkey,
            &request.id,
            execution.result.as_deref(),
            execution.error.as_deref(),
        ) {
            Ok(response) => response,
            Err(ProtocolError::ResponseTooLarge) => {
                execution = Execution::denied(
                    "response exceeds the size limit",
                    "response_too_large",
                    execution.session_id,
                );
                build_response(
                    &remote_keys,
                    &event.pubkey,
                    &request.id,
                    None,
                    execution.error.as_deref(),
                )?
            }
            Err(error) => return Err(error),
        };
        if !execution.approved {
            self.metrics.denied_requests.fetch_add(1, Ordering::Relaxed);
        }
        let response_json = response.as_json();
        self.store
            .cache_response(
                &event.id.to_hex(),
                execution.session_id,
                &response_json,
                execution.approved,
                execution.reason,
                execution.logout_after_publish,
            )
            .await?;
        Ok(Some(response))
    }

    async fn execute(
        &self,
        grant: &RuntimeGrant,
        remote_keys: &Keys,
        client: &PublicKey,
        request: &RequestWire,
        session: Option<crate::store::ActiveSession>,
    ) -> Result<Execution, StoreError> {
        if request.method == "connect" {
            return self.connect(grant, remote_keys, client, request).await;
        }

        let Some(session) = session else {
            return Ok(Execution::denied(
                "session required",
                "session_required",
                None,
            ));
        };
        self.store.touch_session(session.id).await?;

        match request.method.as_str() {
            "get_public_key" if request.params.is_empty() => Ok(Execution::success(
                grant.stored_public_key.clone(),
                Some(session.id),
            )),
            "ping" if request.params.is_empty() => {
                Ok(Execution::success("pong".to_string(), Some(session.id)))
            }
            "switch_relays" if request.params.is_empty() => {
                let relays = self.store.enabled_relays().await?;
                let result = serde_json::to_string(&relays)
                    .map_err(|e| StoreError::InvalidInput(e.to_string()))?;
                Ok(Execution::success(result, Some(session.id)))
            }
            "logout" if request.params.is_empty() => {
                let mut execution = Execution::success("ack".to_string(), Some(session.id));
                execution.logout_after_publish = true;
                Ok(execution)
            }
            "sign_event" => self.sign_event(grant, request, &session).await,
            "nip04_encrypt" | "nip04_decrypt" | "nip44_encrypt" | "nip44_decrypt" => {
                self.crypto(grant, request, &session).await
            }
            "get_public_key" | "ping" | "switch_relays" | "logout" => Ok(Execution::denied(
                "invalid parameters",
                "invalid_parameters",
                Some(session.id),
            )),
            _ => Ok(Execution::denied(
                "unsupported method",
                "unsupported_method",
                Some(session.id),
            )),
        }
    }

    async fn connect(
        &self,
        grant: &RuntimeGrant,
        remote_keys: &Keys,
        client: &PublicKey,
        request: &RequestWire,
    ) -> Result<Execution, StoreError> {
        if !(2..=4).contains(&request.params.len())
            || request.params[0] != remote_keys.public_key().to_hex()
            || request.params[1].is_empty()
        {
            return Ok(Execution::denied(
                "invalid connection request",
                "invalid_connect",
                None,
            ));
        }
        let requested =
            match RequestedCapabilities::parse(request.params.get(2).map(String::as_str)) {
                Ok(requested) => requested,
                Err(_) => {
                    return Ok(Execution::denied(
                        "invalid requested permissions",
                        "invalid_requested_capabilities",
                        None,
                    ))
                }
            };
        let metadata = request.params.get(3).map(String::as_str);
        match self
            .store
            .claim_invitation(grant, client, &request.params[1], &requested, metadata)
            .await
        {
            Ok(session_id) => Ok(Execution::success("ack".to_string(), Some(session_id))),
            Err(StoreError::InvitationNotClaimable) => Ok(Execution::denied(
                "connection rejected",
                "invitation_not_claimable",
                None,
            )),
            Err(error) => Err(error),
        }
    }

    async fn sign_event(
        &self,
        grant: &RuntimeGrant,
        request: &RequestWire,
        session: &crate::store::ActiveSession,
    ) -> Result<Execution, StoreError> {
        if request.params.len() != 1 || request.params[0].len() > MAX_EVENT_CONTENT_BYTES {
            return Ok(Execution::denied(
                "invalid parameters",
                "invalid_parameters",
                Some(session.id),
            ));
        }
        let user_pubkey = PublicKey::from_hex(&grant.stored_public_key)?;
        let unsigned = match parse_sign_event_template(&request.params[0], &user_pubkey) {
            Ok(unsigned) => unsigned,
            Err(SignEventTemplateError::Invalid) => {
                return Ok(Execution::denied(
                    "invalid event",
                    "invalid_event",
                    Some(session.id),
                ))
            }
            Err(SignEventTemplateError::PubkeyMismatch) => {
                return Ok(Execution::denied(
                    "event pubkey mismatch",
                    "pubkey_mismatch",
                    Some(session.id),
                ))
            }
        };
        let policy = match PolicyDocument::parse(&grant.policy_document) {
            Ok(policy) => policy,
            Err(_) => {
                return Ok(Execution::denied(
                    "policy is invalid",
                    "invalid_policy",
                    Some(session.id),
                ))
            }
        };
        let requested =
            RequestedCapabilities::from_json(session.requested_capabilities.as_deref())?;
        if !policy.allows_sign_event(unsigned.kind.as_u16(), &requested) {
            return Ok(Execution::denied(
                "policy denied request",
                "policy_denied",
                Some(session.id),
            ));
        }
        let keys = self.store.decrypt_stored_keys(grant)?;
        let signed = unsigned
            .finalize(&keys)
            .map_err(|e| StoreError::InvalidInput(e.to_string()))?;
        Ok(Execution::success(signed.as_json(), Some(session.id)))
    }

    async fn crypto(
        &self,
        grant: &RuntimeGrant,
        request: &RequestWire,
        session: &crate::store::ActiveSession,
    ) -> Result<Execution, StoreError> {
        if request.params.len() != 2 || request.params[1].len() > MAX_EVENT_CONTENT_BYTES {
            return Ok(Execution::denied(
                "invalid parameters",
                "invalid_parameters",
                Some(session.id),
            ));
        }
        let recipient = match PublicKey::from_hex(&request.params[0]) {
            Ok(recipient) => recipient,
            Err(_) => {
                return Ok(Execution::denied(
                    "invalid public key",
                    "invalid_parameters",
                    Some(session.id),
                ))
            }
        };
        let user_pubkey = PublicKey::from_hex(&grant.stored_public_key)?;
        let policy = match PolicyDocument::parse(&grant.policy_document) {
            Ok(policy) => policy,
            Err(_) => {
                return Ok(Execution::denied(
                    "policy is invalid",
                    "invalid_policy",
                    Some(session.id),
                ))
            }
        };
        let requested =
            RequestedCapabilities::from_json(session.requested_capabilities.as_deref())?;
        if !policy.allows_crypto(
            request.method.as_str(),
            &recipient,
            &user_pubkey,
            &requested,
        ) {
            return Ok(Execution::denied(
                "policy denied request",
                "policy_denied",
                Some(session.id),
            ));
        }
        let keys = self.store.decrypt_stored_keys(grant)?;
        let result = match request.method.as_str() {
            "nip04_encrypt" => nip04::encrypt(keys.secret_key(), &recipient, &request.params[1]),
            "nip04_decrypt" => nip04::decrypt(keys.secret_key(), &recipient, &request.params[1]),
            "nip44_encrypt" => nip44::encrypt(
                keys.secret_key(),
                &recipient,
                &request.params[1],
                nip44::Version::default(),
            ),
            "nip44_decrypt" => nip44::decrypt(keys.secret_key(), &recipient, &request.params[1]),
            _ => unreachable!(),
        };
        match result {
            Ok(result) => Ok(Execution::success(result, Some(session.id))),
            Err(_) => Ok(Execution::denied(
                "cryptographic operation failed",
                "crypto_failed",
                Some(session.id),
            )),
        }
    }

    pub async fn shutdown_replication(&self) {
        let mut tasks =
            std::mem::take(&mut *self.replications.lock().unwrap_or_else(|e| e.into_inner()));
        if tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while tasks.join_next().await.is_some() {}
        })
        .await
        .is_err()
        {
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
        }
    }

    /// NIP-46 envelopes are ephemeral events. Refresh an old envelope before
    /// retrying it; the encrypted RPC result stays identical and the requested
    /// key operation is never executed again. Recheck authority before release.
    pub async fn publish_cached_response(
        &self,
        client: &Client,
        request_event_id: &str,
    ) -> Result<(), ProtocolError> {
        let response = {
            let _authority = self.store.authority.lock().await;
            let Some(json) = self.store.cached_session_response(request_event_id).await? else {
                return Ok(());
            };
            let mut response = Event::from_json(json).map_err(|_| ProtocolError::Response)?;
            let Some((remote, peer)) = self.store.response_context(request_event_id).await? else {
                return Ok(());
            };
            if response.kind != Kind::NostrConnect
                || response.pubkey.to_hex() != remote
                || recipient(&response)?.to_hex() != peer
            {
                return Err(ProtocolError::Response);
            }
            let Some(grant) = self.store.grant_for_recipient(&remote).await? else {
                return Ok(());
            };
            if Timestamp::now()
                .as_secs()
                .saturating_sub(response.created_at.as_secs())
                >= 30
            {
                response.verify().map_err(|_| ProtocolError::Response)?;
                let remote_keys = self.store.decrypt_remote_keys(&grant)?;
                response = EventBuilder::new(Kind::NostrConnect, response.content)
                    .tags(response.tags)
                    .finalize(&remote_keys)
                    .map_err(|_| ProtocolError::Response)?;
                self.store
                    .refresh_response_envelope(request_event_id, &response.as_json())
                    .await?;
            }
            response
        };
        self.publish_response(client, &response).await
    }

    /// Network waits never hold the authority lock. The durable outbox owns retries.
    pub async fn publish_response(
        &self,
        client: &Client,
        response: &Event,
    ) -> Result<(), ProtocolError> {
        let mut sends = tokio::task::JoinSet::new();
        for (url, relay) in client.relays().await {
            let response = response.clone();
            sends.spawn(async move {
                let result = tokio::time::timeout(std::time::Duration::from_secs(3), async {
                    relay
                        .send_event(&response)
                        .ok_timeout(std::time::Duration::from_secs(2))
                        .await
                })
                .await;
                (
                    url,
                    matches!(result, Ok(Ok(ref output)) if output.status().is_ack()),
                )
            });
        }
        while let Some(result) = sends.join_next().await {
            if let Ok((url, true)) = result {
                // Keep redundant delivery off the first-ACK path, bounded and owned by this
                // processor. Dropping the last processor aborts outstanding replication tasks.
                {
                    let mut replications =
                        self.replications.lock().unwrap_or_else(|e| e.into_inner());
                    while replications.try_join_next().is_some() {}
                    if replications.len() < 16 && !sends.is_empty() {
                        let store = self.store.clone();
                        replications.spawn(async move {
                            while let Some(result) = sends.join_next().await {
                                if let Ok((url, true)) = result {
                                    let _ = store.mark_relay_published(url.as_str()).await;
                                }
                            }
                        });
                    }
                }
                self.store.mark_relay_published(url.as_str()).await?;
                return Ok(());
            }
        }
        self.metrics.relay_failures.fetch_add(1, Ordering::Relaxed);
        Err(ProtocolError::Response)
    }
}

fn validate_outer_event(event: &Event) -> Result<(), ProtocolError> {
    if event.kind != Kind::NostrConnect
        || event.content.len() > MAX_EVENT_CONTENT_BYTES
        || event.as_json().len() > MAX_EVENT_CONTENT_BYTES + 4096
    {
        return Err(ProtocolError::EventTooLarge);
    }
    event.verify().map_err(|_| ProtocolError::InvalidEvent)?;
    let now = Timestamp::now().as_secs();
    let created_at = event.created_at.as_secs();
    if created_at.saturating_add(MAX_REQUEST_AGE_SECONDS) < now
        || created_at > now.saturating_add(MAX_FUTURE_SKEW_SECONDS)
    {
        return Err(ProtocolError::InvalidEvent);
    }
    Ok(())
}

pub(crate) fn recipient(event: &Event) -> Result<PublicKey, ProtocolError> {
    let mut tags = event.tags.iter().filter(|tag| tag.kind() == "p");
    let tag = tags.next().ok_or(ProtocolError::InvalidRecipient)?;
    if tags.next().is_some() || tag.as_slice().len() != 2 {
        return Err(ProtocolError::InvalidRecipient);
    }
    PublicKey::from_hex(tag.content().ok_or(ProtocolError::InvalidRecipient)?)
        .map_err(|_| ProtocolError::InvalidRecipient)
}

fn validate_request(request: &RequestWire) -> Result<(), ProtocolError> {
    if request.id.is_empty()
        || request.id.len() > MAX_REQUEST_ID_BYTES
        || request.method.is_empty()
        || request.method.len() > MAX_METHOD_BYTES
        || request.params.len() > MAX_PARAMS
        || request
            .params
            .iter()
            .any(|parameter| parameter.len() > MAX_EVENT_CONTENT_BYTES)
    {
        return Err(ProtocolError::Malformed);
    }
    Ok(())
}

fn parse_sign_event_template(
    serialized: &str,
    expected_pubkey: &PublicKey,
) -> Result<UnsignedEvent, SignEventTemplateError> {
    let mut value: Value =
        serde_json::from_str(serialized).map_err(|_| SignEventTemplateError::Invalid)?;
    let object = value
        .as_object_mut()
        .ok_or(SignEventTemplateError::Invalid)?;

    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "id" | "pubkey" | "created_at" | "kind" | "tags" | "content" | "sig"
        )
    }) {
        return Err(SignEventTemplateError::Invalid);
    }

    // NIP-46 specifies only kind/content/tags/created_at. Some clients, including
    // nak, serialize their full Event type before signing, which adds zero-value
    // id, pubkey, and sig fields. Normalize that representation without trusting
    // client-supplied signing metadata.
    if let Some(signature) = object.remove("sig") {
        let is_placeholder = signature.as_str().is_some_and(|signature| {
            signature.len() == 128 && signature.bytes().all(|b| b == b'0')
        });
        if !is_placeholder {
            return Err(SignEventTemplateError::Invalid);
        }
    }
    if object
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| id.len() == 64 && id.bytes().all(|b| b == b'0'))
    {
        object.remove("id");
    }

    match object.get("pubkey") {
        None => {
            object.insert(
                "pubkey".to_string(),
                Value::String(expected_pubkey.to_hex()),
            );
        }
        Some(Value::String(pubkey))
            if pubkey.len() == 64 && pubkey.bytes().all(|byte| byte == b'0') =>
        {
            object.insert(
                "pubkey".to_string(),
                Value::String(expected_pubkey.to_hex()),
            );
        }
        Some(Value::String(pubkey)) => {
            let parsed =
                PublicKey::from_hex(pubkey).map_err(|_| SignEventTemplateError::Invalid)?;
            if parsed != *expected_pubkey {
                return Err(SignEventTemplateError::PubkeyMismatch);
            }
        }
        Some(_) => return Err(SignEventTemplateError::Invalid),
    }

    UnsignedEvent::from_json(value.to_string()).map_err(|_| SignEventTemplateError::Invalid)
}

fn build_response(
    remote_keys: &Keys,
    client: &PublicKey,
    request_id: &str,
    result: Option<&str>,
    error: Option<&str>,
) -> Result<Event, ProtocolError> {
    let message = serde_json::to_string(&ResponseWire {
        id: request_id,
        result,
        error,
    })
    .map_err(|_| ProtocolError::Response)?;
    let content = nip44::encrypt(
        remote_keys.secret_key(),
        client,
        message,
        nip44::Version::default(),
    )
    .map_err(|_| ProtocolError::Response)?;
    if content.len() > MAX_EVENT_CONTENT_BYTES {
        return Err(ProtocolError::ResponseTooLarge);
    }
    EventBuilder::new(Kind::NostrConnect, content)
        .tag(Tag::public_key(*client))
        .finalize(remote_keys)
        .map_err(|_| ProtocolError::Response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use keycast_core::v2::envelope::EnvelopeCipher;
    use serde_json::json;
    use sqlx_sqlite::SqlitePoolOptions;
    use zeroize::Zeroizing;

    #[tokio::test]
    async fn signing_without_an_active_session_is_denied_before_key_use() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect test database");
        let processor = RequestProcessor::new(
            Store::new(pool, EnvelopeCipher::from_key(Zeroizing::new([7_u8; 32]))),
            Arc::new(RuntimeMetrics::default()),
        );
        let remote_keys = Keys::generate();
        let user_keys = Keys::generate();
        let client = Keys::generate().public_key();
        let grant = RuntimeGrant {
            expires_at: None,
            id: 1,
            team_id: 1,
            stored_key_id: 1,
            remote_signer_public_key: remote_keys.public_key().to_hex(),
            remote_signer_secret_envelope: vec![],
            remote_envelope_version: 1,
            remote_key_id: String::new(),
            stored_public_key: user_keys.public_key().to_hex(),
            stored_secret_envelope: vec![],
            stored_envelope_version: 1,
            stored_key_id_tag: String::new(),
            policy_document:
                "{\"version\":1,\"capabilities\":{\"sign_event\":{\"allowed_kinds\":[1]}}}"
                    .to_string(),
        };
        let request = RequestWire {
            id: "request-1".to_string(),
            method: "sign_event".to_string(),
            params: vec!["{}".to_string()],
        };

        let execution = processor
            .execute(&grant, &remote_keys, &client, &request, None)
            .await
            .expect("safe denial");

        assert!(!execution.approved);
        assert_eq!(execution.reason, Some("session_required"));
        assert_eq!(execution.error.as_deref(), Some("session required"));
    }

    #[test]
    fn stale_and_future_outer_requests_are_rejected() {
        let keys = Keys::generate();
        let recipient = Keys::generate().public_key();
        let old = EventBuilder::new(Kind::NostrConnect, "ciphertext")
            .tag(Tag::public_key(recipient))
            .custom_created_at(Timestamp::from_secs(
                Timestamp::now().as_secs() - MAX_REQUEST_AGE_SECONDS - 1,
            ))
            .finalize(&keys)
            .expect("build old event");
        assert!(matches!(
            validate_outer_event(&old),
            Err(ProtocolError::InvalidEvent)
        ));

        let future = EventBuilder::new(Kind::NostrConnect, "ciphertext")
            .tag(Tag::public_key(recipient))
            .custom_created_at(Timestamp::from_secs(
                Timestamp::now().as_secs() + MAX_FUTURE_SKEW_SECONDS + 1,
            ))
            .finalize(&keys)
            .expect("build future event");
        assert!(matches!(
            validate_outer_event(&future),
            Err(ProtocolError::InvalidEvent)
        ));
    }

    #[test]
    fn sign_event_accepts_canonical_nip46_and_nak_templates() {
        let user = Keys::generate().public_key();
        let canonical = json!({
            "kind": 1,
            "content": "hello",
            "tags": [],
            "created_at": Timestamp::now().as_secs()
        });
        let parsed = parse_sign_event_template(&canonical.to_string(), &user)
            .expect("canonical NIP-46 template");
        assert_eq!(parsed.pubkey, user);

        let nak = json!({
            "id": "0".repeat(64),
            "pubkey": "0".repeat(64),
            "created_at": Timestamp::now().as_secs(),
            "kind": 1,
            "tags": [],
            "content": "hello",
            "sig": "0".repeat(128)
        });
        let parsed = parse_sign_event_template(&nak.to_string(), &user)
            .expect("nak unsigned full-event representation");
        assert_eq!(parsed.pubkey, user);
        assert_eq!(parsed.id, None);
    }

    #[test]
    fn sign_event_rejects_ambiguous_or_mismatched_templates() {
        let user = Keys::generate().public_key();
        let other = Keys::generate().public_key();
        let base = json!({
            "kind": 1,
            "content": "hello",
            "tags": [],
            "created_at": Timestamp::now().as_secs()
        });

        let mut mismatched = base.clone();
        mismatched["pubkey"] = Value::String(other.to_hex());
        assert_eq!(
            parse_sign_event_template(&mismatched.to_string(), &user),
            Err(SignEventTemplateError::PubkeyMismatch)
        );

        let mut unexpected = base.clone();
        unexpected["delegation"] = Value::String("not supported".to_string());
        assert_eq!(
            parse_sign_event_template(&unexpected.to_string(), &user),
            Err(SignEventTemplateError::Invalid)
        );

        let mut signed = base;
        signed["sig"] = Value::String("1".repeat(128));
        assert_eq!(
            parse_sign_event_template(&signed.to_string(), &user),
            Err(SignEventTemplateError::Invalid)
        );
    }
}
