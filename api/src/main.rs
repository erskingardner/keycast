use std::path::PathBuf;

use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::routing::get;
use axum::Router;
use keycast_api::state::{KeycastState, SignerClient};
use keycast_core::v2::control::{ControlRequest, ControlResponse};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!(
            "keycast_api {} ({})",
            keycast_core::VERSION,
            keycast_core::BUILD_REVISION
        );
        return Ok(());
    }
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return api_healthcheck().await;
    }

    let socket_path = std::env::var_os("KEYCAST_SIGNER_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/run/keycast/signer.sock"));
    let state = KeycastState {
        signer: SignerClient::new(socket_path),
    };

    let readiness_state = state.clone();
    let mut app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/api/health", get(|| async { StatusCode::OK }))
        .route(
            "/ready",
            get(move || {
                let state = readiness_state.clone();
                async move { readiness(&state).await }
            }),
        )
        .nest(
            "/api",
            Router::new()
                .fallback(keycast_api::gateway::forward)
                .with_state(state),
        )
        .layer(TraceLayer::new_for_http());

    if let Some(cors) = cors_layer()? {
        app = app.layer(cors);
    }

    let bind = std::env::var("KEYCAST_API_BIND").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(address = %listener.local_addr()?, "Keycast API ready");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn api_healthcheck() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::var("KEYCAST_API_HEALTH_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let operation = async {
        let mut stream = tokio::net::TcpStream::connect(address).await?;
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await?;
        let mut response = Vec::new();
        (&mut stream).take(4096).read_to_end(&mut response).await?;
        if response.starts_with(b"HTTP/1.1 200") || response.starts_with(b"HTTP/1.0 200") {
            Ok(())
        } else {
            Err(std::io::Error::other("API is not ready"))
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), operation)
        .await
        .map_err(|_| std::io::Error::other("API healthcheck timed out"))??;
    Ok(())
}

async fn readiness(state: &KeycastState) -> StatusCode {
    match state.signer.request(&ControlRequest::Status).await {
        Ok(ControlResponse::Status { status }) if status.ready => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

fn cors_layer() -> Result<Option<CorsLayer>, Box<dyn std::error::Error>> {
    let raw = std::env::var("KEYCAST_ALLOWED_ORIGINS").unwrap_or_default();
    let origins: Vec<HeaderValue> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    if origins.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT]),
    ))
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = terminate.recv() => {},
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
