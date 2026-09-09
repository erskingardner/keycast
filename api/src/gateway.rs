use crate::state::KeycastState;
use axum::{
    body::{to_bytes, Body, HttpBody},
    extract::State,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use keycast_core::v2::{
    control::{ControlRequest, ControlResponse},
    management::MAX_HTTP_BODY,
};

pub async fn forward(State(state): State<KeycastState>, request: Request<Body>) -> Response {
    static ADMISSION: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(32);
    // Bound slow uploads separately so they cannot reserve signer round trips.
    // Peer identity comes only from the socket; forwarded headers are never trusted.
    let peer = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|info| info.0.ip());
    let (parts, body) = request.into_parts();
    if body.size_hint().lower() > MAX_HTTP_BODY as u64 {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let upload = if body.is_end_stream() {
        None
    } else {
        match UploadPermit::acquire(peer) {
            Some(permit) => Some(permit),
            None => return StatusCode::TOO_MANY_REQUESTS.into_response(),
        }
    };
    let bytes = match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        to_bytes(body, MAX_HTTP_BODY),
    )
    .await
    {
        Ok(Ok(b)) => b,
        Ok(Err(_)) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        Err(_) => return StatusCode::REQUEST_TIMEOUT.into_response(),
    };
    drop(upload);
    let Ok(_permit) = ADMISSION.try_acquire() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(body) = String::from_utf8(bytes.to_vec()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let request = ControlRequest::Http {
        method: parts.method.to_string(),
        path: parts.uri.to_string(),
        authorization: parts
            .headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .map(str::to_owned),
        // A browser key import travels through this body; erase it on drop.
        body: keycast_core::v2::secret::Secret::new(body),
    };
    match state.signer.request(&request).await {
        Ok(ControlResponse::Http { reply }) => Response::builder()
            .status(reply.status)
            .header("content-type", "application/json")
            .header("cache-control", "no-store")
            .body(Body::from(reply.body))
            .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response()),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

static UPLOADS: std::sync::Mutex<Vec<(Option<std::net::IpAddr>, usize)>> =
    std::sync::Mutex::new(Vec::new());
struct UploadPermit(Option<std::net::IpAddr>);
impl UploadPermit {
    fn acquire(peer: Option<std::net::IpAddr>) -> Option<Self> {
        let mut uploads = UPLOADS.lock().unwrap_or_else(|e| e.into_inner());
        if uploads.iter().map(|(_, count)| count).sum::<usize>() >= 64 {
            return None;
        }
        if let Some((_, count)) = uploads.iter_mut().find(|(ip, _)| *ip == peer) {
            if *count >= 4 {
                return None;
            }
            *count += 1;
        } else {
            uploads.push((peer, 1));
        }
        Some(Self(peer))
    }
}
impl Drop for UploadPermit {
    fn drop(&mut self) {
        let mut uploads = UPLOADS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(position) = uploads.iter().position(|(ip, _)| *ip == self.0) {
            uploads[position].1 -= 1;
            if uploads[position].1 == 0 {
                uploads.swap_remove(position);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn slow_uploads_do_not_reserve_signer_calls_and_timeout_releases_their_budget() {
        let path = std::env::temp_dir().join(format!(
            "keycast-api-test-{}-{}.sock",
            std::process::id(),
            chrono_nonce()
        ));
        let socket = tokio::net::UnixListener::bind(&path).unwrap();
        let signer = tokio::spawn(async move {
            loop {
                let (mut stream, _) = socket.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut input = Vec::new();
                    stream.read_to_end(&mut input).await.unwrap();
                    let reply = ControlResponse::Http {
                        reply: keycast_core::v2::control::HttpReply {
                            status: 200,
                            body: "{}".into(),
                        },
                    };
                    stream
                        .write_all(&serde_json::to_vec(&reply).unwrap())
                        .await
                        .unwrap();
                });
            }
        });
        let state = KeycastState {
            signer: crate::state::SignerClient::new(path.clone()),
        };
        let app = axum::Router::new().fallback(forward).with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap()
        });
        let mut slow = Vec::new();
        for _ in 0..4 {
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            stream.write_all(b"POST /teams HTTP/1.1\r\nHost: test\r\nContent-Length: 1000\r\nConnection: close\r\n\r\nx").await.unwrap();
            slow.push(stream);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let mut read = tokio::net::TcpStream::connect(address).await.unwrap();
        read.write_all(b"GET /config HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            read.read_to_end(&mut response),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200"));
        for stream in &mut slow {
            let mut response = Vec::new();
            tokio::time::timeout(
                std::time::Duration::from_secs(4),
                stream.read_to_end(&mut response),
            )
            .await
            .unwrap()
            .unwrap();
            assert!(response.starts_with(b"HTTP/1.1 408"));
        }
        assert!(UPLOADS.lock().unwrap().is_empty());
        server.abort();
        signer.abort();
        let _ = server.await;
        let _ = signer.await;
        std::fs::remove_file(path).unwrap();
    }
    fn chrono_nonce() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
