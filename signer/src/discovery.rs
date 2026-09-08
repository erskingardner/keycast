//! Bounded public NIP-65 discovery. No root credential or managed key is used
//! for network discovery or transport qualification.
use crate::{
    public_relay,
    store::{Store, StoreError},
};
use async_wsocket::{
    futures_util::{SinkExt, StreamExt},
    Message,
};
use nostr::prelude::*;
use serde::Serialize;
use sqlx::{query::query, query_as::query_as, query_scalar::query_scalar};
use std::{collections::BTreeMap, time::Duration};

#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub url: String,
    pub read: bool,
    pub write: bool,
}
#[derive(Debug, Serialize)]
pub struct KeyRelayInfo {
    pub event_id: Option<String>,
    pub created_at: Option<i64>,
    pub fetched_at: Option<i64>,
    pub status: String,
    pub auto_activate: bool,
    pub relays: Vec<KeyRelay>,
}
#[derive(Debug, Serialize)]
pub struct KeyRelay {
    pub url: String,
    pub read: bool,
    pub write: bool,
    pub listed: bool,
    pub status: String,
    pub verified_at: Option<i64>,
    pub active: bool,
}

pub fn parse(event: &Event, author: PublicKey, now: u64) -> Option<Vec<Candidate>> {
    if event.kind != Kind::RelayList
        || event.pubkey != author
        || event.created_at.as_secs() > now + 60
        || event.content.len() > 8192
        || event.tags.len() > 64
        || event.as_json().len() > 16384
        || event.verify().is_err()
    {
        return None;
    }
    let mut found = BTreeMap::<String, Candidate>::new();
    for tag in event.tags.iter() {
        let fields = tag.as_slice();
        if fields.first().map(String::as_str) != Some("r") || !(2..=3).contains(&fields.len()) {
            continue;
        }
        let marker = fields.get(2).map(String::as_str);
        if !matches!(marker, None | Some("read" | "write")) {
            continue;
        }
        let Some(url) = public_relay::normalize(&fields[1]) else {
            continue;
        };
        if !found.contains_key(&url) && found.len() >= 8 {
            continue;
        }
        let row = found.entry(url.clone()).or_insert(Candidate {
            url,
            read: false,
            write: false,
        });
        row.read |= marker != Some("write");
        row.write |= marker != Some("read");
    }
    Some(found.into_values().collect())
}

async fn fetch_list(url: &str, author: PublicKey) -> Option<Event> {
    let url = Url::parse(url).ok()?;
    let (mut sink, mut stream) = public_relay::connect(&url).await.ok()?;
    let req = serde_json::json!(["REQ","keycast-relays",{"kinds":[10002],"authors":[author.to_hex()],"limit":1}]);
    let result = tokio::time::timeout(Duration::from_secs(4), async {
        sink.send(Message::Text(req.to_string().into()))
            .await
            .ok()?;
        let mut best: Option<Event> = None;
        for _ in 0..64 {
            let msg = stream.next().await?.ok()?;
            let Message::Text(text) = msg else {
                continue;
            };
            if text.len() > 17000 {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&text).ok()?;
            if value[0] == "EOSE" && value[1] == "keycast-relays" {
                break;
            }
            if value[0] != "EVENT" || value[1] != "keycast-relays" {
                continue;
            }
            let Ok(event) = Event::from_json(value[2].to_string()) else {
                continue;
            };
            if parse(&event, author, Timestamp::now().as_secs()).is_some()
                && best.as_ref().is_none_or(|old| newer(&event, old))
            {
                best = Some(event);
            }
        }
        best
    })
    .await
    .ok()
    .flatten();
    let _ = tokio::time::timeout(Duration::from_millis(200), sink.close()).await;
    result
}
fn newer(event: &Event, old: &Event) -> bool {
    event.created_at > old.created_at || (event.created_at == old.created_at && event.id < old.id)
}

/// An ephemeral throwaway identity tests ACK + matching subscription delivery.
/// This never authenticates with, or publishes social content from, an imported key.
async fn qualify(url: &str) -> bool {
    let Ok(url) = Url::parse(url) else {
        return false;
    };
    let Ok((mut sink, mut stream)) = public_relay::connect(&url).await else {
        return false;
    };
    let keys = Keys::generate();
    let Ok(content) = nostr::nips::nip44::encrypt(
        keys.secret_key(),
        &keys.public_key(),
        "relay compatibility probe",
        nostr::nips::nip44::Version::default(),
    ) else {
        return false;
    };
    let Ok(event) = EventBuilder::new(Kind::NostrConnect, content)
        .tag(Tag::public_key(keys.public_key()))
        .finalize(&keys)
    else {
        return false;
    };
    let result=tokio::time::timeout(Duration::from_secs(5),async {
        sink.send(Message::Text(serde_json::json!(["REQ","keycast-probe",{"kinds":[24133],"#p":[keys.public_key().to_hex()]}]).to_string().into())).await.ok()?;
        let mut accepted=false; let mut ack=false; let mut echoed=false;
        for _ in 0..64 {
            let Message::Text(text)=stream.next().await?.ok()? else { continue; };
            if text.len()>17000 { return None; }
            let v:serde_json::Value=serde_json::from_str(&text).ok()?;
            if v[0]=="AUTH" || v[0]=="CLOSED" { return None; }
            if v[0]=="EOSE" && v[1]=="keycast-probe" && !accepted {
                accepted=true;
                sink.send(Message::Text(serde_json::json!(["EVENT",event]).to_string().into())).await.ok()?;
            }
            if v[0]=="OK" && v[1]==event.id.to_hex() { if v[2]!=true { return None; } ack=true; }
            if v[0]=="EVENT" && v[1]=="keycast-probe" {
                let echo=Event::from_json(v[2].to_string()).ok()?;
                echoed=echo.id==event.id && echo.verify().is_ok();
            }
            if ack && echoed { return Some(()); }
        }
        None
    }).await.ok().flatten().is_some();
    let _ = tokio::time::timeout(Duration::from_millis(200), sink.close()).await;
    result
}

impl Store {
    pub async fn key_relay_info(&self, id: i64) -> Result<KeyRelayInfo, StoreError> {
        let (event_id,created_at,fetched_at,status): (Option<String>,Option<i64>,Option<i64>,String)=query_as("SELECT event_id,created_at,fetched_at,status FROM key_relay_lists WHERE stored_key_id=?").bind(id).fetch_one(&self.pool).await?;
        let auto_activate =
            query_scalar("SELECT auto_activate FROM relay_discovery_policy WHERE singleton=1")
                .fetch_one(&self.pool)
                .await?;
        let rows:Vec<(String,bool,bool,bool,String,Option<i64>,bool)>=query_as("SELECT k.url,k.read,k.write,k.listed,k.status,k.verified_at,coalesce(r.enabled,0) AND (k.retire_at IS NULL OR k.retire_at>unixepoch()) AS active FROM key_relays k LEFT JOIN relays r ON r.id=k.relay_id WHERE stored_key_id=? ORDER BY k.listed DESC,k.url").bind(id).fetch_all(&self.pool).await?;
        let relays = rows
            .into_iter()
            .map(
                |(url, read, write, listed, status, verified_at, active)| KeyRelay {
                    url,
                    read,
                    write,
                    listed,
                    status,
                    verified_at,
                    active,
                },
            )
            .collect();
        Ok(KeyRelayInfo {
            event_id,
            created_at,
            fetched_at,
            status,
            auto_activate,
            relays,
        })
    }
    pub async fn transport_relays(&self, key: i64) -> Result<Vec<String>, StoreError> {
        Ok(query_scalar("SELECT DISTINCT r.url FROM relays r WHERE r.enabled=1 AND (r.discovered=0 OR EXISTS(SELECT 1 FROM key_relays k WHERE k.relay_id=r.id AND k.stored_key_id=? AND (k.retire_at IS NULL OR k.retire_at>unixepoch()))) ORDER BY r.sort_order,r.url").bind(key).fetch_all(&self.pool).await?)
    }
    pub async fn active_transport_relays(&self) -> Result<Vec<String>, StoreError> {
        Ok(query_scalar("SELECT url FROM relays r WHERE enabled=1 AND (discovered=0 OR EXISTS(SELECT 1 FROM key_relays k WHERE k.relay_id=r.id AND (k.retire_at IS NULL OR k.retire_at>unixepoch()))) ORDER BY sort_order,url").fetch_all(&self.pool).await?)
    }
}

pub async fn poll(store: &Store) -> Result<(), StoreError> {
    // Retire removed hints after a 24-hour client migration overlap.
    query("DELETE FROM key_relays WHERE retire_at<=unixepoch()")
        .execute(&store.pool)
        .await?;
    query("DELETE FROM relays WHERE discovered=1 AND NOT EXISTS(SELECT 1 FROM key_relays WHERE relay_id=relays.id)").execute(&store.pool).await?;
    let row:Option<(i64,String)>=query_as("SELECT k.id,k.public_key FROM stored_keys k JOIN key_relay_lists d ON d.stored_key_id=k.id WHERE d.next_fetch_at<=unixepoch() ORDER BY d.next_fetch_at,k.id LIMIT 1").fetch_optional(&store.pool).await?;
    let Some((id, public)) = row else {
        return Ok(());
    };
    query("UPDATE key_relay_lists SET next_fetch_at=unixepoch()+300 WHERE stored_key_id=?")
        .bind(id)
        .execute(&store.pool)
        .await?;
    let author = PublicKey::from_hex(&public)
        .map_err(|_| StoreError::InvalidInput("invalid stored public key".into()))?;
    let seeds = store.enabled_relays().await?;
    let mut best = None;
    for seed in
        std::iter::once("wss://purplepag.es").chain(seeds.iter().take(2).map(String::as_str))
    {
        if let Some(event) = fetch_list(seed, author).await {
            if best.as_ref().is_none_or(|old| newer(&event, old)) {
                best = Some(event);
            }
        }
    }
    let Some(event) = best else {
        query("UPDATE key_relay_lists SET status='unavailable',fetched_at=unixepoch(),next_fetch_at=unixepoch()+1800 WHERE stored_key_id=?").bind(id).execute(&store.pool).await?;
        return Ok(());
    };
    let candidates = parse(&event, author, Timestamp::now().as_secs()).unwrap_or_default();
    let mut tx = store.pool.begin().await?;
    let old: Option<(Option<String>, Option<i64>)> =
        query_as("SELECT event_id,created_at FROM key_relay_lists WHERE stored_key_id=?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((old_id, old_at)) = old else {
        return Ok(());
    };
    if old_at.is_none_or(|at| {
        event.created_at.as_secs() as i64 > at
            || (event.created_at.as_secs() as i64 == at
                && old_id
                    .as_deref()
                    .is_none_or(|old| event.id.to_hex().as_str() <= old))
    }) {
        query("UPDATE key_relays SET listed=0,retire_at=coalesce(retire_at,unixepoch()+86400) WHERE stored_key_id=?").bind(id).execute(&mut *tx).await?;
        for c in &candidates {
            // At most 16 retained hints per key, including retirement overlap.
            query("INSERT OR IGNORE INTO key_relays(stored_key_id,url,read,write) SELECT ?,?,?,? WHERE (SELECT count(*) FROM key_relays WHERE stored_key_id=?)<16").bind(id).bind(&c.url).bind(c.read).bind(c.write).bind(id).execute(&mut *tx).await?;
            query("UPDATE key_relays SET read=?,write=?,listed=1,retire_at=NULL WHERE stored_key_id=? AND url=?").bind(c.read).bind(c.write).bind(id).bind(&c.url).execute(&mut *tx).await?;
        }
        query("UPDATE key_relay_lists SET event_id=?,created_at=? WHERE stored_key_id=?")
            .bind(event.id.to_hex())
            .bind(event.created_at.as_secs() as i64)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    query("UPDATE key_relay_lists SET status='cached',fetched_at=unixepoch(),next_fetch_at=unixepoch()+21600 WHERE stored_key_id=?").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    let automatic: bool =
        query_scalar("SELECT auto_activate FROM relay_discovery_policy WHERE singleton=1")
            .fetch_one(&store.pool)
            .await?;
    if !automatic {
        return Ok(());
    }
    let urls:Vec<String>=query_scalar("SELECT url FROM key_relays WHERE stored_key_id=? AND listed=1 ORDER BY write DESC,url LIMIT 4").bind(id).fetch_all(&store.pool).await?;
    for url in urls {
        let compatible = qualify(&url).await;
        let mut tx = store.pool.begin().await?;
        query("UPDATE key_relays SET status=?,verified_at=CASE WHEN ? THEN unixepoch() ELSE verified_at END WHERE stored_key_id=? AND url=?")
            .bind(if compatible {"compatible"} else {"unavailable"}).bind(compatible).bind(id).bind(&url).execute(&mut *tx).await?;
        let enabled: bool =
            query_scalar("SELECT auto_activate FROM relay_discovery_policy WHERE singleton=1")
                .fetch_one(&mut *tx)
                .await?;
        if compatible && enabled {
            let active: i64 = query_scalar(
                "SELECT count(*) FROM key_relays WHERE stored_key_id=? AND relay_id IS NOT NULL",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if active < 4 {
                query("INSERT OR IGNORE INTO relays(url,enabled,sort_order,discovered) SELECT ?,1,100,1 WHERE (SELECT count(*) FROM relays)<20 AND EXISTS(SELECT 1 FROM key_relays WHERE stored_key_id=? AND url=? AND listed=1)").bind(&url).bind(id).bind(&url).execute(&mut *tx).await?;
                query("UPDATE key_relays SET relay_id=(SELECT id FROM relays WHERE url=? AND enabled=1) WHERE stored_key_id=? AND url=? AND listed=1").bind(&url).bind(id).bind(&url).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verifies_author_and_merges_safe_read_write_tags() {
        let keys = Keys::generate();
        let e = EventBuilder::new(Kind::RelayList, "")
            .tags([
                Tag::parse(["r", "wss://relay.example", "read"]).unwrap(),
                Tag::parse(["r", "wss://relay.example/", "write"]).unwrap(),
                Tag::parse(["r", "wss://127.0.0.1"]).unwrap(),
                Tag::parse(["r", "wss://else.example", "unknown"]).unwrap(),
            ])
            .finalize(&keys)
            .unwrap();
        let list = parse(&e, keys.public_key(), Timestamp::now().as_secs()).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].read && list[0].write);
        assert!(parse(
            &e,
            Keys::generate().public_key(),
            Timestamp::now().as_secs()
        )
        .is_none());
        assert!(parse(&e, keys.public_key(), 0).is_none());
    }
}
