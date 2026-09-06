use keycast_core::{database::Database, v2::envelope::EnvelopeCipher};
use keycast_signer::{relay_transport::Observation, store::Store};
use sqlx::{query::query, query_scalar::query_scalar};
use std::path::PathBuf;
use zeroize::Zeroizing;

fn event(category: &'static str, count: i64, at: i64) -> Observation {
    Observation {
        url: "wss://nos.lol".into(),
        category,
        count,
        first_at: at,
        last_at: at,
    }
}

#[tokio::test]
async fn durable_counts_survive_reopen_while_history_expires_and_is_capped() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let path = std::env::temp_dir().join(format!(
        "keycast-relay-history-{}.db",
        nostr::prelude::Keys::generate().public_key().to_hex()
    ));
    let db = Database::new(path.clone(), root.join("database/migrations"))
        .await
        .unwrap();
    let store = Store::new(
        db.pool.clone(),
        EnvelopeCipher::from_key(Zeroizing::new([42; 32])),
    );
    let now = nostr::prelude::Timestamp::now().as_secs() as i64;
    let old = now - 8 * 86400;
    store
        .persist_relay_observations(&[
            event("error_http_503", 9, old),
            event("attempt", 4, now),
            event("retry", 3, now),
            event("peer_close_1013", 2, now),
        ])
        .await
        .unwrap();
    let id: i64 = query_scalar("SELECT id FROM relays WHERE url='wss://nos.lol'")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    store.prune_relay_history().await.unwrap();
    let r = store
        .relay_reliability()
        .await
        .unwrap()
        .remove(&id)
        .unwrap();
    assert_eq!(r.lifetime.errors, 9);
    assert_eq!(r.last_7_days.errors, 0);
    assert_eq!(r.last_7_days.retries, 3);
    assert_eq!(r.last_7_days.remote_closes, 2);
    assert_eq!(r.tracking_since, Some(old));
    let old_rows: i64 =
        query_scalar("SELECT count(*) FROM relay_reliability_events WHERE last_at<?")
            .bind(now - 604800)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(old_rows, 0);
    let events: Vec<_> = (0..1100)
        .map(|i| event("error_http_503", 1, now - i * 60))
        .collect();
    store.persist_relay_observations(&events).await.unwrap();
    let rows: i64 = query_scalar("SELECT count(*) FROM relay_reliability_events WHERE relay_id=?")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(rows, 1000);
    drop(store);
    db.pool.close().await;
    let reopened = Database::new(path.clone(), root.join("database/migrations"))
        .await
        .unwrap();
    let store = Store::new(
        reopened.pool.clone(),
        EnvelopeCipher::from_key(Zeroizing::new([42; 32])),
    );
    let r = store
        .relay_reliability()
        .await
        .unwrap()
        .remove(&id)
        .unwrap();
    assert_eq!(r.lifetime.errors, 1109);
    assert_eq!(r.last_7_days.errors, 1100);
    assert_eq!(r.lifetime.attempts, 4);
    assert_eq!(r.history.len(), 50);
    assert_eq!(
        r.categories
            .iter()
            .find(|c| c.category == "error_http_503")
            .unwrap()
            .count,
        1100
    );
    // Failed writes roll back totals too, so retaining and retrying a batch doesn't double count.
    query("CREATE TRIGGER fail_diagnostic_insert BEFORE INSERT ON relay_reliability_events BEGIN SELECT RAISE(ABORT, 'test interrupted write'); END").execute(&reopened.pool).await.unwrap();
    assert!(store
        .persist_relay_observations(&[event("attempt", 1, now)])
        .await
        .is_err());
    assert_eq!(
        store.relay_reliability().await.unwrap()[&id]
            .lifetime
            .attempts,
        4
    );
    query("DROP TRIGGER fail_diagnostic_insert")
        .execute(&reopened.pool)
        .await
        .unwrap();
    store
        .persist_relay_observations(&[event("attempt", 1, now)])
        .await
        .unwrap();
    assert_eq!(
        store.relay_reliability().await.unwrap()[&id]
            .lifetime
            .attempts,
        5
    );
    // Disabling preserves counters; explicit deletion follows the relay's existing lifecycle.
    query("UPDATE relays SET enabled=0 WHERE id=?")
        .bind(id)
        .execute(&reopened.pool)
        .await
        .unwrap();
    assert!(store.relay_reliability().await.unwrap().contains_key(&id));
    query("DELETE FROM relays WHERE id=?")
        .bind(id)
        .execute(&reopened.pool)
        .await
        .unwrap();
    assert!(!store.relay_reliability().await.unwrap().contains_key(&id));
    drop(store);
    reopened.pool.close().await;
    std::fs::remove_file(path).unwrap();
}
