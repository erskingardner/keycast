//! Observe the SDK's default transport without changing TLS, proxy, or wire behavior.
use async_wsocket::{
    futures_util::{Sink, Stream},
    Message,
};
use nostr::prelude::{Timestamp, Url};
use nostr_sdk::{
    error::Error,
    transport::websocket::{
        DefaultWebsocketTransport, WebSocketSink, WebSocketStream, WebSocketTransport,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
};

#[derive(Clone, Debug)]
pub struct Observation {
    pub url: String,
    pub category: &'static str,
    pub count: i64,
    pub first_at: i64,
    pub last_at: i64,
}

#[derive(Debug, Default)]
struct Buffer {
    configured: BTreeSet<String>,
    restricted: BTreeSet<String>,
    attempted: BTreeSet<String>,
    pending: BTreeMap<(String, i64, &'static str), Observation>,
    dropped: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Default)]
pub struct RelayTelemetry(Arc<Mutex<Buffer>>);
impl RelayTelemetry {
    pub fn restrict(&self, urls: Vec<String>) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).restricted = urls
            .into_iter()
            .map(|s| s.trim_end_matches('/').to_owned())
            .collect();
    }
    pub fn configure(&self, urls: &[String]) {
        let mut b = self.0.lock().unwrap_or_else(|e| e.into_inner());
        b.configured = urls
            .iter()
            .take(20)
            .map(|u| u.trim_end_matches('/').to_owned())
            .collect();
        let configured = b.configured.clone();
        b.attempted.retain(|url| configured.contains(url));
    }
    pub fn record(&self, url: &str, category: &'static str) {
        self.record_at(url, category, Timestamp::now().as_secs() as i64);
    }
    fn record_at(&self, url: &str, category: &'static str, now: i64) {
        let url = url.trim_end_matches('/');
        let mut b = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !b.configured.contains(url) {
            return;
        }
        let key = (url.to_owned(), now / 60, category);
        if let Some(event) = b.pending.get_mut(&key) {
            event.count = event.count.saturating_add(1);
            event.first_at = event.first_at.min(now);
            event.last_at = event.last_at.max(now);
        } else if b.pending.len() < 2048 {
            b.pending.insert(
                key,
                Observation {
                    url: url.to_owned(),
                    category,
                    count: 1,
                    first_at: now,
                    last_at: now,
                },
            );
        } else {
            let count = b.dropped.entry(url.to_owned()).or_default();
            *count = count.saturating_add(1);
        }
    }
    fn attempt(&self, url: &str) {
        let retry = {
            let mut b = self.0.lock().unwrap_or_else(|e| e.into_inner());
            b.configured.contains(url.trim_end_matches('/'))
                && !b.attempted.insert(url.trim_end_matches('/').to_owned())
        };
        self.record(url, "attempt");
        if retry {
            self.record(url, "retry");
        }
    }
    pub fn drain(&self) -> Vec<Observation> {
        let mut b = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let mut events: Vec<_> = std::mem::take(&mut b.pending).into_values().collect();
        let now = Timestamp::now().as_secs() as i64;
        for (url, count) in std::mem::take(&mut b.dropped) {
            events.push(Observation {
                url,
                category: "telemetry_dropped",
                count,
                first_at: now,
                last_at: now,
            });
        }
        events
    }
}

/// Finite categories keep untrusted text out of durable history and bound aggregation cardinality.
pub fn error_category(error: &str) -> &'static str {
    match crate::relay_diagnostics::transport_reason(error) {
        "HTTP 503: relay temporarily unavailable" => "error_http_503",
        "HTTP 502: relay gateway error" => "error_http_502",
        "HTTP 429: relay rate limit" => "error_http_429",
        "HTTP 403: relay refused access" => "error_http_403",
        "HTTP 401: relay requires authentication" => "error_http_401",
        "HTTP 404: relay endpoint not found" => "error_http_404",
        "TLS or certificate validation failed" => "error_tls",
        "DNS lookup failed" => "error_dns",
        "Connection timed out" => "error_timeout",
        "Connection refused" => "error_refused",
        _ => "error_transport",
    }
}

pub fn description(category: &str) -> &'static str {
    match category {
        "cached_retry" => "Cached response retry queued",
        "retry_coalesced" => "Repeated cached response delivery coalesced",
        "retry_throttled" => "Cached response retry budget full",
        "admission_running_global" => "Fresh request rejected: instance workers full",
        "admission_running_client" => "Fresh request rejected: client workers full",
        "admission_running_grant" => "Fresh request rejected: grant workers full",
        "admission_rate_global" => "Fresh request rejected: instance rate limit",
        "admission_rate_client" => "Fresh request rejected: client rate limit",
        "admission_rate_grant" => "Fresh request rejected: grant rate limit",
        "attempt" => "Connection attempt",
        "retry" => "Reconnection attempt",
        "connected" => "WebSocket connected",
        "attempt_cancelled" => "Connection attempt cancelled or timed out",
        "peer_close_1000" => "Relay closed the connection: 1000 (normal closure)",
        "peer_close_1001" => "Relay closed the connection: 1001 (going away)",
        "peer_close_1008" => "Relay closed the connection: 1008 (policy violation)",
        "peer_close_1011" => "Relay closed the connection: 1011 (server error)",
        "peer_close_1012" => "Relay closed the connection: 1012 (service restart)",
        "peer_close_1013" => "Relay closed the connection: 1013 (try again later)",
        "peer_close_other" => "Relay sent a WebSocket close frame (other or no code)",
        "connection_lost" => {
            "Connection ended without a close frame (network or transport failure)"
        }
        "error_http_503" => "HTTP 503: relay temporarily unavailable",
        "error_http_502" => "HTTP 502: relay gateway error",
        "error_http_429" => "HTTP 429: relay rate limit",
        "error_http_403" => "HTTP 403: relay refused access",
        "error_http_401" => "HTTP 401: relay requires authentication",
        "error_http_404" => "HTTP 404: relay endpoint not found",
        "error_tls" => "TLS or certificate validation failed",
        "error_dns" => "DNS lookup failed",
        "error_timeout" => "Connection timed out",
        "error_refused" => "Connection refused",
        "error_transport" => "WebSocket transport failed (unclassified error)",
        "error_subscription_auth" => "Signing subscription rejected: authentication required",
        "error_subscription_rate" => "Signing subscription rejected: rate limit",
        "error_subscription_other" => "Signing subscription rejected",
        "error_publication" => "Relay rejected a publication",
        "auth_required" => "Relay requested NIP-42 authentication",
        "subscription_accepted" => "Signing subscription accepted",
        "telemetry_dropped" => "Diagnostic buffer overflow; some observations were not counted",
        _ => "Relay diagnostic event",
    }
}

#[derive(Debug, Clone)]
pub struct ObservedTransport(pub RelayTelemetry);
impl WebSocketTransport for ObservedTransport {
    fn support_ping(&self) -> bool {
        DefaultWebsocketTransport.support_ping()
    }
    fn connect<'a>(
        &'a self,
        url: &'a Url,
        proxy: Option<SocketAddr>,
    ) -> Pin<Box<dyn Future<Output = Result<(WebSocketSink, WebSocketStream), Error>> + Send + 'a>>
    {
        Box::pin(async move {
            self.0.attempt(url.as_str());
            let mut attempt = Attempt {
                telemetry: self.0.clone(),
                url: url.to_string(),
                finished: false,
            };
            let restricted = self
                .0
                 .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .restricted
                .contains(url.as_str().trim_end_matches('/'));
            let result = if restricted {
                if proxy.is_some() {
                    Err(Error::policy("discovered relay proxy forbidden"))
                } else {
                    crate::public_relay::connect(url).await
                }
            } else {
                DefaultWebsocketTransport.connect(url, proxy).await
            };
            attempt.finished = true;
            match result {
                Ok((sink, stream)) => {
                    self.0.record(url.as_str(), "connected");
                    let state = Arc::new(Connection {
                        telemetry: self.0.clone(),
                        url: url.to_string(),
                        ended: AtomicBool::new(false),
                        local_close: AtomicBool::new(false),
                    });
                    Ok((
                        Box::pin(ObservedSink {
                            inner: sink,
                            state: state.clone(),
                        }) as WebSocketSink,
                        Box::pin(ObservedStream {
                            inner: stream,
                            state,
                        }) as WebSocketStream,
                    ))
                }
                Err(error) => {
                    self.0
                        .record(url.as_str(), error_category(&error.to_string()));
                    Err(error)
                }
            }
        })
    }
}
struct Attempt {
    telemetry: RelayTelemetry,
    url: String,
    finished: bool,
}
impl Drop for Attempt {
    fn drop(&mut self) {
        if !self.finished {
            self.telemetry.record(&self.url, "attempt_cancelled");
        }
    }
}
struct Connection {
    telemetry: RelayTelemetry,
    url: String,
    ended: AtomicBool,
    local_close: AtomicBool,
}
impl Connection {
    fn end(&self, category: &'static str, error: Option<&Error>) {
        if !self.local_close.load(Ordering::Relaxed) && !self.ended.swap(true, Ordering::Relaxed) {
            self.telemetry.record(&self.url, category);
            if let Some(error) = error {
                self.telemetry
                    .record(&self.url, error_category(&error.to_string()));
            }
        }
    }
    fn check<T>(&self, result: &Poll<Result<T, Error>>) {
        if let Poll::Ready(Err(error)) = result {
            self.end("connection_lost", Some(error));
        }
    }
}
struct ObservedStream {
    inner: WebSocketStream,
    state: Arc<Connection>,
}
impl Stream for ObservedStream {
    type Item = Result<Message, Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = self.inner.as_mut().poll_next(cx);
        match &result {
            Poll::Ready(Some(Ok(Message::Close(frame)))) => self.state.end(
                match frame.as_ref().map(|f| f.code) {
                    Some(1000) => "peer_close_1000",
                    Some(1001) => "peer_close_1001",
                    Some(1008) => "peer_close_1008",
                    Some(1011) => "peer_close_1011",
                    Some(1012) => "peer_close_1012",
                    Some(1013) => "peer_close_1013",
                    _ => "peer_close_other",
                },
                None,
            ),
            Poll::Ready(Some(Err(error))) => self.state.end("connection_lost", Some(error)),
            Poll::Ready(None) => self.state.end("connection_lost", None),
            _ => {}
        }
        result
    }
}
struct ObservedSink {
    inner: WebSocketSink,
    state: Arc<Connection>,
}
impl Sink<Message> for ObservedSink {
    type Error = Error;
    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let result = self.inner.as_mut().poll_ready(cx);
        self.state.check(&result);
        result
    }
    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Error> {
        if matches!(item, Message::Close(_)) {
            self.state.local_close.store(true, Ordering::Relaxed);
        }
        let result = self.inner.as_mut().start_send(item);
        if let Err(error) = &result {
            self.state.end("connection_lost", Some(error));
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let result = self.inner.as_mut().poll_flush(cx);
        self.state.check(&result);
        result
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.state.local_close.store(true, Ordering::Relaxed);
        self.inner.as_mut().poll_close(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_wsocket::futures_util::{SinkExt, StreamExt};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    fn count(events: &[Observation], category: &str) -> i64 {
        events
            .iter()
            .filter(|e| e.category == category)
            .map(|e| e.count)
            .sum()
    }

    #[tokio::test]
    async fn real_http_503s_count_each_attempt_and_retry_without_storing_response_text() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("ws://{}", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![];
                while !request.ends_with(b"\r\n\r\n") {
                    request.push(stream.read_u8().await.unwrap());
                    assert!(request.len() < 8192);
                }
                stream
                    .write_all(
                        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 6\r\n\r\nsecret",
                    )
                    .await
                    .unwrap();
            }
        });
        let telemetry = RelayTelemetry::default();
        telemetry.configure(&[url.to_string()]);
        let transport = ObservedTransport(telemetry.clone());
        for _ in 0..3 {
            assert!(transport.connect(&url, None).await.is_err());
        }
        server.await.unwrap();
        let events = telemetry.drain();
        assert_eq!(count(&events, "attempt"), 3);
        assert_eq!(count(&events, "retry"), 2);
        assert_eq!(count(&events, "error_http_503"), 3);
        assert_eq!(count(&events, "connected"), 0);
        assert!(!format!("{events:?}").contains("secret"));
    }

    #[tokio::test]
    async fn real_remote_close_is_distinct_from_local_shutdown_and_abrupt_loss() {
        for mode in ["remote", "local", "abrupt"] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = Url::parse(&format!("ws://{}", listener.local_addr().unwrap())).unwrap();
            let server = tokio::spawn(async move {
                let (tcp, _) = listener.accept().await.unwrap();
                let mut socket = async_wsocket::native::accept_async(tcp).await.unwrap();
                if mode == "remote" {
                    let frame = async_wsocket::message::CloseFrame {
                        code: 1013,
                        reason: "secret untrusted relay text".into(),
                    };
                    socket
                        .send(async_wsocket::native::Message::Close(Some(frame.into())))
                        .await
                        .unwrap();
                    let _ = socket.next().await;
                } else if mode == "local" {
                    let _ = socket.next().await;
                    let _ = socket.flush().await;
                }
            });
            let telemetry = RelayTelemetry::default();
            telemetry.configure(&[url.to_string()]);
            let (mut sink, mut stream) = ObservedTransport(telemetry.clone())
                .connect(&url, None)
                .await
                .unwrap();
            if mode == "local" {
                sink.close().await.unwrap();
            }
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            drop(stream);
            drop(sink);
            server.await.unwrap();
            let events = telemetry.drain();
            assert_eq!(
                count(&events, "peer_close_1013"),
                i64::from(mode == "remote")
            );
            assert_eq!(
                count(&events, "connection_lost"),
                i64::from(mode == "abrupt")
            );
            assert!(!format!("{events:?}").contains("secret"));
        }
    }

    #[test]
    fn repeated_errors_aggregate_and_overflow_is_bounded_and_visible() {
        let telemetry = RelayTelemetry::default();
        telemetry.configure(&["wss://test.example".into()]);
        for _ in 0..10_000 {
            telemetry.record_at("wss://test.example", "error_http_503", 60);
        }
        let events = telemetry.drain();
        assert_eq!(events.len(), 1);
        assert_eq!(count(&events, "error_http_503"), 10_000);
        for minute in 0..3000 {
            telemetry.record_at("wss://test.example", "attempt", minute * 60);
        }
        let events = telemetry.drain();
        assert_eq!(events.len(), 2049);
        assert_eq!(count(&events, "telemetry_dropped"), 952);
        assert!(telemetry.drain().is_empty());
    }
}
