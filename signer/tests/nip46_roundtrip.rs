use std::sync::Arc;
use std::time::Duration;

use keycast_core::v2::envelope::EnvelopeCipher;
use keycast_signer::protocol::{RequestProcessor, RuntimeMetrics};
use keycast_signer::store::Store;
use nostr::nips::nip44;
use nostr::prelude::{Event, EventBuilder, FinalizeEvent, Keys, Kind, Tag, Timestamp};
use nostr_sdk::prelude::{Client, ClientNotification, Filter, LocalRelay, StreamExt};
use serde_json::{json, Value};
use sqlx::{query::query, query_scalar::query_scalar, raw_sql::raw_sql};
use sqlx_sqlite::SqlitePoolOptions;
use zeroize::Zeroizing;

#[tokio::test]
async fn connect_then_sign_round_trips_through_a_real_local_relay() {
    let relay = LocalRelay::new();
    relay.run().await.expect("start local relay");
    let relay_url = relay.url().await;

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .expect("connect test database");
    query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("enable foreign keys");
    raw_sql(include_str!("../../database/migrations/0001_initial.sql"))
        .execute(&pool)
        .await
        .expect("apply v2 schema");
    query("DELETE FROM relays")
        .execute(&pool)
        .await
        .expect("remove public relay defaults");
    query("INSERT INTO relays(url, sort_order) VALUES (?, 10)")
        .bind(relay_url.to_string())
        .execute(&pool)
        .await
        .expect("configure local relay");

    let team_id: i64 = query_scalar("INSERT INTO teams(name) VALUES ('Test') RETURNING id")
        .fetch_one(&pool)
        .await
        .expect("create team");
    let policy_id: i64 = query_scalar(
        "INSERT INTO policies(team_id, name, document)
         VALUES (?, 'Notes', '{\"version\":1,\"capabilities\":{\"sign_event\":{\"allowed_kinds\":[1]}}}')
         RETURNING id",
    )
    .bind(team_id)
    .fetch_one(&pool)
    .await
    .expect("create policy");
    let store = Store::new(pool, EnvelopeCipher::from_key(Zeroizing::new([23_u8; 32])));
    let user_keys = Keys::generate();
    let actor = user_keys.public_key().to_hex();
    let stored = store
        .seal_stored_key(
            team_id,
            &actor,
            "User key".to_string(),
            Zeroizing::new(user_keys.secret_key().to_secret_hex()),
        )
        .await
        .expect("seal user key");
    let (grant_summary, _, bunker_uri) = store
        .create_grant(
            team_id,
            &actor,
            stored.id,
            policy_id,
            "Test client".to_string(),
            None,
            chrono::Utc::now().timestamp() + 300,
        )
        .await
        .expect("create grant");
    let secret = bunker_uri.split("secret=").nth(1).expect("one-time secret");
    let grant = store
        .active_grants()
        .await
        .expect("load grant")
        .pop()
        .expect("active grant");
    let remote_keys = store.decrypt_remote_keys(&grant).expect("remote key");
    assert_eq!(
        remote_keys.public_key().to_hex(),
        grant_summary.remote_signer_public_key
    );

    let server = Client::new();
    server
        .add_relay(relay_url.clone())
        .await
        .expect("add relay");
    server.connect().and_wait(Duration::from_secs(2)).await;
    let processor = RequestProcessor::new(store, Arc::new(RuntimeMetrics::default()));
    let client_keys = Keys::generate();
    let observer = Client::new();
    observer
        .add_relay(relay_url.clone())
        .await
        .expect("add observer relay");
    let mut notifications = observer.notifications();
    observer.connect().and_wait(Duration::from_secs(2)).await;
    observer
        .subscribe(
            Filter::new()
                .author(remote_keys.public_key())
                .kind(Kind::NostrConnect)
                .pubkey(client_keys.public_key()),
        )
        .await
        .expect("subscribe for responses");

    let connect = request_event(
        &client_keys,
        &remote_keys,
        json!({
            "id": "connect-1",
            "method": "connect",
            "params": [remote_keys.public_key().to_hex(), secret, "sign_event:1", "{\"name\":\"integration test\"}"]
        }),
    );
    processor
        .process_and_publish(&server, &connect)
        .await
        .expect("process connect");
    let connect_response =
        receive_response(&mut notifications, &client_keys, &remote_keys, "connect-1").await;
    assert_eq!(connect_response["result"], "ack");

    // nak and several other clients serialize their zero-valued full Event
    // structure even though NIP-46 specifies an unsigned event template.
    let unsigned = json!({
        "id": "0".repeat(64),
        "pubkey": "0".repeat(64),
        "created_at": Timestamp::now().as_secs(),
        "kind": 1,
        "tags": [],
        "content": "hello",
        "sig": "0".repeat(128),
    })
    .to_string();
    let sign = request_event(
        &client_keys,
        &remote_keys,
        json!({"id":"sign-1","method":"sign_event","params":[unsigned]}),
    );
    processor
        .process_and_publish(&server, &sign)
        .await
        .expect("process sign request");
    let sign_response =
        receive_response(&mut notifications, &client_keys, &remote_keys, "sign-1").await;
    let signed = Event::from_json(sign_response["result"].as_str().expect("signed event JSON"))
        .expect("parse signed event");
    signed.verify().expect("verify signed event");
    assert_eq!(signed.pubkey, user_keys.public_key());
    assert_eq!(signed.kind, Kind::TextNote);

    server.shutdown().await;
    observer.shutdown().await;
    relay.shutdown();
}

fn request_event(client: &Keys, remote: &Keys, request: Value) -> Event {
    let content = nip44::encrypt(
        client.secret_key(),
        &remote.public_key(),
        request.to_string(),
        nip44::Version::default(),
    )
    .expect("encrypt request");
    EventBuilder::new(Kind::NostrConnect, content)
        .tag(Tag::public_key(remote.public_key()))
        .finalize(client)
        .expect("sign request")
}

async fn receive_response<S>(notifications: &mut S, client: &Keys, remote: &Keys, id: &str) -> Value
where
    S: StreamExt<Item = ClientNotification> + Unpin,
{
    tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(notification) = notifications.next().await {
            if let ClientNotification::Event { event, .. } = notification {
                if let Ok(plaintext) =
                    nip44::decrypt(client.secret_key(), &remote.public_key(), &event.content)
                {
                    if let Ok(response) = serde_json::from_str::<Value>(&plaintext) {
                        if response["id"] == id {
                            return response;
                        }
                    }
                }
            }
        }
        panic!("notification stream ended before response {id}")
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for response {id}"))
}
