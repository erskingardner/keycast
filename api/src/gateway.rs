use crate::state::KeycastState;
use axum::{
    body::{to_bytes, Body},
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
    let Ok(_permit) = ADMISSION.try_acquire() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let (parts, body) = request.into_parts();
    let bytes = match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        to_bytes(body, MAX_HTTP_BODY),
    )
    .await
    {
        Ok(Ok(b)) => b,
        _ => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
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
        body,
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
