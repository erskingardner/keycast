use crate::runtime::RuntimeState;
use keycast_core::v2::control::{ControlRequest, ControlResponse, HttpReply, LifecycleRequest};
use keycast_core::v2::management::{MAX_CONTROL_BYTES, MAX_HTTP_BODY};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{watch, Semaphore};
use tokio::task::JoinSet;
use tower::ServiceExt;
use zeroize::Zeroizing;

pub async fn serve_control_socket(
    state: RuntimeState,
    socket_path: PathBuf,
    public_url: url::Url,
    shutdown: watch::Receiver<bool>,
) -> Result<(), std::io::Error> {
    if let Some(parent) = socket_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if let Ok(metadata) = std::fs::symlink_metadata(&socket_path) {
        if !metadata.file_type().is_socket() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "control socket path exists and is not a socket",
            ));
        }
        std::fs::remove_file(&socket_path)?;
    }
    let listener = UnixListener::bind(&socket_path)?;
    if let Err(error) =
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o660))
    {
        // Docker Desktop bind mounts do not always implement chmod for Unix
        // sockets. The API and signer deliberately run as the same UID, so an
        // owner-writable, non-world-writable socket is still safe and usable.
        let mode = std::fs::symlink_metadata(&socket_path)?
            .permissions()
            .mode();
        if mode & 0o200 == 0 || mode & 0o002 != 0 {
            return Err(error);
        }
        tracing::warn!(
            path = %socket_path.display(),
            mode = format_args!("{:o}", mode & 0o777),
            error = %error,
            "filesystem rejected control socket chmod; retaining safe owner-only write access"
        );
    }
    tracing::info!(path = %socket_path.display(), "signer control socket ready");

    let router = crate::management::api::http::routes(crate::management::state::KeycastState {
        public_url: public_url.to_string(),
        db: state.store.pool.clone(),
        signer: crate::management::state::SignerClient {
            runtime: state.clone(),
        },
    });
    serve_connections(listener, state, router, shutdown).await
}

async fn serve_connections(
    listener: UnixListener,
    state: RuntimeState,
    router: axum::Router,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), std::io::Error> {
    let permits = Arc::new(Semaphore::new(8));
    let mut tasks = JoinSet::new();
    loop {
        tokio::select! {
            accepted=listener.accept()=>{
                let (stream,_)=accepted?;
                let Ok(permit)=permits.clone().try_acquire_owned() else {drop(stream);continue;};
                let state=state.clone();let router=router.clone();
                tasks.spawn(async move {
                    let _permit=permit;
                    let _=tokio::time::timeout(Duration::from_secs(15),handle_connection(state,router,stream)).await;
                });
            }
            Some(result)=tasks.join_next(), if !tasks.is_empty()=>{if result.is_err() {tracing::error!("control connection task panicked; continuing to serve");}}
            _=shutdown.changed()=>{if *shutdown.borrow(){break;}}
        }
    }
    if tokio::time::timeout(Duration::from_secs(15), async {
        while tasks.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
    Ok(())
}

async fn handle_connection(
    state: RuntimeState,
    router: axum::Router,
    mut stream: UnixStream,
) -> Result<(), std::io::Error> {
    let mut bytes = Zeroizing::new(Vec::new());
    (&mut stream)
        .take(MAX_CONTROL_BYTES + 1)
        .read_to_end(&mut bytes)
        .await?;
    let response = if bytes.len() as u64 > MAX_CONTROL_BYTES {
        invalid_request()
    } else {
        match serde_json::from_slice::<ControlRequest>(&bytes) {
            Ok(ControlRequest::Status) => match state.status().await {
                Ok(status) => ControlResponse::Status { status },
                Err(e) => safe_error(&e),
            },
            Ok(ControlRequest::Http {
                method,
                path,
                authorization,
                body,
            }) => {
                if body.len() > MAX_HTTP_BODY
                    || path.len() > 4096
                    || !path.starts_with('/')
                    || path.starts_with("//")
                    || authorization.as_ref().is_some_and(|h| h.len() > 16384)
                {
                    invalid_request()
                } else {
                    let builder = axum::http::Request::builder()
                        .method(method.as_str())
                        .uri(path)
                        .header("content-type", "application/json");
                    let builder = if let Some(auth) = authorization {
                        builder.header("authorization", auth)
                    } else {
                        builder
                    };
                    match builder.body(axum::body::Body::from(body.expose().to_owned())) {
                        Ok(request) => {
                            let response =
                                router.oneshot(request).await.expect("infallible router");
                            let status = response.status().as_u16();
                            let bytes = axum::body::to_bytes(
                                response.into_body(),
                                MAX_CONTROL_BYTES as usize,
                            )
                            .await
                            .map_err(std::io::Error::other)?;
                            ControlResponse::Http {
                                reply: HttpReply {
                                    status,
                                    body: String::from_utf8(bytes.to_vec())
                                        .map_err(std::io::Error::other)?,
                                },
                            }
                        }
                        Err(_) => invalid_request(),
                    }
                }
            }
            Err(_) => invalid_request(),
        }
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&response).map_err(std::io::Error::other)?);
    stream.write_all(&bytes).await?;
    stream.shutdown().await
}
fn invalid_request() -> ControlResponse {
    ControlResponse::Error {
        code: "invalid_request".into(),
        message: "invalid control request".into(),
    }
}

pub(crate) async fn dispatch_lifecycle(
    state: &RuntimeState,
    request: LifecycleRequest,
) -> ControlResponse {
    let result = match request {
        LifecycleRequest::SealStoredKey {
            team_id,
            actor_public_key,
            name,
            secret_key,
        } => state
            .store
            .seal_stored_key(
                team_id,
                &actor_public_key,
                name,
                secret_key.into_zeroizing(),
            )
            .await
            .map(|key| ControlResponse::StoredKey { key }),
        LifecycleRequest::CreateGrant {
            team_id,
            actor_public_key,
            stored_key_id,
            policy_id,
            name,
            expires_at,
            invitation_expires_at,
        } => state
            .store
            .create_grant(
                team_id,
                &actor_public_key,
                stored_key_id,
                policy_id,
                name,
                expires_at,
                invitation_expires_at,
            )
            .await
            .map(
                |(grant, _invitation_id, bunker_uri)| ControlResponse::GrantCreated {
                    grant,
                    bunker_uri,
                },
            ),
        LifecycleRequest::CreateInvitation {
            grant_id,
            actor_public_key,
            expires_at,
        } => state
            .store
            .create_invitation(grant_id, &actor_public_key, expires_at)
            .await
            .map(
                |(invitation_id, bunker_uri)| ControlResponse::InvitationCreated {
                    invitation_id,
                    bunker_uri,
                },
            ),
        LifecycleRequest::RevokeGrant {
            grant_id,
            actor_public_key,
        } => state
            .store
            .revoke_grant(grant_id, &actor_public_key)
            .await
            .map(|_| ControlResponse::Ok),
        LifecycleRequest::RevokeInvitation {
            invitation_id,
            actor_public_key,
        } => state
            .store
            .revoke_invitation(invitation_id, &actor_public_key)
            .await
            .map(|_| ControlResponse::Ok),
        LifecycleRequest::Reload => Ok(ControlResponse::Ok),
        LifecycleRequest::Status => {
            return match state.status().await {
                Ok(status) => ControlResponse::Status { status },
                Err(error) => safe_error(&error),
            }
        }
    };

    match result {
        Ok(response) => {
            state.reload.notify_waiters();
            response
        }
        Err(error) => safe_error(&error),
    }
}

pub(crate) fn safe_error(error: &crate::store::StoreError) -> ControlResponse {
    use crate::store::StoreError;

    let (code, message) = match error {
        StoreError::NotFound => ("not_found", "resource not found"),
        StoreError::InvalidInput(_) | StoreError::Policy(_) | StoreError::Nostr(_) => {
            ("invalid_input", "invalid request")
        }
        StoreError::InvitationNotClaimable => ("not_claimable", "invitation is not claimable"),
        _ => ("internal_error", "signer operation failed"),
    };
    tracing::warn!(code, error = %error, "signer control operation rejected");
    ControlResponse::Error {
        code: code.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod connection_tests {
    use super::*;
    #[tokio::test]
    async fn panicking_request_does_not_stop_control_service() {
        let path = PathBuf::from("/tmp").join(format!(
            "kc-control-{}.sock",
            &nostr::prelude::Keys::generate().public_key().to_hex()[..16]
        ));
        let listener = UnixListener::bind(&path).unwrap();
        let pool = sqlx_sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let state = RuntimeState::new(crate::store::Store::new(
            pool,
            keycast_core::v2::envelope::EnvelopeCipher::from_key(Zeroizing::new([1; 32])),
        ));
        let router = axum::Router::new()
            .route(
                "/panic",
                axum::routing::get(|| async {
                    panic!("test connection panic");
                    #[allow(unreachable_code)]
                    ""
                }),
            )
            .route("/ok", axum::routing::get(|| async { "healthy" }));
        let (stop, rx) = watch::channel(false);
        let task = tokio::spawn(serve_connections(listener, state, router, rx));
        for endpoint in ["/panic", "/ok"] {
            let mut stream = UnixStream::connect(&path).await.unwrap();
            let request = ControlRequest::Http {
                method: "GET".into(),
                path: endpoint.into(),
                authorization: None,
                body: keycast_core::v2::secret::Secret::new(String::new()),
            };
            stream
                .write_all(&serde_json::to_vec(&request).unwrap())
                .await
                .unwrap();
            stream.shutdown().await.unwrap();
            let mut response = Vec::new();
            tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response))
                .await
                .unwrap()
                .unwrap();
            if endpoint == "/ok" {
                let ControlResponse::Http { reply } = serde_json::from_slice(&response).unwrap()
                else {
                    panic!("missing reply")
                };
                assert_eq!(reply.status, 200);
                assert_eq!(reply.body, "healthy");
            }
        }
        assert!(!task.is_finished());
        stop.send(true).unwrap();
        task.await.unwrap().unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
