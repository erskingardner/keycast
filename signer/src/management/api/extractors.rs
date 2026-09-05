use axum::extract::FromRequestParts;
use axum::http::{request::Parts, StatusCode};
use nostr::prelude::Event;

pub struct AuthEvent(pub Event);

impl<S> FromRequestParts<S> for AuthEvent
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Event>()
            .cloned()
            .map(Self)
            .ok_or((StatusCode::UNAUTHORIZED, "authentication required"))
    }
}
