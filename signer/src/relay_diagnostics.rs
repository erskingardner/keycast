use nostr_sdk::prelude::Relay;
use serde::Serialize;
use std::collections::VecDeque;

const HISTORY_LIMIT: usize = 32;

pub fn transport_reason(error: &str) -> &'static str {
    if error.contains("HTTP error: 503") {
        "HTTP 503: relay temporarily unavailable"
    } else if error.contains("HTTP error: 502") {
        "HTTP 502: relay gateway error"
    } else if error.contains("HTTP error: 429") {
        "HTTP 429: relay rate limit"
    } else if error.contains("HTTP error: 403") {
        "HTTP 403: relay refused access"
    } else if error.contains("HTTP error: 401") {
        "HTTP 401: relay requires authentication"
    } else if error.contains("HTTP error: 404") {
        "HTTP 404: relay endpoint not found"
    } else if error.to_lowercase().contains("certificate") || error.contains("TLS") {
        "TLS or certificate validation failed"
    } else if error.to_lowercase().contains("dns") || error.contains("resolve") {
        "DNS lookup failed"
    } else if error.to_lowercase().contains("timed out") || error.to_lowercase().contains("timeout")
    {
        "Connection timed out"
    } else if error.to_lowercase().contains("refused") {
        "Connection refused"
    } else {
        "WebSocket transport failed (unclassified error)"
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RelayLogEntry {
    pub occurred_at: i64,
    pub level: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RelayDiagnostics {
    pub connection: String,
    pub subscription: &'static str,
    pub observed_at: i64,
    pub connected_at: Option<i64>,
    pub attempts: usize,
    pub successes: usize,
    pub bytes_sent: usize,
    pub bytes_received: usize,
    pub latency_ms: Option<u64>,
    pub subscription_error: Option<&'static str>,
    pub transport_error: Option<&'static str>,
    pub history: VecDeque<RelayLogEntry>,
}

impl Default for RelayDiagnostics {
    fn default() -> Self {
        Self {
            connection: "unknown".into(),
            subscription: "idle",
            observed_at: 0,
            connected_at: None,
            attempts: 0,
            successes: 0,
            bytes_sent: 0,
            bytes_received: 0,
            latency_ms: None,
            subscription_error: None,
            transport_error: None,
            history: VecDeque::new(),
        }
    }
}

impl RelayDiagnostics {
    pub fn observe(&mut self, relay: &Relay, has_grants: bool, accepted: bool, now: i64) {
        let connection = relay.status().to_string().to_lowercase();
        let stats = relay.stats();
        if self.connection != connection
            || self.attempts != stats.attempts()
            || self.successes != stats.success()
        {
            self.log(
                now,
                if connection == "disconnected" {
                    "warning"
                } else {
                    "info"
                },
                format!(
                    "Connection {connection}; {} attempts, {} successful connections",
                    stats.attempts(),
                    stats.success()
                ),
            );
        }
        self.connection = connection;
        if self.connection == "connected" {
            self.transport_error = None;
        }
        self.observed_at = now;
        let connected_at = stats.connected_at().as_secs() as i64;
        self.connected_at = (connected_at > 0).then_some(connected_at);
        self.attempts = stats.attempts();
        self.successes = stats.success();
        self.bytes_sent = stats.bytes_sent();
        self.bytes_received = stats.bytes_received();
        self.latency_ms = stats.latency().map(|value| value.as_millis() as u64);
        if !has_grants {
            self.subscription("idle", None, now);
        } else if accepted {
            self.subscription("accepted", None, now);
        } else if self.subscription != "rejected" {
            self.subscription("pending", None, now);
        }
    }

    pub fn subscription(&mut self, status: &'static str, error: Option<&'static str>, now: i64) {
        if self.subscription != status || self.subscription_error != error {
            self.log(
                now,
                if error.is_some() { "warning" } else { "info" },
                format!(
                    "Signing subscription {status}{}",
                    error.map(|value| format!(": {value}")).unwrap_or_default()
                ),
            );
        }
        self.subscription = status;
        self.subscription_error = error;
    }

    pub fn log(&mut self, now: i64, level: &'static str, message: String) {
        // Coalesce repeated notices; never store relay-provided text or event payloads here.
        if self
            .history
            .back()
            .is_some_and(|entry| entry.message == message && now - entry.occurred_at < 60)
        {
            return;
        }
        self.history.push_back(RelayLogEntry {
            occurred_at: now,
            level,
            message,
        });
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop_front();
        }
    }
}

/// Extract only protocol-defined categories. Relay text may contain keys or echoed payloads.
pub fn rejection_reason(message: &str) -> &'static str {
    let prefix = message
        .split_once(':')
        .map(|(prefix, _)| prefix)
        .unwrap_or("");
    match prefix {
        "auth-required" => "Relay requires NIP-42 authentication",
        "restricted" => "Relay restricts access",
        "rate-limited" => "Relay rate limit reached",
        "pow" => "Relay requires proof of work",
        "payment-required" => "Relay requires payment",
        "blocked" => "Relay blocked the request",
        "invalid" => "Relay rejected the message format",
        "error" => "Relay reported an internal error",
        _ => "Relay rejected the request (unclassified reason)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_bounded_and_rejections_never_echo_untrusted_text() {
        let mut diagnostics = RelayDiagnostics::default();
        for i in 0..100 {
            diagnostics.log(i, "info", format!("State {i}"));
        }
        assert_eq!(diagnostics.history.len(), 32);
        assert_eq!(
            rejection_reason("auth-required: nsec1secret"),
            "Relay requires NIP-42 authentication"
        );
        assert!(!rejection_reason("nsec1secret").contains("nsec"));
        diagnostics.subscription(
            "rejected",
            Some(rejection_reason("restricted: secret")),
            101,
        );
        diagnostics.subscription("idle", None, 102);
        assert!(diagnostics.subscription_error.is_none());
        assert_eq!(diagnostics.subscription, "idle");
        assert_eq!(
            transport_reason("HTTP error: 503 Service Unavailable: nsec1secret"),
            "HTTP 503: relay temporarily unavailable"
        );
        assert!(!transport_reason("unknown nsec1secret").contains("nsec"));
    }
}
