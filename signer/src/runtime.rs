use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use keycast_core::database::Database;
use keycast_core::v2::control::SignerStatus;
use keycast_core::v2::{ENVELOPE_VERSION, SCHEMA_VERSION};
use nostr::prelude::{Filter, Kind, PublicKey, Timestamp};
use nostr_sdk::prelude::{Client, ClientNotification, StreamExt};
use thiserror::Error;
use tokio::sync::{watch, Notify};

use crate::admission::{Admission, RelayCopies};
use crate::control::serve_control_socket;
use crate::protocol::{recipient, RequestProcessor, RuntimeMetrics};
use crate::store::{Store, StoreError};

#[derive(Clone)]
pub struct RuntimeState {
    pub store: Store,
    pub metrics: Arc<RuntimeMetrics>,
    pub connected_relays: Arc<AtomicUsize>,
    pub relay_diagnostics: Arc<
        tokio::sync::RwLock<
            std::collections::BTreeMap<String, crate::relay_diagnostics::RelayDiagnostics>,
        >,
    >,
    pub ready: Arc<AtomicBool>,
    pub integrity_ok: Arc<AtomicBool>,
    pub quarantined_grants: Arc<AtomicUsize>,
    pub reload: Arc<Notify>,
    pub last_progress: Arc<AtomicU64>,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("control socket failed: {0}")]
    Control(#[from] std::io::Error),
    #[error("relay supervisor failed: {0}")]
    Relay(#[from] StoreError),
    #[error("{0} task stopped unexpectedly")]
    UnexpectedExit(&'static str),
    #[error("{0} task panicked: {1}")]
    TaskJoin(&'static str, String),
}

impl RuntimeState {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            metrics: Arc::new(RuntimeMetrics::default()),
            connected_relays: Arc::new(AtomicUsize::new(0)),
            relay_diagnostics: Default::default(),
            ready: Arc::new(AtomicBool::new(false)),
            integrity_ok: Arc::new(AtomicBool::new(true)),
            quarantined_grants: Arc::new(AtomicUsize::new(0)),
            reload: Arc::new(Notify::new()),
            last_progress: Arc::new(AtomicU64::new(progress_tick())),
        }
    }

    pub async fn status(&self) -> Result<SignerStatus, StoreError> {
        let (grants, sessions, invitations, relays, last_processed_at) =
            self.store.counts().await?;
        Ok(SignerStatus {
            resources: self.store.resources().await?,
            ready: self.ready.load(Ordering::Relaxed),
            quarantined_grants: self.quarantined_grants.load(Ordering::Relaxed),
            integrity_ok: self.integrity_ok.load(Ordering::Relaxed),
            recovery_pending: sqlx::query_scalar::query_scalar(
                "SELECT recovery_pending FROM instance_settings WHERE singleton=1",
            )
            .fetch_one(&self.store.pool)
            .await?,
            schema_version: SCHEMA_VERSION,
            envelope_version: ENVELOPE_VERSION,
            credential_key_id: Some(self.store.cipher.key_id().to_string()),
            active_grants: grants,
            active_sessions: sessions,
            claimable_invitations: invitations,
            enabled_relays: relays,
            connected_relays: self.connected_relays.load(Ordering::Relaxed),
            last_processed_at,
            ingress_rejections: self.metrics.ingress_rejections.load(Ordering::Relaxed),
            parse_errors: self.metrics.parse_errors.load(Ordering::Relaxed),
            denied_requests: self.metrics.denied_requests.load(Ordering::Relaxed),
            relay_failures: self.metrics.relay_failures.load(Ordering::Relaxed),
        })
    }
}

pub async fn run(
    database: Database,
    cipher: keycast_core::v2::envelope::EnvelopeCipher,
    socket_path: PathBuf,
    public_url: url::Url,
) -> Result<(), RuntimeError> {
    let state = RuntimeState::new(Store::new(database.pool.clone(), cipher));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let control_state = state.clone();
    let control_path = socket_path.clone();
    let control_shutdown = shutdown_rx.clone();
    let mut control = tokio::spawn(async move {
        serve_control_socket(control_state, control_path, public_url, control_shutdown).await
    });

    let relay_state = state.clone();
    let relay_shutdown = shutdown_rx.clone();
    let mut relay =
        tokio::spawn(async move { relay_supervisor(relay_state, relay_shutdown).await });

    enum Exit {
        Signal,
        Stalled,
        Control(Result<Result<(), std::io::Error>, tokio::task::JoinError>),
        Relay(Result<Result<(), StoreError>, tokio::task::JoinError>),
    }
    let exit = tokio::select! {
        _ = wait_for_shutdown_signal() => Exit::Signal,
        _ = async { loop { tokio::time::sleep(Duration::from_secs(5)).await; if progress_tick().saturating_sub(state.last_progress.load(Ordering::Relaxed))>45 { break; } } } => Exit::Stalled,
        result = &mut control => Exit::Control(result),
        result = &mut relay => Exit::Relay(result),
    };
    let _ = shutdown_tx.send(true);
    state.reload.notify_waiters();

    let outcome = tokio::time::timeout(Duration::from_secs(30), async {
        match exit {
            Exit::Stalled => {
                control.abort();
                relay.abort();
                let _ = (&mut control).await;
                let _ = (&mut relay).await;
                Err(RuntimeError::UnexpectedExit("signer progress watchdog"))
            }
            Exit::Signal => {
                control_result((&mut control).await).and(relay_result((&mut relay).await))
            }
            Exit::Control(result) => {
                let primary: Result<(), RuntimeError> = control_result(result)
                    .and_then(|_| Err(RuntimeError::UnexpectedExit("control socket")));
                primary.and(relay_result((&mut relay).await))
            }
            Exit::Relay(result) => {
                let primary: Result<(), RuntimeError> = relay_result(result)
                    .and_then(|_| Err(RuntimeError::UnexpectedExit("relay supervisor")));
                primary.and(control_result((&mut control).await))
            }
        }
    })
    .await;
    let outcome = match outcome {
        Ok(result) => result,
        Err(_) => {
            control.abort();
            relay.abort();
            Err(RuntimeError::UnexpectedExit("shutdown deadline"))
        }
    };
    if tokio::time::timeout(Duration::from_secs(10), database.pool.close())
        .await
        .is_err()
    {
        return Err(RuntimeError::UnexpectedExit("database close deadline"));
    }
    let _ = std::fs::remove_file(socket_path);
    outcome
}

fn control_result(
    result: Result<Result<(), std::io::Error>, tokio::task::JoinError>,
) -> Result<(), RuntimeError> {
    match result {
        Ok(result) => result.map_err(RuntimeError::Control),
        Err(error) => Err(RuntimeError::TaskJoin("control socket", error.to_string())),
    }
}

fn relay_result(
    result: Result<Result<(), StoreError>, tokio::task::JoinError>,
) -> Result<(), RuntimeError> {
    match result {
        Ok(result) => result.map_err(RuntimeError::Relay),
        Err(error) => Err(RuntimeError::TaskJoin(
            "relay supervisor",
            error.to_string(),
        )),
    }
}

/// Shared by the daemon and real-relay integration tests.
pub async fn relay_supervisor(
    state: RuntimeState,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), StoreError> {
    use nostr::prelude::{Event, RelayMessage, SubscriptionId};
    use std::collections::{BTreeSet, HashMap, HashSet};
    let telemetry = crate::relay_transport::RelayTelemetry::default();
    let client = Client::builder()
        .websocket_transport(crate::relay_transport::ObservedTransport(telemetry.clone()))
        .build();
    let mut telemetry_flush = tokio::time::interval(Duration::from_secs(1));
    let mut pending_telemetry = Vec::new();
    let mut notifications = client.notifications();
    let processor = RequestProcessor::new(state.store.clone(), state.metrics.clone());
    let mut workers = tokio::task::JoinSet::new();
    let mut worker_events = HashMap::new();
    let mut publishers = tokio::task::JoinSet::<(String, bool, bool)>::new();
    let mut publishing = HashSet::<String>::new();
    let mut inflight = HashMap::<String, std::time::Instant>::new();
    let mut relay_copies = RelayCopies::default();
    let mut accepted = HashSet::<String>::new();
    let mut subscription_retries = HashMap::<String, SubscriptionRetry>::new();
    let mut subscribed_keys = BTreeSet::<PublicKey>::new();
    let mut subscription = SubscriptionId::generate();
    let mut refresh = tokio::time::interval(Duration::from_secs(10));
    let mut retry = tokio::time::interval(Duration::from_millis(100));
    let mut prune = tokio::time::interval(Duration::from_secs(5));
    let mut maintenance = tokio::time::interval(Duration::from_secs(60));
    let mut integrity = tokio::time::interval(Duration::from_secs(3600));
    let admission = Admission::default();
    let mut minimum = 1i64;
    let mut recovery_pending = false;
    let mut delivery_failure: Option<std::time::Instant> = None;
    let mut storage_ok = true;
    let mut initialized = false;
    let result = async {
        loop {
            if *shutdown.borrow() { break; }
            let step: Result<bool, StoreError> = async {
            tokio::select! {
                notification = notifications.next() => {
                    let Some(notification) = notification else { return Err(StoreError::InvalidInput("relay notification worker stopped".into())); };
                    if let ClientNotification::Message { relay_url, message } = notification {
                        match *message {
                            RelayMessage::EndOfStoredEvents(id) if id.as_ref() == &subscription => {
                                if let Some(diagnostics) = state.relay_diagnostics.write().await.get_mut(relay_url.as_str().trim_end_matches('/')) {
                                    diagnostics.subscription("accepted", None, Timestamp::now().as_secs() as i64);
                                }
                                telemetry.record(relay_url.as_str(), "subscription_accepted");
                                accepted.insert(relay_url.to_string());
                                subscription_retries.remove(relay_url.as_str());
                                state.store.mark_relay_subscription(relay_url.as_str(),None).await?;
                            }
                            RelayMessage::Closed { subscription_id, message } if subscription_id.as_ref() == &subscription => {
                                accepted.remove(&relay_url.to_string());
                                let reason = crate::relay_diagnostics::rejection_reason(message.as_ref());
                                telemetry.record(relay_url.as_str(), if message.starts_with("auth-required:") { "error_subscription_auth" } else if message.starts_with("rate-limited:") { "error_subscription_rate" } else { "error_subscription_other" });
                                if let Some(diagnostics) = state.relay_diagnostics.write().await.get_mut(relay_url.as_str().trim_end_matches('/')) {
                                    diagnostics.subscription("rejected", Some(reason), Timestamp::now().as_secs() as i64);
                                }
                                state.store.mark_relay_subscription(relay_url.as_str(),Some(reason)).await?;
                                state.ready.store(false, Ordering::Relaxed);
                            }
                            RelayMessage::Auth { .. } => {
                                telemetry.record(relay_url.as_str(), "auth_required");
                                if let Some(diagnostics) = state.relay_diagnostics.write().await.get_mut(relay_url.as_str().trim_end_matches('/')) {
                                    diagnostics.log(Timestamp::now().as_secs() as i64, "info", "Relay requested NIP-42 authentication".into());
                                }
                            }
                            RelayMessage::Ok { status: false, message, .. } => {
                                telemetry.record(relay_url.as_str(), "error_publication");
                                if let Some(diagnostics) = state.relay_diagnostics.write().await.get_mut(relay_url.as_str().trim_end_matches('/')) {
                                    diagnostics.log(Timestamp::now().as_secs() as i64, "warning", format!("Publication rejected: {}", crate::relay_diagnostics::rejection_reason(message.as_ref())));
                                }
                            }
                            RelayMessage::Event { subscription_id, event } if subscription_id.as_ref() == &subscription => {
                                let event = event.into_owned();
                                let id = event.id.to_hex();
                                let peer = event.pubkey.to_hex();
                                if event.kind != Kind::NostrConnect { return Ok(true); }
                                let Ok(target) = recipient(&event) else { return Ok(true); };
                                if !subscribed_keys.contains(&target) { return Ok(true); }
                                let now = std::time::Instant::now();
                                if relay_copies.redundant(&id, relay_url.as_str(), now) || inflight.contains_key(&id) { return Ok(true); }
                                let Some(ticket) = admission.try_admit(&peer, &target.to_hex()) else {
                                    state.metrics.ingress_rejections.fetch_add(1,Ordering::Relaxed);
                                    return Ok(true);
                                };
                                relay_copies.admitted(&id, relay_url.as_str(), now);
                                inflight.insert(id.clone(),std::time::Instant::now());
                                let processor = processor.clone();
                                let store = state.store.clone();
                                let worker_id = id.clone();
                                let task = workers.spawn(async move {
                                    let _ticket = ticket;
                                    let result = processor.prepare_response(&event).await;
                                    if result.is_ok() {
                                        let _ = store.mark_relay_received(relay_url.as_str(), &id, event.created_at.as_secs() as i64).await;
                                    }
                                    (id, result)
                                });
                                worker_events.insert(task.id(), worker_id);
                            }
                            _ => {}
                        }
                    }
                }
                Some(result) = workers.join_next(), if !workers.is_empty() => {
                    let (id,result) = match result {
                        Ok(value) => { worker_events.retain(|_, event_id| event_id != &value.0); value },
                        Err(error) => {
                            if let Some(id) = worker_events.remove(&error.id()) { inflight.remove(&id); relay_copies.forget(&id); }
                            tracing::error!("request worker panicked; durable input retained for retry");
                            return Ok(true);
                        }
                    };
                    inflight.remove(&id);
                    if result.is_err() { relay_copies.forget(&id); }
                    retry.reset_immediately();
                    match result {
                        Err(crate::protocol::ProtocolError::Capacity(response)) if publishers.len() < 16 => {
                            let processor = processor.clone();
                            let client = client.clone();
                            publishers.spawn(async move {
                                let published = processor.publish_response(&client, &response).await.is_ok();
                                (id, published, false)
                            });
                        }
                        Err(error) => tracing::debug!(event_id=%id, error=%error, "request rejected or pending retry"),
                        _ => {},
                    }
                }
                Some(result) = publishers.join_next(), if !publishers.is_empty() => {
                    let (id,published,durable) = match result {
                        Ok(value) => value,
                        Err(_) => {
                            publishing.clear();
                            tracing::error!("publisher panicked; durable responses retained for retry");
                            return Ok(true);
                        }
                    };
                    publishing.remove(&id);
                    if published { delivery_failure = None; }
                    else { delivery_failure = Some(std::time::Instant::now()); }
                    if durable { state.store.mark_published(&id,published).await?; }
                }
                _ = retry.tick() => {
                    for (id,_) in state.store.outbox().await? {
                        if publishers.len() >= 16 { break; }
                        if !publishing.insert(id.clone()) { continue; }
                        let processor = processor.clone();
                        let client = client.clone();
                        publishers.spawn(async move {
                            let published = processor.publish_cached_response(&client,&id).await.is_ok();
                            (id,published,true)
                        });
                    }
                }
                _ = refresh.tick() => {
                    if inflight.values().any(|start|start.elapsed()>Duration::from_secs(45)) {
                        return Err(StoreError::InvalidInput("request worker progress stalled".into()));
                    }
                    (minimum,recovery_pending) = sqlx::query_as::query_as("SELECT minimum_connected_relays,recovery_pending FROM instance_settings WHERE singleton=1").fetch_one(&state.store.pool).await?;
                    let grants = state.store.active_grants().await?;
                    let mut public_keys = BTreeSet::new();
                    let mut quarantined = 0;
                    for grant in &grants {
                        match state.store.validate_runtime_grant(grant) {
                            Ok(()) => { if let Ok(key) = PublicKey::from_hex(&grant.remote_signer_public_key) { public_keys.insert(key); } }
                            Err(_) => { quarantined += 1; tracing::warn!(grant_id=grant.id,"grant quarantined: invalid policy or key envelope"); }
                        }
                    }
                    state.quarantined_grants.store(quarantined,Ordering::Relaxed);
                    let desired = state.store.enabled_relays().await?;
                    telemetry.configure(&desired);
                    let mut changed = false;
                    for (url,_) in client.relays().await {
                        if !desired.iter().any(|wanted| wanted.trim_end_matches('/') == url.as_str().trim_end_matches('/')) {
                            let _ = client.remove_relay(url).await;
                            changed = true;
                        }
                    }
                    for url in &desired {
                        changed |= client.add_relay(url.as_str()).await.unwrap_or(false);
                    }
                    client.connect().await;
                    let relays = client.relays().await;
                    accepted.retain(|url| relays.iter().any(|(u,r)| u.as_str()==url && r.status().is_connected()));
                    state.relay_diagnostics.write().await.retain(|url,_| desired.iter().any(|wanted| wanted.trim_end_matches('/') == url));
                    for (url,relay) in &relays {
                        let previous = state.relay_diagnostics.read().await.get(url.as_str().trim_end_matches('/')).cloned();
                        let connection = relay.status().to_string().to_lowercase();
                        let changed = previous.as_ref().is_none_or(|p| p.connection != connection || p.attempts != relay.stats().attempts() || p.successes != relay.stats().success());
                        if changed && relay.status().is_connected() {
                            state.store.mark_relay_connected(url.as_str()).await?;
                        } else if changed && connection == "disconnected" {
                            state.store.mark_relay_subscription(url.as_str(),Some("disconnected")).await?;
                        }
                    }
                    subscription_retries.retain(|url,_| relays.keys().any(|u|u.as_str()==url));
                    let connected = relays.values().filter(|r| r.status().is_connected()).count();
                    state.connected_relays.store(connected,Ordering::Relaxed);
                    if public_keys != subscribed_keys || changed {
                        let _ = client.unsubscribe(&subscription).await;
                        subscription = SubscriptionId::generate();
                        accepted.clear();
                        subscription_retries.clear();
                        subscribed_keys = public_keys;
                        if !subscribed_keys.is_empty() {
                            let filter = Filter::new().pubkeys(subscribed_keys.iter().copied()).kind(Kind::NostrConnect)
                                .since(Timestamp::from_secs(Timestamp::now().as_secs().saturating_sub(300)));
                            if client.subscribe(filter).with_id(subscription.clone()).await.is_err() {
                                state.metrics.relay_failures.fetch_add(1,Ordering::Relaxed);
                            }
                        }
                    }
                    if !subscribed_keys.is_empty() {
                        for (url,relay) in client.relays().await {
                            if relay.status().is_connected() && !accepted.contains(url.as_str()) {
                                let retry = subscription_retries.entry(url.to_string()).or_default();
                                if !retry.due(std::time::Instant::now()) { continue; }
                                retry.attempted(std::time::Instant::now());
                                let filter=Filter::new().pubkeys(subscribed_keys.iter().copied()).kind(Kind::NostrConnect).since(Timestamp::from_secs(Timestamp::now().as_secs().saturating_sub(300)));
                                let _=relay.subscribe(filter).with_id(subscription.clone()).await;
                            }
                        }
                    }
                    {
                        let mut diagnostics = state.relay_diagnostics.write().await;
                        for (url, relay) in &relays {
                            diagnostics.entry(url.as_str().trim_end_matches('/').to_owned()).or_default()
                                .observe(relay, !subscribed_keys.is_empty(), accepted.contains(&url.to_string()), Timestamp::now().as_secs() as i64);
                        }

                    }
                    initialized = true;
                    // Encrypted inputs admitted before a crash can resume without relay redelivery.
                    for json in state.store.pending_inputs().await? {
                        if workers.len() >= 32 { break; }
                        if let Ok(event) = Event::from_json(json) {
                            let id = event.id.to_hex();
                            if inflight.contains_key(&id) { continue; }
                            let Ok(target) = recipient(&event) else { continue; };
                            if !subscribed_keys.contains(&target) { continue; }
                            let Some(ticket) = admission.try_admit(&event.pubkey.to_hex(), &target.to_hex()) else { continue; };
                            inflight.insert(id.clone(),std::time::Instant::now());
                            let processor = processor.clone();
                            let worker_id = id.clone();
                            let task = workers.spawn(async move { let _ticket=ticket; let result=processor.prepare_response(&event).await; (id,result) });
                            worker_events.insert(task.id(), worker_id);
                        }
                    }
                }
                _ = state.reload.notified() => { refresh.reset_immediately(); }
                _ = prune.tick() => { state.store.prune_requests().await?; }
                _ = telemetry_flush.tick() => {
                    if pending_telemetry.is_empty() { pending_telemetry = telemetry.drain(); }
                    state.store.persist_relay_observations(&pending_telemetry).await?;
                    {
                        let mut diagnostics = state.relay_diagnostics.write().await;
                        for event in &pending_telemetry {
                            if let Some(relay) = diagnostics.get_mut(&event.url) {
                                if event.category.starts_with("error_") {
                                    let message = crate::relay_transport::description(event.category);
                                    relay.log(event.last_at, "warning", message.into());
                                    if relay.connection != "connected" { relay.transport_error = Some(message); }
                                }
                            }
                        }
                    }
                    pending_telemetry.clear();
                }
                _ = maintenance.tick() => { state.store.maintenance().await?; state.store.prune_relay_history().await?; }
                _ = integrity.tick() => {
                    let healthy: String = sqlx::query_scalar::query_scalar("PRAGMA quick_check").fetch_one(&state.store.pool).await?;
                    state.integrity_ok.store(healthy=="ok",Ordering::Relaxed);
                    if healthy!="ok" { return Err(StoreError::InvalidInput("database integrity check failed".into())); }
                }
                _ = shutdown.changed() => { if *shutdown.borrow() {return Ok(false);} }
            }
            Ok(true)
            }.await;
            match step {
                Ok(false) => break,
                Ok(true) => storage_ok = true,
                Err(StoreError::Database(error)) if retryable_database_error(&error) => {
                    if storage_ok { tracing::warn!("signer database temporarily unavailable; retrying"); }
                    storage_ok = false;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) => return Err(error),
            }
            let delivery_ok = delivery_is_healthy(delivery_failure, std::time::Instant::now());
            state.last_progress.store(progress_tick(),Ordering::Relaxed);
            state.ready.store(initialized && storage_ok && !recovery_pending && state.integrity_ok.load(Ordering::Relaxed) && state.quarantined_grants.load(Ordering::Relaxed)==0
                && (subscribed_keys.is_empty() || (delivery_ok && accepted.len()>=minimum.max(1) as usize)),Ordering::Relaxed);
        }
        Ok(())
    }.await;
    state.ready.store(false, Ordering::Relaxed);
    // Aborting cannot lose an admitted input or committed response; both are durable.
    workers.abort_all();
    publishers.abort_all();
    while workers.join_next().await.is_some() {}
    while publishers.join_next().await.is_some() {}
    processor.shutdown_replication().await;
    let _ = tokio::time::timeout(Duration::from_secs(5), client.shutdown()).await;
    // A graceful stop flushes the last batch; an abrupt crash can lose at most the unflushed buffer.
    pending_telemetry.extend(telemetry.drain());
    if !pending_telemetry.is_empty() {
        match tokio::time::timeout(
            Duration::from_secs(5),
            state.store.persist_relay_observations(&pending_telemetry),
        )
        .await
        {
            Ok(Ok(())) => {}
            _ => tracing::warn!("could not flush final relay diagnostic counters"),
        }
    }
    result
}

fn delivery_is_healthy(failure: Option<std::time::Instant>, now: std::time::Instant) -> bool {
    failure.is_none_or(|failed| now.duration_since(failed) >= Duration::from_secs(60))
}

// Pool pressure, busy/locked databases and a full disk are recoverable without killing
// unrelated sessions. Corruption and programming/schema errors remain fail-stop.
fn retryable_database_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::PoolTimedOut | sqlx::Error::Io(_) => true,
        sqlx::Error::Database(error) => error
            .code()
            .and_then(|c| c.parse::<i32>().ok())
            .is_some_and(|code| matches!(code & 255, 5 | 6 | 13)),
        _ => false,
    }
}

// A rejecting or silent relay cannot provoke a tight resubscription loop.
struct SubscriptionRetry {
    next: std::time::Instant,
    attempts: u32,
}
impl Default for SubscriptionRetry {
    fn default() -> Self {
        Self {
            next: std::time::Instant::now() + Duration::from_secs(10),
            attempts: 0,
        }
    }
}
impl SubscriptionRetry {
    fn due(&self, now: std::time::Instant) -> bool {
        now >= self.next
    }
    fn attempted(&mut self, now: std::time::Instant) {
        let delay = (10u64 << self.attempts.min(5)).min(300);
        self.attempts = self.attempts.saturating_add(1);
        // Randomize retries across hosts, without relying on wall-clock time.
        let mut random = [0u8; 1];
        let _ = getrandom::fill(&mut random);
        self.next = now + Duration::from_secs(delay + u64::from(random[0]) % 6);
    }
}
#[cfg(unix)]
async fn wait_for_shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = terminate.recv() => {},
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn progress_tick() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs()
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    #[test]
    fn failed_delivery_signal_expires_on_an_idle_instance() {
        let now = std::time::Instant::now();
        assert!(delivery_is_healthy(None, now));
        assert!(!delivery_is_healthy(
            Some(now),
            now + Duration::from_secs(59)
        ));
        assert!(delivery_is_healthy(
            Some(now),
            now + Duration::from_secs(60)
        ));
    }
    #[test]
    fn subscription_backoff_is_monotonic_bounded_and_resets_on_replacement() {
        let mut retry = SubscriptionRetry::default();
        let mut now = std::time::Instant::now();
        assert!(!retry.due(now));
        for expected in [10, 20, 40, 80, 160, 300, 300] {
            retry.attempted(now);
            let delay = retry.next.duration_since(now).as_secs();
            assert!((expected..=expected + 5).contains(&delay));
            assert!(!retry.due(now));
            now = retry.next;
            assert!(retry.due(now));
        }
        assert_eq!(SubscriptionRetry::default().attempts, 0);
    }
}
