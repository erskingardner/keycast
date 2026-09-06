//! Bounded diagnostic storage, separate from security audit events.
use crate::{
    relay_transport::{description, Observation},
    store::{Store, StoreError},
};
use serde::Serialize;
use sqlx::{query::query, query_as::query_as};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize)]
pub struct ReliabilityCounts {
    pub attempts: i64,
    pub retries: i64,
    pub connections: i64,
    pub remote_closes: i64,
    pub connection_losses: i64,
    pub errors: i64,
    pub cancelled_attempts: i64,
    pub dropped_observations: i64,
}
impl ReliabilityCounts {
    fn add(&mut self, category: &str, count: i64) {
        match category {
            "attempt" => self.attempts += count,
            "retry" => self.retries += count,
            "connected" => self.connections += count,
            "connection_lost" => self.connection_losses += count,
            "attempt_cancelled" => self.cancelled_attempts += count,
            "telemetry_dropped" => self.dropped_observations += count,
            _ if category.starts_with("peer_close_") => self.remote_closes += count,
            _ if category.starts_with("error_") => self.errors += count,
            _ => {}
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct ReliabilityCategory {
    pub category: String,
    pub message: &'static str,
    pub count: i64,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReliabilityEvent {
    pub category: String,
    pub message: &'static str,
    pub count: i64,
    pub first_at: i64,
    pub last_at: i64,
}
#[derive(Debug, Clone, Serialize)]
pub struct RelayReliability {
    pub tracking_since: Option<i64>,
    pub window_start: i64,
    pub lifetime: ReliabilityCounts,
    pub last_7_days: ReliabilityCounts,
    pub categories: Vec<ReliabilityCategory>,
    pub history: Vec<ReliabilityEvent>,
}
impl RelayReliability {
    fn new(window_start: i64) -> Self {
        Self {
            tracking_since: None,
            window_start,
            lifetime: Default::default(),
            last_7_days: Default::default(),
            categories: vec![],
            history: vec![],
        }
    }
}

impl Store {
    /// One transaction prevents counters and history disagreeing after interrupted writes.
    /// The caller retains its batch until commit and retries on transient storage failure.
    pub async fn persist_relay_observations(
        &self,
        events: &[Observation],
    ) -> Result<(), StoreError> {
        if events.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        let mut relays = std::collections::BTreeSet::new();
        for event in events {
            let id: Option<i64> =
                sqlx::query_scalar::query_scalar("SELECT id FROM relays WHERE rtrim(url,'/')=?")
                    .bind(event.url.trim_end_matches('/'))
                    .fetch_optional(&mut *tx)
                    .await?;
            let Some(id) = id else {
                continue;
            };
            relays.insert(id);
            query("INSERT INTO relay_reliability_totals(relay_id,category,count,first_at,last_at) VALUES(?,?,?,?,?) ON CONFLICT(relay_id,category) DO UPDATE SET count=count+excluded.count,first_at=min(first_at,excluded.first_at),last_at=max(last_at,excluded.last_at)")
                .bind(id).bind(event.category).bind(event.count).bind(event.first_at).bind(event.last_at).execute(&mut *tx).await?;
            query("INSERT INTO relay_reliability_hours(relay_id,hour,category,count) VALUES(?,?,?,?) ON CONFLICT(relay_id,hour,category) DO UPDATE SET count=count+excluded.count")
                .bind(id).bind(event.first_at / 3600 * 3600).bind(event.category).bind(event.count).execute(&mut *tx).await?;
            query("INSERT INTO relay_reliability_events(relay_id,minute,category,count,first_at,last_at) VALUES(?,?,?,?,?,?) ON CONFLICT(relay_id,minute,category) DO UPDATE SET count=count+excluded.count,first_at=min(first_at,excluded.first_at),last_at=max(last_at,excluded.last_at)")
                .bind(id).bind(event.first_at / 60 * 60).bind(event.category).bind(event.count).bind(event.first_at).bind(event.last_at).execute(&mut *tx).await?;
        }
        for id in relays {
            query("DELETE FROM relay_reliability_events WHERE relay_id=? AND (minute,category) IN (SELECT minute,category FROM relay_reliability_events WHERE relay_id=? ORDER BY minute DESC,category LIMIT -1 OFFSET 1000)")
                .bind(id).bind(id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn prune_relay_history(&self) -> Result<(), StoreError> {
        // Bounded tables: <= 168 hourly buckets/category/relay and <= 1,000 event rows/relay.
        query("DELETE FROM relay_reliability_hours WHERE hour < (unixepoch()/3600-167)*3600")
            .execute(&self.pool)
            .await?;
        query("DELETE FROM relay_reliability_events WHERE last_at < unixepoch()-604800")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn relay_reliability(&self) -> Result<BTreeMap<i64, RelayReliability>, sqlx::Error> {
        let now = nostr::prelude::Timestamp::now().as_secs() as i64;
        let start = (now / 3600 - 167) * 3600;
        let mut result = BTreeMap::new();
        // Read one consistent SQLite snapshot even while the collector flushes.
        let mut tx = self.pool.begin().await?;
        let totals: Vec<(i64, String, i64, i64)> =
            query_as("SELECT relay_id,category,count,first_at FROM relay_reliability_totals")
                .fetch_all(&mut *tx)
                .await?;
        for (id, category, count, first_at) in totals {
            let r = result
                .entry(id)
                .or_insert_with(|| RelayReliability::new(start));
            r.tracking_since = Some(r.tracking_since.map_or(first_at, |old| old.min(first_at)));
            r.lifetime.add(&category, count);
        }
        let hours: Vec<(i64,String,i64)> = query_as("SELECT relay_id,category,sum(count) FROM relay_reliability_hours WHERE hour>=? AND hour<=? GROUP BY relay_id,category ORDER BY sum(count) DESC,category")
            .bind(start).bind(now / 3600 * 3600).fetch_all(&mut *tx).await?;
        for (id, category, count) in hours {
            let r = result
                .entry(id)
                .or_insert_with(|| RelayReliability::new(start));
            r.last_7_days.add(&category, count);
            r.categories.push(ReliabilityCategory {
                message: description(&category),
                category,
                count,
            });
        }
        let history: Vec<(i64,String,i64,i64,i64)> = query_as("SELECT relay_id,category,count,first_at,last_at FROM (SELECT *,row_number() OVER (PARTITION BY relay_id ORDER BY minute DESC,category) AS position FROM relay_reliability_events WHERE last_at>=?) WHERE position<=50 ORDER BY last_at DESC,category")
            .bind(now - 604800).fetch_all(&mut *tx).await?;
        for (id, category, count, first_at, last_at) in history {
            result
                .entry(id)
                .or_insert_with(|| RelayReliability::new(start))
                .history
                .push(ReliabilityEvent {
                    message: description(&category),
                    category,
                    count,
                    first_at,
                    last_at,
                });
        }
        tx.commit().await?;
        Ok(result)
    }
}
