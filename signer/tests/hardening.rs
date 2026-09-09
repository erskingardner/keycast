use keycast_core::v2::{
    envelope::EnvelopeCipher,
    management::{MANAGEMENT_KIND, MANAGEMENT_READ_KIND},
    policy::RequestedCapabilities,
};
use keycast_signer::{
    protocol::{RequestProcessor, RuntimeMetrics},
    runtime::{relay_supervisor, RuntimeState},
    store::Store,
};
use nostr::{nips::nip44, prelude::*};
use nostr_sdk::prelude::{
    Client, ClientNotification, LocalRelay, QueryPolicy, QueryPolicyResult, StreamExt, WritePolicy,
    WritePolicyResult,
};
use serde_json::{json, Value};
use sqlx::{query::query, query_scalar::query_scalar, raw_sql::raw_sql};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use zeroize::Zeroizing;

struct Fixture {
    store: Store,
    remote: Keys,
    user: Keys,
    client: Keys,
    secret: String,
    grant: i64,
    policy: i64,
}
impl Fixture {
    async fn new(relay: &str) -> Self {
        let pool = sqlx_sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        query("PRAGMA foreign_keys=ON")
            .execute(&pool)
            .await
            .unwrap();
        raw_sql(concat!(
            include_str!("../../database/migrations/0001_initial.sql"),
            "\n",
            include_str!("../../database/migrations/0003_relay_reliability.sql"),
            "\n",
            include_str!("../../database/migrations/0004_key_relay_discovery.sql")
        ))
        .execute(&pool)
        .await
        .unwrap();
        Self::populate(pool, relay).await
    }
    async fn populate(pool: sqlx_sqlite::SqlitePool, relay: &str) -> Self {
        query("DELETE FROM relays").execute(&pool).await.unwrap();
        query("INSERT INTO relays(url) VALUES(?)")
            .bind(relay)
            .execute(&pool)
            .await
            .unwrap();
        let team: i64 = query_scalar("INSERT INTO teams(name) VALUES('test') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
        let policy:i64=query_scalar("INSERT INTO policies(team_id,name,document) VALUES(?,'test',?) RETURNING id").bind(team).bind(json!({"version":1,"capabilities":{"sign_event":{"allowed_kinds":[1,27235,MANAGEMENT_KIND,MANAGEMENT_READ_KIND]}}}).to_string()).fetch_one(&pool).await.unwrap();
        let store = Store::new(pool, EnvelopeCipher::from_key(Zeroizing::new([21; 32])));
        let user = Keys::generate();
        let key = store
            .seal_stored_key(
                team,
                &user.public_key().to_hex(),
                "test".into(),
                Zeroizing::new(user.secret_key().to_secret_hex()),
            )
            .await
            .unwrap();
        let (grant, _, uri) = store
            .create_grant(
                team,
                &user.public_key().to_hex(),
                key.id,
                policy,
                "test".into(),
                None,
                chrono::Utc::now().timestamp() + 300,
            )
            .await
            .unwrap();
        let remote = store
            .decrypt_remote_keys(
                &store
                    .active_grants()
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|g| g.id == grant.id)
                    .unwrap(),
            )
            .unwrap();
        Self {
            store,
            remote,
            user,
            client: Keys::generate(),
            secret: uri
                .split("secret=")
                .nth(1)
                .unwrap()
                .split('&')
                .next()
                .unwrap()
                .into(),
            grant: grant.id,
            policy,
        }
    }
    fn processor(&self) -> RequestProcessor {
        RequestProcessor::new(self.store.clone(), Arc::new(RuntimeMetrics::default()))
    }
    fn event(&self, id: &str, method: &str, params: Vec<String>) -> Event {
        let plain = json!({"id":id,"method":method,"params":params}).to_string();
        let content = nip44::encrypt(
            self.client.secret_key(),
            &self.remote.public_key(),
            plain,
            nip44::Version::default(),
        )
        .unwrap();
        EventBuilder::new(Kind::NostrConnect, content)
            .tag(Tag::public_key(self.remote.public_key()))
            .finalize(&self.client)
            .unwrap()
    }
    fn connect(&self) -> Event {
        self.event(
            "connect",
            "connect",
            vec![self.remote.public_key().to_hex(), self.secret.clone()],
        )
    }
    fn decrypt(&self, response: &Event) -> Value {
        serde_json::from_str(
            &nip44::decrypt(
                self.client.secret_key(),
                &self.remote.public_key(),
                &response.content,
            )
            .unwrap(),
        )
        .unwrap()
    }
}
async fn ready(state: &RuntimeState) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while !state.ready.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("runtime ready after EOSE");
}
async fn reply<S: StreamExt<Item = ClientNotification> + Unpin>(
    stream: &mut S,
    id: &str,
    f: &Fixture,
) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(n) = stream.next().await {
            if let ClientNotification::Message { message, .. } = n {
                if let RelayMessage::Event { event, .. } = *message {
                    if event.pubkey == f.remote.public_key() {
                        let value = f.decrypt(&event);
                        if value["id"] == id {
                            return value;
                        }
                    }
                }
            }
        }
        panic!("stream ended")
    })
    .await
    .expect("response through production runtime")
}
#[derive(Debug)]
struct FreshEnvelopes(Arc<std::sync::atomic::AtomicUsize>);
impl WritePolicy for FreshEnvelopes {
    fn admit_event<'a>(
        &'a self,
        event: &'a Event,
        _: &'a std::net::SocketAddr,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = WritePolicyResult> + Send + 'a>> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::Relaxed);
            if Timestamp::now()
                .as_secs()
                .saturating_sub(event.created_at.as_secs())
                >= 60
            {
                WritePolicyResult::reject(MachineReadablePrefix::Invalid, "ephemeral event expired")
            } else {
                WritePolicyResult::Accept
            }
        })
    }
}

#[tokio::test]
async fn cached_reply_refreshes_its_ephemeral_envelope_without_executing_again() {
    let publications = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let relay = LocalRelay::builder()
        .write_policy(FreshEnvelopes(publications.clone()))
        .build();
    relay.run().await.unwrap();
    let f = Fixture::new(relay.url().await.as_str()).await;
    let processor = f.processor();
    processor.prepare_response(&f.connect()).await.unwrap();
    let request = f.event("cached-reply", "ping", vec![]);
    let response = processor.prepare_response(&request).await.unwrap().unwrap();
    let old = EventBuilder::new(Kind::NostrConnect, response.content.clone())
        .tags(response.tags.clone())
        .custom_created_at(Timestamp::from_secs(Timestamp::now().as_secs() - 120))
        .finalize(&f.remote)
        .unwrap();
    query("UPDATE processed_requests SET response_event_json=? WHERE event_id=?")
        .bind(old.as_json())
        .bind(request.id.to_hex())
        .execute(&f.store.pool)
        .await
        .unwrap();
    let client = Client::new();
    client.add_relay(relay.url().await).await.unwrap();
    client.connect().and_wait(Duration::from_secs(2)).await;
    assert!(processor.publish_response(&client, &old).await.is_err());
    processor
        .publish_cached_response(&client, &request.id.to_hex())
        .await
        .unwrap();
    let fresh = Event::from_json(
        f.store
            .cached_session_response(&request.id.to_hex())
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    fresh.verify().unwrap();
    assert_ne!(fresh.id, old.id);
    assert!(Timestamp::now().as_secs() - fresh.created_at.as_secs() < 30);
    assert!(
        fresh.content == old.content,
        "inner RPC ciphertext must remain unchanged"
    );
    assert_eq!(f.decrypt(&fresh)["result"], "pong");
    let count: i64 = query_scalar("SELECT count(*) FROM audit_events WHERE request_event_id=?")
        .bind(request.id.to_hex())
        .fetch_one(&f.store.pool)
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "refreshing delivery must not execute or audit the operation again"
    );
    assert_eq!(publications.load(Ordering::Relaxed), 2);
    // A previously queued response is no longer publishable after revocation.
    f.store
        .revoke_grant(f.grant, &f.user.public_key().to_hex())
        .await
        .unwrap();
    query("UPDATE processed_requests SET response_event_json=? WHERE event_id=?")
        .bind(old.as_json())
        .bind(request.id.to_hex())
        .execute(&f.store.pool)
        .await
        .unwrap();
    processor
        .publish_cached_response(&client, &request.id.to_hex())
        .await
        .unwrap();
    let retained: String =
        query_scalar("SELECT response_event_json FROM processed_requests WHERE event_id=?")
            .bind(request.id.to_hex())
            .fetch_one(&f.store.pool)
            .await
            .unwrap();
    assert_eq!(Event::from_json(retained).unwrap().id, old.id);
    assert_eq!(
        publications.load(Ordering::Relaxed),
        2,
        "revoked response must not reach a relay"
    );
    client.shutdown().await;
    relay.shutdown();
}

#[tokio::test]
async fn delayed_copies_from_other_relays_do_not_republish_or_consume_admission() {
    let mut relays = Vec::new();
    for _ in 0..3 {
        let relay = LocalRelay::new();
        relay.run().await.unwrap();
        relays.push(relay);
    }
    let f = Fixture::new(relays[0].url().await.as_str()).await;
    for relay in &relays[1..] {
        query("INSERT INTO relays(url) VALUES(?)")
            .bind(relay.url().await.as_str())
            .execute(&f.store.pool)
            .await
            .unwrap();
    }
    f.processor().prepare_response(&f.connect()).await.unwrap();
    let observer = Client::new();
    for relay in &relays {
        observer.add_relay(relay.url().await).await.unwrap();
    }
    let mut stream = observer.notifications();
    observer.connect().and_wait(Duration::from_secs(2)).await;
    observer
        .subscribe(
            Filter::new()
                .kind(Kind::NostrConnect)
                .pubkey(f.client.public_key()),
        )
        .await
        .unwrap();
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    ready(&state).await;
    let first = observer
        .relay(relays[0].url().await)
        .await
        .unwrap()
        .unwrap();
    let request = f.event("relay-copies", "ping", vec![]);
    first.send_event(&request).await.unwrap();
    assert_eq!(
        reply(&mut stream, "relay-copies", &f).await["result"],
        "pong"
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let status: String =
                query_scalar("SELECT status FROM processed_requests WHERE event_id=?")
                    .bind(request.id.to_hex())
                    .fetch_one(&f.store.pool)
                    .await
                    .unwrap();
            if status == "completed" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let before: i64 =
        query_scalar("SELECT publish_attempts FROM processed_requests WHERE event_id=?")
            .bind(request.id.to_hex())
            .fetch_one(&f.store.pool)
            .await
            .unwrap();
    // Other relays deliver the same signed event after the first response was ACKed.
    for relay in &relays[1..] {
        observer
            .relay(relay.url().await)
            .await
            .unwrap()
            .unwrap()
            .send_event(&request)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    let after: i64 =
        query_scalar("SELECT publish_attempts FROM processed_requests WHERE event_id=?")
            .bind(request.id.to_hex())
            .fetch_one(&f.store.pool)
            .await
            .unwrap();
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    observer.shutdown().await;
    for relay in relays {
        relay.shutdown();
    }
    assert_eq!(
        after, before,
        "redundant relay copies must not restart response publication"
    );
    assert_eq!(state.metrics.ingress_rejections.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn runtime_handles_duplicate_delivery_and_durable_restart_outbox() {
    let relay = LocalRelay::new();
    relay.run().await.unwrap();
    let f = Fixture::new(relay.url().await.as_str()).await;
    let observer = Client::new();
    observer.add_relay(relay.url().await).await.unwrap();
    let mut stream = observer.notifications();
    observer.connect().and_wait(Duration::from_secs(2)).await;
    observer
        .subscribe(
            Filter::new()
                .kind(Kind::NostrConnect)
                .pubkey(f.client.public_key()),
        )
        .await
        .unwrap();
    // Simulate crash after response commit but before network delivery.
    let connect = f.connect();
    f.processor().prepare_response(&connect).await.unwrap();
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    ready(&state).await;
    assert_eq!(reply(&mut stream, "connect", &f).await["result"], "ack");
    // The same inbound event must reach our cache even after SDK deduplication.
    observer.send_event(&connect).await.unwrap();
    assert_eq!(reply(&mut stream, "connect", &f).await["result"], "ack");
    observer.send_event(&connect).await.unwrap();
    assert_eq!(reply(&mut stream, "connect", &f).await["result"], "ack");
    let count: i64 = query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(&f.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    // A crash after inbox admission but before execution is also recoverable.
    let pending = f.event("pending", "ping", vec![]);
    let grant = f.store.active_grants().await.unwrap().pop().unwrap();
    let session = f
        .store
        .active_session(f.grant, &f.client.public_key())
        .await
        .unwrap()
        .unwrap();
    f.store
        .begin_request(
            &pending.id.to_hex(),
            &grant,
            Some(session.id),
            &f.client.public_key(),
            "pending",
            "ping",
            &pending.as_json(),
        )
        .await
        .unwrap();
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    ready(&state).await;
    assert_eq!(reply(&mut stream, "pending", &f).await["result"], "pong");
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    observer.shutdown().await;
    relay.shutdown();
}
#[tokio::test]
async fn logout_survives_failed_publish_and_reserved_management_kind_is_denied() {
    let f = Fixture::new("ws://127.0.0.1:1").await;
    let p = f.processor();
    p.prepare_response(&f.connect()).await.unwrap();
    for (kind, allowed) in [
        (1, true),
        (27235, true),
        (MANAGEMENT_KIND, false),
        (MANAGEMENT_READ_KIND, false),
    ] {
        let template = json!({"pubkey":f.user.public_key(),"created_at":Timestamp::now(),"kind":kind,"tags":[],"content":""});
        let response = p
            .prepare_response(&f.event(
                &format!("kind-{kind}"),
                "sign_event",
                vec![template.to_string()],
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(f.decrypt(&response).get("result").is_some(), allowed);
    }
    let logout = f.event("logout", "logout", vec![]);
    assert!(p
        .process_and_publish(&Client::new(), &logout)
        .await
        .is_err());
    assert!(f
        .store
        .active_session(f.grant, &f.client.public_key())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        f.decrypt(&p.prepare_response(&logout).await.unwrap().unwrap())["result"],
        "ack"
    );
    assert!(p
        .prepare_response(&f.event("after", "ping", vec![]))
        .await
        .is_err());
}
#[tokio::test]
async fn stale_grant_cannot_claim_and_unknown_clients_cannot_fill_inbox() {
    let f = Fixture::new("ws://127.0.0.1:1").await;
    let grant = f.store.active_grants().await.unwrap().pop().unwrap();
    let mut expired = grant.clone();
    expired.expires_at = Some(chrono::Utc::now().timestamp() - 1);
    assert!(
        f.store.decrypt_stored_keys(&expired).is_err(),
        "expiry must be rechecked at key use after admission I/O"
    );
    let invalid = f.event(
        "invalid",
        "connect",
        vec![f.remote.public_key().to_hex(), "wrong".into()],
    );
    assert!(f.processor().prepare_response(&invalid).await.is_err());
    let count: i64 = query_scalar("SELECT count(*) FROM processed_requests")
        .fetch_one(&f.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    f.processor().prepare_response(&f.connect()).await.unwrap();
    query("UPDATE request_storage_budget SET bytes=134217728,records=10000")
        .execute(&f.store.pool)
        .await
        .unwrap();
    assert!(
        f.processor()
            .prepare_response(&f.event("over-budget", "ping", vec![]))
            .await
            .is_err(),
        "storage exhaustion must abort admission, not silently skip its accounting trigger"
    );
    let persisted: i64 = query_scalar("SELECT count(*) FROM processed_requests")
        .fetch_one(&f.store.pool)
        .await
        .unwrap();
    assert_eq!(persisted, 1);
    query("UPDATE grants SET created_at=unixepoch()-1,expires_at=unixepoch() WHERE id=?")
        .bind(f.grant)
        .execute(&f.store.pool)
        .await
        .unwrap();
    assert!(f
        .store
        .claim_invitation(
            &grant,
            &f.client.public_key(),
            &f.secret,
            &RequestedCapabilities::unrestricted(),
            None
        )
        .await
        .is_err());
}
#[tokio::test]
async fn corrupt_grant_is_quarantined_while_healthy_grants_continue() {
    let relay = LocalRelay::new();
    relay.run().await.unwrap();
    let f = Fixture::new(relay.url().await.as_str()).await;
    // A second grant with a corrupt envelope must not stop the valid grant.
    let g = f.store.active_grants().await.unwrap().pop().unwrap();
    let (bad, _, _) = f
        .store
        .create_grant(
            g.team_id,
            &f.user.public_key().to_hex(),
            g.stored_key_id,
            f.policy,
            "bad".into(),
            None,
            chrono::Utc::now().timestamp() + 300,
        )
        .await
        .unwrap();
    query("UPDATE grants SET remote_signer_secret_envelope=x'00' WHERE id=?")
        .bind(bad.id)
        .execute(&f.store.pool)
        .await
        .unwrap();
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    tokio::time::timeout(Duration::from_secs(3), async {
        while state.quarantined_grants.load(Ordering::Relaxed) == 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let observer = Client::new();
    observer.add_relay(relay.url().await).await.unwrap();
    let mut stream = observer.notifications();
    observer.connect().and_wait(Duration::from_secs(2)).await;
    observer
        .subscribe(
            Filter::new()
                .kind(Kind::NostrConnect)
                .pubkey(f.client.public_key()),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    observer.send_event(&f.connect()).await.unwrap();
    assert_eq!(reply(&mut stream, "connect", &f).await["result"], "ack");
    assert!(!state.ready.load(Ordering::Relaxed));
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    observer.shutdown().await;
    relay.shutdown();
}

#[derive(Debug)]
struct RejectQueries(Arc<std::sync::atomic::AtomicUsize>);
impl QueryPolicy for RejectQueries {
    fn admit_query<'a>(
        &'a self,
        _: &'a mut Filter,
        _: &'a std::net::SocketAddr,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = QueryPolicyResult> + Send + 'a>> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            QueryPolicyResult::reject(MachineReadablePrefix::Blocked, "queries disabled")
        })
    }
}
#[tokio::test]
async fn connected_relay_that_rejects_subscription_is_not_ready() {
    let rejects = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let relay = LocalRelay::builder()
        .query_policy(RejectQueries(rejects.clone()))
        .build();
    relay.run().await.unwrap();
    let f = Fixture::new(relay.url().await.as_str()).await;
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    tokio::time::timeout(Duration::from_secs(4), async {
        while rejects.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!state.ready.load(Ordering::Relaxed));
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    relay.shutdown();
}
#[derive(Debug)]
struct SlowAck;
impl WritePolicy for SlowAck {
    fn admit_event<'a>(
        &'a self,
        _: &'a Event,
        _: &'a std::net::SocketAddr,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = WritePolicyResult> + Send + 'a>> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            WritePolicyResult::Accept
        })
    }
}
#[tokio::test]
async fn first_healthy_ack_does_not_wait_for_slow_relay() {
    let good = LocalRelay::new();
    good.run().await.unwrap();
    let slow = LocalRelay::builder().write_policy(SlowAck).build();
    slow.run().await.unwrap();
    let f = Fixture::new(good.url().await.as_str()).await;
    let c = Client::new();
    c.add_relay(good.url().await).await.unwrap();
    c.add_relay(slow.url().await).await.unwrap();
    c.connect().and_wait(Duration::from_secs(2)).await;
    query("INSERT INTO relays(url) VALUES(?)")
        .bind(slow.url().await.as_str())
        .execute(&f.store.pool)
        .await
        .unwrap();
    let processor = f.processor();
    let start = std::time::Instant::now();
    processor
        .process_and_publish(&c, &f.connect())
        .await
        .unwrap();
    assert!(start.elapsed() < Duration::from_millis(750));
    processor.shutdown_replication().await;
    let acknowledged: i64 =
        query_scalar("SELECT count(*) FROM relay_checkpoints WHERE last_published_at IS NOT NULL")
            .fetch_one(&f.store.pool)
            .await
            .unwrap();
    assert_eq!(
        acknowledged, 2,
        "bounded replication must finish after first ACK returns"
    );
    c.shutdown().await;
    good.shutdown();
    slow.shutdown();
}

#[tokio::test]
async fn host_cli_backup_rotation_restore_and_overwrite_refusal() {
    use std::{path::Path, process::Command};
    let directory = std::env::temp_dir().join(format!(
        "keycast-cli-test-{}",
        Keys::generate().public_key()
    ));
    std::fs::create_dir(&directory).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let path = directory.join("source.db");
    let db = keycast_core::database::Database::new(path.clone(), root.join("database/migrations"))
        .await
        .unwrap();
    let f = Fixture::populate(db.pool.clone(), "ws://127.0.0.1:1").await;
    f.processor().prepare_response(&f.connect()).await.unwrap();
    let root_file = directory.join("root.key");
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&root_file)
        .unwrap();
    file.write_all("15".repeat(32).as_bytes()).unwrap();
    drop(file);
    let backup_key = directory.join("backup.key");
    let archive = directory.join("backup.kcb");
    let rotated = directory.join("next.key");
    let restored = directory.join("restored");
    let cli = |args: Vec<&str>, db_path: &Path, credential: &Path| {
        Command::new(env!("CARGO_BIN_EXE_keycast_signer"))
            .args(args)
            .env("KEYCAST_DATABASE_PATH", db_path)
            .env("KEYCAST_ROOT_KEY_FILE", credential)
            .env("KEYCAST_MIGRATIONS_PATH", root.join("database/migrations"))
            .env_remove("CREDENTIALS_DIRECTORY")
            .output()
            .unwrap()
    };
    query("INSERT INTO users(public_key) VALUES(?)")
        .bind(f.user.public_key().to_hex())
        .execute(&f.store.pool)
        .await
        .unwrap();
    query("INSERT INTO team_members(team_id,user_public_key,role) VALUES(1,?,'admin')")
        .bind(f.user.public_key().to_hex())
        .execute(&f.store.pool)
        .await
        .unwrap();
    let imported = Keys::generate();
    let mut import = Command::new(env!("CARGO_BIN_EXE_keycast_signer"))
        .args(["import", "1", &f.user.public_key().to_hex(), "CLI import"])
        .env("KEYCAST_DATABASE_PATH", &path)
        .env("KEYCAST_ROOT_KEY_FILE", &root_file)
        .env("KEYCAST_MIGRATIONS_PATH", root.join("database/migrations"))
        .env_remove("CREDENTIALS_DIRECTORY")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    import
        .stdin
        .take()
        .unwrap()
        .write_all(imported.secret_key().to_secret_hex().as_bytes())
        .unwrap();
    let output = import.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(&imported.public_key().to_hex()));
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains(&imported.secret_key().to_secret_hex())
    );
    let audit_path = directory.join("audit.jsonl");
    assert!(cli(
        vec!["audit-export", audit_path.to_str().unwrap()],
        &path,
        &root_file
    )
    .status
    .success());
    let export = std::fs::read_to_string(&audit_path).unwrap();
    assert!(export.contains("stored_key.create"));
    assert!(!export.contains(&f.secret));
    assert!(!export.contains(&imported.secret_key().to_secret_hex()));
    assert!(export
        .lines()
        .all(|line| serde_json::from_str::<Value>(line).is_ok()));
    for file in [&backup_key, &rotated] {
        let out = cli(
            vec!["generate-key", file.to_str().unwrap()],
            &path,
            &root_file,
        );
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let out = cli(
        vec![
            "backup",
            backup_key.to_str().unwrap(),
            archive.to_str().unwrap(),
        ],
        &path,
        &root_file,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!cli(
        vec![
            "backup",
            backup_key.to_str().unwrap(),
            archive.to_str().unwrap()
        ],
        &path,
        &root_file
    )
    .status
    .success());
    let lock = keycast_signer::maintenance::lock(
        &keycast_signer::maintenance::maintenance_path(&path),
        true,
    )
    .unwrap();
    assert!(!cli(
        vec!["rotate-root", rotated.to_str().unwrap()],
        &path,
        &root_file
    )
    .status
    .success());
    drop(lock);
    let out = cli(
        vec!["rotate-root", rotated.to_str().unwrap()],
        &path,
        &root_file,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(keycast_signer::maintenance::verify_root(&f.store)
        .await
        .is_err());
    let rotated_store = Store::new(
        db.pool.clone(),
        EnvelopeCipher::from_file(&rotated).unwrap(),
    );
    keycast_signer::maintenance::verify_root(&rotated_store)
        .await
        .unwrap();
    for g in rotated_store.active_grants().await.unwrap() {
        rotated_store.validate_runtime_grant(&g).unwrap();
    }
    let out = cli(
        vec![
            "restore",
            backup_key.to_str().unwrap(),
            archive.to_str().unwrap(),
            restored.to_str().unwrap(),
        ],
        &path,
        &rotated,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!cli(
        vec![
            "restore",
            backup_key.to_str().unwrap(),
            archive.to_str().unwrap(),
            restored.to_str().unwrap()
        ],
        &path,
        &rotated
    )
    .status
    .success());
    let restored_db = keycast_core::database::Database::new(
        restored.join("keycast-v2.db"),
        root.join("database/migrations"),
    )
    .await
    .unwrap();
    let pending: i64 = query_scalar("SELECT recovery_pending FROM instance_settings")
        .fetch_one(&restored_db.pool)
        .await
        .unwrap();
    assert_eq!(pending, 1);
    let active: i64 = query_scalar("SELECT count(*) FROM sessions WHERE ended_at IS NULL")
        .fetch_one(&restored_db.pool)
        .await
        .unwrap();
    assert_eq!(active, 0);
    assert!(!restored.join("RESTORE_INCOMPLETE").exists());
    let out = cli(
        vec!["review-restore"],
        &restored.join("keycast-v2.db"),
        &restored.join("root.key"),
    );
    assert!(out.status.success());
    let live: i64 = query_scalar("SELECT count(*) FROM grants WHERE revoked_at IS NULL")
        .fetch_one(&restored_db.pool)
        .await
        .unwrap();
    assert_eq!(live, 0);
    restored_db.pool.close().await;
    db.pool.close().await;
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn runtime_isolates_noisy_grant_before_waiting_for_authority() {
    let relay = LocalRelay::new();
    relay.run().await.unwrap();
    let mut f = Fixture::new(relay.url().await.as_str()).await;
    let noisy_remote = f.remote.clone();
    let original = f.store.active_grants().await.unwrap().pop().unwrap();
    let (grant, _, uri) = f
        .store
        .create_grant(
            original.team_id,
            &f.user.public_key().to_hex(),
            original.stored_key_id,
            f.policy,
            "quiet".into(),
            None,
            chrono::Utc::now().timestamp() + 300,
        )
        .await
        .unwrap();
    f.grant = grant.id;
    f.secret = uri
        .split("secret=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap()
        .into();
    f.remote = f
        .store
        .decrypt_remote_keys(
            &f.store
                .active_grants()
                .await
                .unwrap()
                .into_iter()
                .find(|g| g.id == f.grant)
                .unwrap(),
        )
        .unwrap();
    let observer = Client::new();
    observer.add_relay(relay.url().await).await.unwrap();
    let mut stream = observer.notifications();
    observer.connect().and_wait(Duration::from_secs(2)).await;
    observer
        .subscribe(
            Filter::new()
                .kind(Kind::NostrConnect)
                .pubkey(f.client.public_key()),
        )
        .await
        .unwrap();
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    ready(&state).await;
    let authority = f.store.authority.lock().await;
    for n in 0..12 {
        let attacker = Keys::generate();
        let event = EventBuilder::new(Kind::NostrConnect, format!("invalid-{n}"))
            .tag(Tag::public_key(noisy_remote.public_key()))
            .finalize(&attacker)
            .unwrap();
        observer.send_event(&event).await.unwrap();
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        while state.metrics.ingress_rejections.load(Ordering::Relaxed) < 4 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("per-grant concurrent limit rejects rotating clients");
    observer.send_event(&f.connect()).await.unwrap();
    drop(authority);
    assert_eq!(reply(&mut stream, "connect", &f).await["result"], "ack");
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    observer.shutdown().await;
    relay.shutdown();
}

#[tokio::test]
async fn forged_flood_cannot_starve_a_client_that_already_holds_a_session() {
    // A grant's remote-signer public key is published in every bunker URI and in
    // every client request, so anyone can address events to it. Admission must
    // not let that traffic consume the capacity a live session depends on.
    let relay = LocalRelay::new();
    relay.run().await.unwrap();
    let f = Fixture::new(relay.url().await.as_str()).await;
    let observer = Client::new();
    observer.add_relay(relay.url().await).await.unwrap();
    let mut stream = observer.notifications();
    observer.connect().and_wait(Duration::from_secs(2)).await;
    observer
        .subscribe(
            Filter::new()
                .kind(Kind::NostrConnect)
                .pubkey(f.client.public_key()),
        )
        .await
        .unwrap();
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    ready(&state).await;

    // Establish the session that must survive the flood.
    observer.send_event(&f.connect()).await.unwrap();
    assert_eq!(reply(&mut stream, "connect", &f).await["result"], "ack");
    let sessions: i64 = query_scalar("SELECT count(*) FROM sessions WHERE ended_at IS NULL")
        .fetch_one(&f.store.pool)
        .await
        .unwrap();
    assert_eq!(sessions, 1);
    // Let the runtime pick the new session up into its live-session index.
    for _ in 0..40 {
        state.reload.notify_waiters();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Hold the authority gate so admitted workers occupy their lane, then flood
    // from rotating keys the way an attacker on a shared relay would.
    let authority = f.store.authority.lock().await;
    let before = state.metrics.ingress_rejections.load(Ordering::Relaxed);
    for n in 0..24 {
        let attacker = Keys::generate();
        let forged = EventBuilder::new(Kind::NostrConnect, format!("forged-{n}"))
            .tag(Tag::public_key(f.remote.public_key()))
            .finalize(&attacker)
            .unwrap();
        observer.send_event(&forged).await.unwrap();
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        while state.metrics.ingress_rejections.load(Ordering::Relaxed) <= before {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the newcomer lane rejects a forged flood");

    // The established client is served from its own lane while that flood is
    // still being rejected. Before the lanes were split this ping was dropped.
    observer
        .send_event(&f.event("ping-under-flood", "ping", vec![]))
        .await
        .unwrap();
    drop(authority);
    assert_eq!(
        reply(&mut stream, "ping-under-flood", &f).await["result"],
        "pong"
    );

    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    observer.shutdown().await;
    relay.shutdown();
}

include!("support/process_recovery.rs");

#[tokio::test]
#[ignore = "explicit local latency/soak run; writes only disposable fixtures"]
async fn local_runtime_soak_and_latency_report() {
    let samples: usize = std::env::var("KEYCAST_SOAK_REQUESTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    assert!((100..=10000).contains(&samples));
    let (directory, f) = file_fixture().await;
    let relay = LocalRelay::builder()
        .rate_limit(nostr_sdk::prelude::RateLimit {
            max_reqs: 500,
            notes_per_minute: 10000,
        })
        .build();
    relay.run().await.unwrap();
    query("UPDATE relays SET url=?")
        .bind(relay.url().await.as_str())
        .execute(&f.store.pool)
        .await
        .unwrap();
    let observer = Client::new();
    observer.add_relay(relay.url().await).await.unwrap();
    let mut stream = observer.notifications();
    observer.connect().and_wait(Duration::from_secs(2)).await;
    observer
        .subscribe(
            Filter::new()
                .kind(Kind::NostrConnect)
                .pubkey(f.client.public_key()),
        )
        .await
        .unwrap();
    let mut child = daemon(&directory);
    daemon_ready(&directory, &mut child).await;
    assert!(
        !observer
            .send_event(&f.connect())
            .await
            .unwrap()
            .success
            .is_empty(),
        "test relay must acknowledge connection input"
    );
    assert_eq!(reply(&mut stream, "connect", &f).await["result"], "ack");
    let mut latencies = Vec::new();
    let start = std::time::Instant::now();
    for n in 0..samples {
        if n > 0 && n % 250 == 0 {
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            child = daemon(&directory);
            daemon_ready(&directory, &mut child).await;
        }
        let id = format!("soak-{n}");
        let event = f.event(
            &id,
            "sign_event",
            vec![
                json!({"created_at":Timestamp::now(),"kind":1,"tags":[],"content":id}).to_string(),
            ],
        );
        let began = std::time::Instant::now();
        assert!(
            !observer
                .send_event(&event)
                .await
                .unwrap()
                .success
                .is_empty(),
            "test relay must acknowledge input"
        );
        let response = reply(&mut stream, &id, &f).await;
        let signed =
            Event::from_json(response["result"].as_str().expect("allowed sign request")).unwrap();
        signed.verify().unwrap();
        assert_eq!(signed.pubkey, f.user.public_key());
        assert_eq!(signed.content, id);
        latencies.push(began.elapsed().as_micros() as u64);
        // Exercise sustained valid traffic below the intentional per-client admission budget.
        tokio::time::sleep(Duration::from_millis(70)).await;
    }
    latencies.sort_unstable();
    let stats = daemon_status(&directory).await.unwrap();
    println!(
        "{}",
        json!({"samples":samples,"elapsed_seconds":start.elapsed().as_secs_f64(),"p50_us":latencies[samples/2],"p95_us":latencies[samples*95/100],"p99_us":latencies[samples*99/100],"maximum_us":latencies[samples-1],"resources":stats.resources,"process_restarts":(samples-1)/250})
    );
    assert!(stats.ready);
    assert!(stats.resources.inbox_records <= samples as i64 + 1);
    assert!(stats.resources.wal_bytes < 64 * 1024 * 1024);
    drop(child);
    observer.shutdown().await;
    relay.shutdown();
    f.store.pool.close().await;
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn crypto_policy_and_session_narrowing_cover_all_four_methods() {
    let f = Fixture::new("ws://127.0.0.1:1").await;
    let methods = [
        "nip04_encrypt",
        "nip04_decrypt",
        "nip44_encrypt",
        "nip44_decrypt",
    ];
    let mut caps = json!({});
    for method in methods {
        caps[method] = json!({"recipient":"self_only"});
    }
    query("UPDATE policies SET document=? WHERE id=?")
        .bind(json!({"version":1,"capabilities":caps}).to_string())
        .bind(f.policy)
        .execute(&f.store.pool)
        .await
        .unwrap();
    f.processor().prepare_response(&f.connect()).await.unwrap();
    let other = Keys::generate();
    for method in methods {
        for peer in [&f.user, &other] {
            let input = if method == "nip04_decrypt" {
                nostr::nips::nip04::encrypt(peer.secret_key(), &f.user.public_key(), "message")
                    .unwrap()
            } else if method == "nip44_decrypt" {
                nip44::encrypt(
                    peer.secret_key(),
                    &f.user.public_key(),
                    "message",
                    nip44::Version::default(),
                )
                .unwrap()
            } else {
                "message".to_owned()
            };
            let event = f.event(
                &format!("{method}-{}", peer.public_key()),
                method,
                vec![peer.public_key().to_hex(), input],
            );
            let response = f
                .processor()
                .prepare_response(&event)
                .await
                .unwrap()
                .unwrap();
            let response = f.decrypt(&response);
            assert_eq!(
                response.get("result").is_some(),
                peer.public_key() == f.user.public_key(),
                "{method}"
            );
            if let Some(result) = response["result"].as_str() {
                if method.ends_with("decrypt") {
                    assert_eq!(result, "message");
                } else if method == "nip04_encrypt" {
                    assert_eq!(
                        nostr::nips::nip04::decrypt(
                            peer.secret_key(),
                            &f.user.public_key(),
                            result
                        )
                        .unwrap(),
                        "message"
                    );
                } else {
                    assert_eq!(
                        nip44::decrypt(peer.secret_key(), &f.user.public_key(), result).unwrap(),
                        "message"
                    );
                }
            }
        }
        let invalid = f.event(
            &format!("invalid-{method}"),
            method,
            vec!["not-a-pubkey".into(), "message".into()],
        );
        assert!(f
            .decrypt(
                &f.processor()
                    .prepare_response(&invalid)
                    .await
                    .unwrap()
                    .unwrap()
            )
            .get("error")
            .is_some());
    }
    query("UPDATE sessions SET requested_capabilities='[]'")
        .execute(&f.store.pool)
        .await
        .unwrap();
    for method in methods {
        let response = f
            .processor()
            .prepare_response(&f.event(
                &format!("narrow-{method}"),
                method,
                vec![f.user.public_key().to_hex(), "message".into()],
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(
            f.decrypt(&response).get("error").is_some(),
            "empty requested capabilities must deny {method}"
        );
    }
    let session = f
        .store
        .active_session(f.grant, &f.client.public_key())
        .await
        .unwrap()
        .unwrap();
    f.store.touch_session(session.id).await.unwrap();
    f.store.end_session(session.id).await.unwrap();
    assert!(f.store.touch_session(session.id).await.is_err());
}

#[tokio::test]
async fn malformed_routing_signatures_and_clock_skew_never_enter_durable_inbox() {
    let f = Fixture::new("ws://127.0.0.1:1").await;
    let valid = f.connect();
    let mut cases = Vec::new();
    for tags in [
        vec![],
        vec![
            Tag::public_key(f.remote.public_key()),
            Tag::parse(["p", "bad"]).unwrap(),
        ],
        vec![Tag::parse(["p", &f.remote.public_key().to_hex(), "extra"]).unwrap()],
    ] {
        cases.push(
            EventBuilder::new(Kind::NostrConnect, valid.content.clone())
                .tags(tags)
                .finalize(&f.client)
                .unwrap(),
        );
    }
    for created in [
        Timestamp::now().as_secs() - 400,
        Timestamp::now().as_secs() + 120,
    ] {
        cases.push(
            EventBuilder::new(Kind::NostrConnect, valid.content.clone())
                .tag(Tag::public_key(f.remote.public_key()))
                .custom_created_at(Timestamp::from_secs(created))
                .finalize(&f.client)
                .unwrap(),
        );
    }
    let mut forged = serde_json::to_value(&valid).unwrap();
    forged["content"] = json!("tampered");
    cases.push(serde_json::from_value(forged).unwrap());
    for event in cases {
        assert!(f.processor().prepare_response(&event).await.is_err());
    }
    assert_eq!(
        query_scalar::<_, i64>("SELECT count(*) FROM processed_requests")
            .fetch_one(&f.store.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        f.decrypt(
            &f.processor()
                .prepare_response(&valid)
                .await
                .unwrap()
                .unwrap()
        )["result"],
        "ack"
    );
}

#[tokio::test]
async fn authentication_required_relay_never_reports_signing_ready() {
    let relay = LocalRelay::builder()
        .nip42(nostr_sdk::prelude::LocalRelayBuilderNip42::read())
        .build();
    relay.run().await.unwrap();
    let f = Fixture::new(relay.url().await.as_str()).await;
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            // Includes startup, before the first refresh: no transient empty-grant readiness.
            assert!(!state.ready.load(Ordering::Relaxed));
            let rejected = state
                .relay_diagnostics
                .read()
                .await
                .values()
                .any(|relay| relay.subscription == "rejected");
            if rejected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("authentication-required subscription rejected");
    assert!(!state.ready.load(Ordering::Relaxed));
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    relay.shutdown();
}

#[tokio::test]
async fn changing_relays_preserves_the_live_session_and_serves_on_the_new_subscription() {
    let first = LocalRelay::new();
    first.run().await.unwrap();
    let next = LocalRelay::new();
    next.run().await.unwrap();
    let f = Fixture::new(first.url().await.as_str()).await;
    let observer = Client::new();
    observer.add_relay(first.url().await).await.unwrap();
    observer.add_relay(next.url().await).await.unwrap();
    let mut stream = observer.notifications();
    observer.connect().and_wait(Duration::from_secs(2)).await;
    observer
        .subscribe(
            Filter::new()
                .kind(Kind::NostrConnect)
                .pubkey(f.client.public_key()),
        )
        .await
        .unwrap();
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    ready(&state).await;
    observer.send_event(&f.connect()).await.unwrap();
    assert_eq!(reply(&mut stream, "connect", &f).await["result"], "ack");
    query("DELETE FROM relays")
        .execute(&f.store.pool)
        .await
        .unwrap();
    query("INSERT INTO relays(url) VALUES(?)")
        .bind(next.url().await.as_str())
        .execute(&f.store.pool)
        .await
        .unwrap();
    let next_url = next.url().await;
    state.reload.notify_one();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let accepted = state
                .relay_diagnostics
                .read()
                .await
                .get(next_url.as_str().trim_end_matches('/'))
                .is_some_and(|relay| relay.subscription == "accepted");
            if accepted {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    observer.remove_relay(first.url().await).await.unwrap();
    first.shutdown();
    observer
        .send_event(&f.event("new-relay", "ping", vec![]))
        .await
        .unwrap();
    assert_eq!(reply(&mut stream, "new-relay", &f).await["result"], "pong");
    assert_eq!(
        query_scalar::<_, i64>("SELECT count(*) FROM sessions WHERE ended_at IS NULL")
            .fetch_one(&f.store.pool)
            .await
            .unwrap(),
        1
    );
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    observer.shutdown().await;
    next.shutdown();
}

#[tokio::test]
async fn oversized_crypto_response_is_durably_denied_before_relay_rejection() {
    let f = Fixture::new("ws://127.0.0.1:1").await;
    query("UPDATE policies SET document=? WHERE id=?")
        .bind(json!({"version":1,"capabilities":{"nip44_encrypt":{"recipient":"any"}}}).to_string())
        .bind(f.policy)
        .execute(&f.store.pool)
        .await
        .unwrap();
    let processor = f.processor();
    processor.prepare_response(&f.connect()).await.unwrap();
    let request = f.event(
        "large-response",
        "nip44_encrypt",
        vec![Keys::generate().public_key().to_hex(), "x".repeat(150000)],
    );
    let response = processor
        .prepare_response(&request)
        .await
        .expect("bounded denial must be returned")
        .unwrap();
    assert!(f.decrypt(&response).get("error").is_some());
    let status: String = query_scalar("SELECT status FROM processed_requests WHERE event_id=?")
        .bind(request.id.to_hex())
        .fetch_one(&f.store.pool)
        .await
        .unwrap();
    assert_eq!(status, "denied");
    assert_eq!(
        processor
            .prepare_response(&request)
            .await
            .unwrap()
            .unwrap()
            .id,
        response.id
    );
}

#[tokio::test]
async fn one_grants_storage_pressure_returns_a_retry_error_and_preserves_other_teams() {
    let f = Fixture::new("ws://127.0.0.1:1").await;
    let p = f.processor();
    p.prepare_response(&f.connect()).await.unwrap();
    let mut capacity = false;
    for n in 0..60 {
        let event = f.event(
            &format!("large-{n}"),
            "unsupported",
            vec!["x".repeat(128000)],
        );
        match p.prepare_response(&event).await {
            Err(keycast_signer::protocol::ProtocolError::Capacity(reply)) => {
                assert!(f.decrypt(&reply)["error"]
                    .as_str()
                    .unwrap()
                    .contains("retry later"));
                capacity = true;
                break;
            }
            result => {
                assert!(f.decrypt(&result.unwrap().unwrap())["error"].is_string());
            }
        }
    }
    assert!(capacity);
    let resources = f.store.resources().await.unwrap();
    assert!(resources.inbox_bytes <= 4 * 1024 * 1024);
    assert_eq!(
        resources.pending_inputs, 0,
        "reserved response space must allow every admitted operation to complete"
    );
    let mut other = Fixture::populate(f.store.pool.clone(), "ws://127.0.0.1:1").await;
    other.store = f.store.clone();
    // populate selects the first active grant; select this fixture's own remote key.
    let grant = other
        .store
        .active_grants()
        .await
        .unwrap()
        .into_iter()
        .find(|g| g.id == other.grant)
        .unwrap();
    other.remote = other.store.decrypt_remote_keys(&grant).unwrap();
    let processor = other.processor();
    processor.prepare_response(&other.connect()).await.unwrap();
    assert_eq!(
        other.decrypt(
            &processor
                .prepare_response(&other.event("healthy", "ping", vec![]))
                .await
                .unwrap()
                .unwrap()
        )["result"],
        "pong"
    );
    query("DELETE FROM processed_requests")
        .execute(&f.store.pool)
        .await
        .unwrap();
    assert_eq!(f.store.resources().await.unwrap().inbox_bytes, 0);
}

#[tokio::test]
async fn logout_blocks_cached_signatures_and_denied_retries_keep_their_outcome() {
    let f = Fixture::new("ws://127.0.0.1:1").await;
    let p = f.processor();
    p.prepare_response(&f.connect()).await.unwrap();
    let denied = f.event("denied", "unsupported", vec![]);
    p.prepare_response(&denied).await.unwrap();
    f.store
        .mark_published(&denied.id.to_hex(), true)
        .await
        .unwrap();
    p.prepare_response(&denied).await.unwrap();
    assert_eq!(
        query_scalar::<_, String>("SELECT status FROM processed_requests WHERE event_id=?")
            .bind(denied.id.to_hex())
            .fetch_one(&f.store.pool)
            .await
            .unwrap(),
        "denied"
    );
    let template = json!({"kind":1,"created_at":Timestamp::now(),"tags":[],"content":"test"});
    let signed = f.event("signed", "sign_event", vec![template.to_string()]);
    p.prepare_response(&signed).await.unwrap();
    let logout = f.event("logout", "logout", vec![]);
    p.prepare_response(&logout).await.unwrap();
    assert!(p.prepare_response(&signed).await.is_err());
    let outbox = f.store.outbox().await.unwrap();
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox[0].0, logout.id.to_hex());
}

#[tokio::test]
async fn invitation_lifetime_is_enforced_in_the_signer_store() {
    let f = Fixture::new("ws://127.0.0.1:1").await;
    let actor = f.user.public_key().to_hex();
    let now = chrono::Utc::now().timestamp();
    assert!(f
        .store
        .create_invitation(f.grant, &actor, now + 7 * 86400 + 60)
        .await
        .is_err());
    assert!(f
        .store
        .create_invitation(f.grant, &actor, now + 7 * 86400)
        .await
        .is_ok());
    let grant = f.store.active_grants().await.unwrap().pop().unwrap();
    assert!(f
        .store
        .create_grant(
            grant.team_id,
            &actor,
            grant.stored_key_id,
            f.policy,
            "excessive".into(),
            None,
            9999999999
        )
        .await
        .is_err());
    assert!(f
        .store
        .create_grant(
            grant.team_id,
            &actor,
            grant.stored_key_id,
            f.policy,
            "short".into(),
            Some(now + 30),
            now + 20
        )
        .await
        .is_ok());
}

#[tokio::test]
async fn runtime_recovers_from_pool_pressure_without_restarting() {
    let relay = LocalRelay::new();
    relay.run().await.unwrap();
    let pool = sqlx_sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_millis(50))
        .connect("sqlite::memory:")
        .await
        .unwrap();
    raw_sql(concat!(
        include_str!("../../database/migrations/0001_initial.sql"),
        "\n",
        include_str!("../../database/migrations/0003_relay_reliability.sql"),
        "\n",
        include_str!("../../database/migrations/0004_key_relay_discovery.sql")
    ))
    .execute(&pool)
    .await
    .unwrap();
    let f = Fixture::populate(pool, relay.url().await.as_str()).await;
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    ready(&state).await;
    let connection = f.store.pool.acquire().await.unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        !task.is_finished(),
        "pool pressure must not terminate the supervisor"
    );
    drop(connection);
    ready(&state).await;
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    relay.shutdown();
}

#[tokio::test]
async fn idle_connected_relays_clear_stale_errors_without_a_signing_subscription() {
    let relay = LocalRelay::new();
    relay.run().await.unwrap();
    let url = relay.url().await;
    let f = Fixture::new(url.as_str()).await;
    query("UPDATE grants SET revoked_at=unixepoch()")
        .execute(&f.store.pool)
        .await
        .unwrap();
    f.store
        .mark_relay_subscription(url.as_str(), Some("disconnected"))
        .await
        .unwrap();
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            state.reload.notify_one();
            let connected = state
                .relay_diagnostics
                .read()
                .await
                .values()
                .any(|relay| relay.connection == "connected");
            if connected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let snapshot = state
        .relay_diagnostics
        .read()
        .await
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(snapshot.subscription, "idle");
    assert!(snapshot.connected_at.is_some());
    assert!(snapshot.successes > 0);
    assert!(snapshot.subscription_error.is_none());
    let checkpoint: (Option<i64>, i64, Option<String>) = sqlx::query_as::query_as(
        "SELECT last_connected_at,consecutive_failures,last_error FROM relay_checkpoints",
    )
    .fetch_one(&f.store.pool)
    .await
    .unwrap();
    assert!(checkpoint.0.is_some());
    assert_eq!(checkpoint.1, 0);
    assert!(checkpoint.2.is_none());
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    let reliability = f.store.relay_reliability().await.unwrap();
    let r = reliability.values().next().unwrap();
    assert!(r.lifetime.attempts >= 1);
    assert!(r.lifetime.connections >= 1);
    assert_eq!(
        r.lifetime.remote_closes, 0,
        "local shutdown is not a peer-forced close"
    );
    relay.shutdown();
}

#[tokio::test]
async fn completed_reconnect_backlog_does_not_reject_fresh_requests() {
    let relay = LocalRelay::new();
    relay.run().await.unwrap();
    let f = Fixture::new(relay.url().await.as_str()).await;
    let processor = f.processor();
    processor.prepare_response(&f.connect()).await.unwrap();
    let mut requests = Vec::new();
    for n in 0..13 {
        let event = f.event(&format!("replay-{n}"), "ping", vec![]);
        processor.prepare_response(&event).await.unwrap();
        f.store
            .mark_published(&event.id.to_hex(), true)
            .await
            .unwrap();
        requests.push(event);
    }
    let observer = Client::new();
    observer.add_relay(relay.url().await).await.unwrap();
    let mut stream = observer.notifications();
    observer.connect().and_wait(Duration::from_secs(2)).await;
    observer
        .subscribe(
            Filter::new()
                .kind(Kind::NostrConnect)
                .pubkey(f.client.public_key()),
        )
        .await
        .unwrap();
    let state = RuntimeState::new(f.store.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(relay_supervisor(state.clone(), rx));
    ready(&state).await;
    // Runtime restart loaded only durable IDs. Hold authority to keep the replay
    // workers busy while all thirteen old requests and a fresh one arrive.
    let authority = f.store.authority.lock().await;
    for event in &requests {
        observer.send_event(event).await.unwrap();
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        while state.metrics.cached_retries.load(Ordering::Relaxed) < 13 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    observer
        .send_event(&f.event("fresh-after-replay", "ping", vec![]))
        .await
        .unwrap();
    drop(authority);
    assert_eq!(
        reply(&mut stream, "fresh-after-replay", &f).await["result"],
        "pong"
    );
    assert_eq!(state.metrics.ingress_rejections.load(Ordering::Relaxed), 0);

    // Also check the durable record count: no operation was re-executed under a new ID.
    let records: i64 = query_scalar("SELECT count(*) FROM processed_requests WHERE method='ping'")
        .fetch_one(&f.store.pool)
        .await
        .unwrap();
    assert_eq!(records, 14);

    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    observer.shutdown().await;
    relay.shutdown();
}

#[tokio::test]
async fn relay_discovery_scopes_advertising_and_response_publication_to_owning_key() {
    let baseline = LocalRelay::new();
    baseline.run().await.unwrap();
    let first = Fixture::new(baseline.url().await.as_str()).await;
    let second = Fixture::populate(first.store.pool.clone(), baseline.url().await.as_str()).await;
    let scoped = LocalRelay::new();
    scoped.run().await.unwrap();
    let url = scoped.url().await.to_string();
    let relay_id: i64 = query_scalar("INSERT INTO relays(url,discovered) VALUES(?,1) RETURNING id")
        .bind(&url)
        .fetch_one(&first.store.pool)
        .await
        .unwrap();
    let key = first
        .store
        .active_grants()
        .await
        .unwrap()
        .into_iter()
        .find(|g| g.id == first.grant)
        .unwrap()
        .stored_key_id;
    query("INSERT INTO key_relays(stored_key_id,url,read,write,relay_id,status) VALUES(?,?,1,1,?,'compatible')").bind(key).bind(&url).bind(relay_id).execute(&first.store.pool).await.unwrap();
    let a = first.processor();
    let b = second.processor();
    a.prepare_response(&first.connect()).await.unwrap();
    b.prepare_response(&second.connect()).await.unwrap();
    let r = a
        .prepare_response(&first.event("switch-first", "switch_relays", vec![]))
        .await
        .unwrap()
        .unwrap();
    let advertised: Vec<String> =
        serde_json::from_str(first.decrypt(&r)["result"].as_str().unwrap()).unwrap();
    assert!(advertised.contains(&url));
    let r = b
        .prepare_response(&second.event("switch-second", "switch_relays", vec![]))
        .await
        .unwrap()
        .unwrap();
    let advertised: Vec<String> =
        serde_json::from_str(second.decrypt(&r)["result"].as_str().unwrap()).unwrap();
    assert!(!advertised.contains(&url));
    let client = Client::new();
    client.add_relay(baseline.url().await).await.unwrap();
    client.add_relay(scoped.url().await).await.unwrap();
    client.connect().and_wait(Duration::from_secs(2)).await;
    b.publish_response(&client, &r).await.unwrap();
    b.shutdown_replication().await;
    let leaked: i64 = query_scalar(
        "SELECT count(*) FROM relay_checkpoints WHERE relay_id=? AND last_published_at IS NOT NULL",
    )
    .bind(relay_id)
    .fetch_one(&first.store.pool)
    .await
    .unwrap();
    assert_eq!(
        leaked, 0,
        "unrelated key response must not be published on the discovered relay"
    );
    query("UPDATE key_relays SET retire_at=unixepoch()-1 WHERE relay_id=?")
        .bind(relay_id)
        .execute(&first.store.pool)
        .await
        .unwrap();
    assert!(!first
        .store
        .transport_relays(key)
        .await
        .unwrap()
        .contains(&url));
    client.shutdown().await;
    baseline.shutdown();
    scoped.shutdown();
}
