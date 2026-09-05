//! Bounded admission shared by live relay input and durable-inbox recovery.
//! A grant has one stable recipient key, so rotating client keys cannot evade its budget.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

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
    fn full(&self, client: &str, grant: &str, limits: (usize, usize, usize)) -> bool {
        self.total >= limits.0
            || self.clients.get(client).copied().unwrap_or(0) >= limits.1
            || self.grants.get(grant).copied().unwrap_or(0) >= limits.2
    }
}
#[derive(Default)]
struct State {
    second: u64,
    rate: Counts,
    running: Counts,
}
#[derive(Clone)]
pub(crate) struct Admission {
    start: Instant,
    state: Arc<Mutex<State>>,
}
impl Default for Admission {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            state: Arc::default(),
        }
    }
}
impl Admission {
    pub fn try_admit(&self, client: &str, grant: &str) -> Option<Ticket> {
        self.try_admit_at(client, grant, self.start.elapsed().as_secs())
    }
    fn try_admit_at(&self, client: &str, grant: &str, second: u64) -> Option<Ticket> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if second != state.second {
            state.second = second;
            state.rate = Counts::default();
        }
        // Limits also bound map cardinality; rejected identities are never inserted.
        if state.running.full(client, grant, (32, 2, 8))
            || state.rate.full(client, grant, (128, 16, 32))
        {
            return None;
        }
        state.running.increment(client, grant);
        state.rate.increment(client, grant);
        Some(Ticket {
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
