//! Small independent retry lane. Only verified event IDs already present in the
//! response cache enter here; payloads and private keys never enter the queue.
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};
#[derive(Default)]
pub(crate) struct ReplayQueue {
    pending: VecDeque<(String, String, String)>,
    active: HashSet<String>,
    clients: HashMap<String, usize>,
    grants: HashMap<String, usize>,
    window: Option<Instant>,
    admitted: usize,
}
impl ReplayQueue {
    pub fn enqueue(
        &mut self,
        id: String,
        peer: String,
        grant: String,
        now: Instant,
    ) -> &'static str {
        if self.active.contains(&id) {
            return "retry_coalesced";
        }
        if self
            .window
            .is_none_or(|at| now.duration_since(at) >= Duration::from_secs(1))
        {
            self.window = Some(now);
            self.admitted = 0;
            self.clients.clear();
            self.grants.clear();
        }
        if self.active.len() >= 256
            || self.admitted >= 128
            || self.clients.get(&peer).copied().unwrap_or(0) >= 32
            || self.grants.get(&grant).copied().unwrap_or(0) >= 64
        {
            return "retry_throttled";
        }
        self.admitted += 1;
        *self.clients.entry(peer.clone()).or_default() += 1;
        *self.grants.entry(grant.clone()).or_default() += 1;
        self.active.insert(id.clone());
        self.pending.push_back((id, peer, grant));
        "cached_retry"
    }
    pub fn pop(&mut self, _now: Instant) -> Option<String> {
        self.pending.pop_front().map(|(id, _, _)| id)
    }
    pub fn finish(&mut self, id: &str) {
        self.active.remove(id);
    }
    pub fn clear(&mut self) {
        self.active.clear();
        self.pending.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backlog_coalesces_and_has_independent_bounded_budget() {
        let mut queue = ReplayQueue::default();
        let now = Instant::now();
        for n in 0..32 {
            assert_eq!(
                queue.enqueue(n.to_string(), "client".into(), "grant".into(), now),
                "cached_retry"
            );
        }
        assert_eq!(
            queue.enqueue("0".into(), "client".into(), "grant".into(), now),
            "retry_coalesced"
        );
        assert_eq!(
            queue.enqueue("excess".into(), "client".into(), "grant".into(), now),
            "retry_throttled"
        );
        assert_eq!(
            queue.enqueue("other".into(), "other".into(), "other".into(), now),
            "cached_retry"
        );
        assert_eq!(queue.pop(now).unwrap(), "0");
        queue.finish("0");
        assert_eq!(
            queue.enqueue(
                "0".into(),
                "client".into(),
                "grant".into(),
                now + Duration::from_secs(2)
            ),
            "cached_retry"
        );
        let admission = crate::admission::Admission::default();
        assert!(admission.try_admit("client", "grant").is_some());
    }
}
