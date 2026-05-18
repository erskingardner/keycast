use super::{auth_middleware, public_config};
use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use sqlx_sqlite::SqlitePool;

use crate::api::http::teams;

pub fn routes(pool: SqlitePool) -> Router {
    tracing::debug!("Building routes");
    Router::new()
        .route("/config", get(public_config))
        .merge(protected_routes(pool))
}

fn protected_routes(pool: SqlitePool) -> Router {
    Router::new()
        .route("/teams", get(teams::list_teams))
        .route("/teams", post(teams::create_team))
        .route("/teams/:id", get(teams::get_team))
        .route("/teams/:id", put(teams::update_team))
        .route("/teams/:id", delete(teams::delete_team))
        .route("/teams/:id/users", post(teams::add_user))
        .route(
            "/teams/:id/users/:user_public_key",
            delete(teams::remove_user),
        )
        .route("/teams/:id/keys", post(teams::add_key))
        .route("/teams/:id/keys/:pubkey", get(teams::get_key))
        .route("/teams/:id/keys/:pubkey", delete(teams::remove_key))
        .route(
            "/teams/:id/keys/:pubkey/authorizations",
            post(teams::add_authorization),
        )
        .route(
            "/teams/:id/keys/:pubkey/authorizations/:authorization_id",
            delete(teams::remove_authorization),
        )
        .route("/teams/:id/policies", post(teams::add_policy))
        .layer(middleware::from_fn(auth_middleware))
        .with_state(pool)
}
