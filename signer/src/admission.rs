//! Bounded admission shared by live relay input and durable-inbox recovery.
//! A grant has one stable recipient key, so rotating client keys cannot evade its budget.
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Remember relay copies, not request payloads. A new relay's first copy of an
/// already admitted event is redundant; another copy from the same relay may be
/// a client retry and must still reach the durable response cache.
#[derive(Default)]
pub(crate) struct RelayCopies {
    events: HashMap<String, HashSet<String>>,
    order: VecDeque<(String, Instant)>,
}
impl RelayCopies {
    const CAPACITY: usize = 4096;
    const MAX_RELAYS: usize = 20;
    // Coalesce fanout without blocking a retry via a different relay after a
    // lost response. Same-relay retries are never suppressed by this cache.
    const TTL: Duration = Duration::from_secs(2);

    pub fn redundant(&mut self, id: &str, relay: &str, now: Instant) -> bool {
        self.expire(now);
        if let Some(relays) = self.events.get_mut(id) {
            if !relays.contains(relay) && relays.len() < Self::MAX_RELAYS {
                relays.insert(relay.to_owned());
                return true;
            }
        }
        false
    }

    /// Call only after admission succeeds. Dropped input must remain eligible
    /// when another relay delivers it later.
    pub fn admitted(&mut self, id: &str, relay: &str, now: Instant) {
        self.expire(now);
        if self.events.contains_key(id) {
            return;
        }
        if self.events.len() == Self::CAPACITY {
            if let Some((old, _)) = self.order.pop_front() {
                self.events.remove(&old);
            }
        }
        self.events
            .insert(id.to_owned(), HashSet::from([relay.to_owned()]));
        self.order.push_back((id.to_owned(), now));
    }

    fn expire(&mut self, now: Instant) {
        while self
            .order
            .front()
            .is_some_and(|(_, at)| now.duration_since(*at) >= Self::TTL)
        {
            let (id, _) = self.order.pop_front().unwrap();
            self.events.remove(&id);
        }
    }

    pub fn forget(&mut self, id: &str) {
        self.events.remove(id);
        self.order.retain(|(stored, _)| stored != id);
    }
}

#[derive(Default)]
struct Counts {
    total: usize,
    clients: HashMap<String, usize>,
    grants: HashMap<String, usize>,
}
impl Counts {
    fn increment(&mut self, client: &str, grant: &str) {
        self.total += 1;
        *self.clients.entry(client.to_owned()).or_default() += 1;
        *self.grants.entry(grant.to_owned()).or_default() += 1;
    }
}
#[derive(Default)]
struct State {
    second: u64,
    rate: Counts,
    running: Counts,
}
/// Global, per-client and per-grant ceilings for one admission lane.
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    running: (usize, usize, usize),
    rate: (usize, usize, usize),
}
impl Limits {
    /// Clients holding a live session on the target grant.
    pub const ESTABLISHED: Self = Self {
        running: (32, 2, 8),
        rate: (128, 16, 32),
    };
    /// Everything else, including `connect` and forged traffic. A grant's
    /// remote-signer key is public, so anyone can publish events addressed to it;
    /// keeping that traffic in its own lane means a flood cannot consume the
    /// budget that established sessions draw from.
    pub const NEWCOMER: Self = Self {
        running: (8, 2, 4),
        rate: (32, 4, 8),
    };
}

#[derive(Clone)]
pub(crate) struct Admission {
    start: Instant,
    limits: Limits,
    state: Arc<Mutex<State>>,
}
impl Default for Admission {
    fn default() -> Self {
        Self::new(Limits::ESTABLISHED)
    }
}
impl Admission {
    pub fn new(limits: Limits) -> Self {
        Self {
            start: Instant::now(),
            limits,
            state: Arc::default(),
        }
    }
    pub fn try_admit(&self, client: &str, grant: &str) -> Option<Ticket> {
        self.try_admit_reason(client, grant).ok()
    }
    pub fn try_admit_reason(&self, client: &str, grant: &str) -> Result<Ticket, &'static str> {
        self.admit_at(client, grant, self.start.elapsed().as_secs())
    }
    #[cfg(test)]
    fn try_admit_at(&self, client: &str, grant: &str, second: u64) -> Option<Ticket> {
        self.admit_at(client, grant, second).ok()
    }
    fn admit_at(&self, client: &str, grant: &str, second: u64) -> Result<Ticket, &'static str> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if second != state.second {
            state.second = second;
            state.rate = Counts::default();
        }
        // Limits also bound map cardinality; rejected identities are never inserted.
        for (counts, limits, reasons) in [
            (
                &state.running,
                self.limits.running,
                [
                    "admission_running_global",
                    "admission_running_client",
                    "admission_running_grant",
                ],
            ),
            (
                &state.rate,
                self.limits.rate,
                [
                    "admission_rate_global",
                    "admission_rate_client",
                    "admission_rate_grant",
                ],
            ),
        ] {
            if counts.total >= limits.0 {
                return Err(reasons[0]);
            }
            if counts.clients.get(client).copied().unwrap_or(0) >= limits.1 {
                return Err(reasons[1]);
            }
            if counts.grants.get(grant).copied().unwrap_or(0) >= limits.2 {
                return Err(reasons[2]);
            }
        }
        state.running.increment(client, grant);
        state.rate.increment(client, grant);
        Ok(Ticket {
            state: self.state.clone(),
            client: client.to_owned(),
            grant: grant.to_owned(),
        })
    }
}
/// Owned by the worker: completion, cancellation and unwind all return capacity.
pub(crate) struct Ticket {
    state: Arc<Mutex<State>>,
    client: String,
    grant: String,
}
impl Drop for Ticket {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.running.total -= 1;
        let running = &mut state.running;
        for (map, key) in [
            (&mut running.clients, &self.client),
            (&mut running.grants, &self.grant),
        ] {
            let count = map.get_mut(key).expect("admitted worker");
            *count -= 1;
            if *count == 0 {
                map.remove(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_fanout_does_not_spend_the_unique_request_budget() {
        let admission = Admission::default();
        let mut copies = RelayCopies::default();
        let now = Instant::now();
        for n in 0..16 {
            let id = format!("request-{n}");
            assert!(!copies.redundant(&id, "first", now));
            drop(admission.try_admit_at("client", "grant", 0).unwrap());
            copies.admitted(&id, "first", now);
            for relay in ["second", "third"] {
                assert!(copies.redundant(&id, relay, now));
            }
        }
        assert!(admission.try_admit_at("client", "grant", 0).is_none());
        // Preserve client retransmissions, including failover to another relay.
        assert!(!copies.redundant("request-0", "first", now));
        assert!(!copies.redundant("request-0", "second", now));
        assert!(!copies.redundant("request-0", "fourth", now + RelayCopies::TTL));
    }

    #[test]
    fn relay_copy_cache_is_bounded_and_failed_admission_is_retryable() {
        let now = Instant::now();
        let mut copies = RelayCopies::default();
        assert!(!copies.redundant("rejected", "first", now));
        assert!(!copies.redundant("rejected", "second", now));
        for n in 0..RelayCopies::CAPACITY + 10 {
            copies.admitted(&format!("{n}"), "first", now);
        }
        assert_eq!(copies.events.len(), RelayCopies::CAPACITY);
        assert_eq!(copies.order.len(), RelayCopies::CAPACITY);
        assert!(!copies.redundant("0", "second", now));
        copies.forget("10");
        assert!(!copies.redundant("10", "second", now));
        copies.redundant("expired", "first", now + RelayCopies::TTL);
        assert!(copies.events.is_empty());
        assert!(copies.order.is_empty());
    }

    #[test]
    fn rotating_clients_cannot_starve_another_grant() {
        let admission = Admission::default();
        let held: Vec<_> = (0..8)
            .map(|n| admission.try_admit_at(&n.to_string(), "noisy", 0).unwrap())
            .collect();
        assert!(admission.try_admit_at("new-identity", "noisy", 0).is_none());
        assert!(admission.try_admit_at("legitimate", "quiet", 0).is_some());
        drop(held);
        assert!(admission.try_admit_at("new-identity", "noisy", 0).is_some());
    }

    #[test]
    fn completed_work_still_consumes_rate_and_windows_reset() {
        let admission = Admission::default();
        for _ in 0..16 {
            drop(admission.try_admit_at("client", "grant", 0).unwrap());
        }
        assert!(admission.try_admit_at("client", "grant", 0).is_none());
        for _ in 0..16 {
            drop(admission.try_admit_at("other", "grant", 0).unwrap());
        }
        assert!(admission.try_admit_at("third", "grant", 0).is_none());
        assert!(admission.try_admit_at("third", "different", 0).is_some());
        assert!(admission.try_admit_at("client", "grant", 1).is_some());
    }

    #[tokio::test]
    async fn cancelled_workers_return_capacity_without_resetting_rate() {
        let admission = Admission::default();
        let first = admission.try_admit_at("client", "grant", 0).unwrap();
        let second = admission.try_admit_at("client", "grant", 0).unwrap();
        assert!(admission
            .try_admit_at("client", "another-grant", 0)
            .is_none());
        let worker = tokio::spawn(async move {
            let _tickets = (first, second);
            std::future::pending::<()>().await;
        });
        worker.abort();
        let _ = worker.await;
        assert!(admission.try_admit_at("client", "grant", 0).is_some());
        let state = admission.state.lock().unwrap();
        assert_eq!(state.running.total, 0);
        assert!(state.running.clients.is_empty());
        assert!(state.running.grants.is_empty());
        assert_eq!(state.rate.total, 3);
    }

    #[test]
    fn a_newcomer_flood_cannot_starve_established_sessions() {
        let established = Admission::new(Limits::ESTABLISHED);
        let newcomers = Admission::new(Limits::NEWCOMER);

        // Hold the tickets so concurrency actually accumulates. A dropped ticket
        // returns its slot at once, which would leave the rate limit as the only
        // constraint under test and make either constant look load-bearing.
        let held: Vec<_> = (0..512)
            .filter_map(|n| newcomers.try_admit_at(&format!("forged-{n}"), "victim-grant", 0))
            .collect();
        assert_eq!(held.len(), Limits::NEWCOMER.running.2);
        assert!(newcomers
            .try_admit_at("another", "victim-grant", 0)
            .is_none());

        // Completed work still spends the per-grant rate budget for the window.
        drop(held);
        let admitted = (0..512)
            .filter(|n| {
                newcomers
                    .try_admit_at(&format!("burst-{n}"), "other-grant", 1)
                    .is_some()
            })
            .count();
        assert_eq!(admitted, Limits::NEWCOMER.rate.2);

        // The established lane is untouched and still serves the real client.
        assert!(established
            .try_admit_at("live-client", "victim-grant", 0)
            .is_some());
    }

    #[test]
    fn global_limits_and_attacker_maps_are_bounded() {
        let admission = Admission::default();
        let held: Vec<_> = (0..32)
            .map(|n| {
                admission
                    .try_admit_at(&n.to_string(), &n.to_string(), 0)
                    .unwrap()
            })
            .collect();
        for n in 32..1000 {
            assert!(admission
                .try_admit_at(&n.to_string(), &n.to_string(), 0)
                .is_none());
        }
        drop(held);
        for n in 32..128 {
            drop(
                admission
                    .try_admit_at(&n.to_string(), &n.to_string(), 0)
                    .unwrap(),
            );
        }
        assert!(admission.try_admit_at("overflow", "overflow", 0).is_none());
        let state = admission.state.lock().unwrap();
        assert_eq!(state.rate.clients.len(), 128);
        assert_eq!(state.rate.grants.len(), 128);
    }
}
