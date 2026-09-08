use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};

use crate::management::api::http::{auth_middleware, public_config, teams};
use crate::management::state::KeycastState;

pub fn routes(state: KeycastState) -> Router {
    Router::new()
        .route("/config", get(public_config))
        .merge(protected_routes(state.clone()))
        .with_state(state)
}

fn protected_routes(state: KeycastState) -> Router<KeycastState> {
    Router::new()
        .route("/status", get(teams::status))
        .route("/relays", put(teams::update_relays))
        .route("/relay-discovery", put(teams::update_discovery_policy))
        .route("/teams", get(teams::list_teams).post(teams::create_team))
        .route(
            "/teams/{id}",
            get(teams::get_team)
                .put(teams::update_team)
                .delete(teams::delete_team),
        )
        .route("/teams/{id}/users", post(teams::add_user))
        .route(
            "/teams/{id}/users/{user_public_key}",
            delete(teams::remove_user),
        )
        .route("/teams/{id}/keys", post(teams::add_key))
        .route(
            "/teams/{id}/keys/{pubkey}",
            get(teams::get_key).delete(teams::remove_key),
        )
        .route("/teams/{id}/policies", post(teams::add_policy))
        .route(
            "/teams/{id}/policies/{policy_id}",
            put(teams::update_policy).delete(teams::remove_policy),
        )
        .route("/teams/{id}/keys/{pubkey}/grants", post(teams::add_grant))
        .route(
            "/teams/{id}/keys/{pubkey}/grants/{grant_id}",
            delete(teams::revoke_grant),
        )
        .route(
            "/teams/{id}/grants/{grant_id}/invitations",
            post(teams::create_invitation),
        )
        .route(
            "/teams/{id}/invitations/{invitation_id}",
            delete(teams::revoke_invitation),
        )
        .route("/teams/{id}/audit", get(teams::list_audit))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
}
