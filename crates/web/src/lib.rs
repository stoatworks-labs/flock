mod auth;
mod error;
mod handlers;
mod state;
mod static_assets;
mod ws;

pub use auth::LoginGuard;
pub use flock_discovery::DiscoveredHost;
pub use state::AppState;

use axum::middleware;
use axum::routing::{get, post, put};
use axum::Router;

/// Builds the full flock router: static frontend + REST API + live-push
/// websocket. Mirrors srt-router's `pub fn app(state) -> Router` shape so
/// the binary crate owns wiring/config and this crate stays a pure library.
///
/// Split into two sub-routers so the auth middleware (`route_layer`, which
/// only applies to routes defined directly on the router it's called on)
/// covers everything except the static frontend, `/health`, and
/// login/logout - those need to stay reachable before a session exists.
pub fn app(state: AppState) -> Router {
    let public = Router::new()
        .route("/", get(static_assets::index))
        .route("/app.js", get(static_assets::app_js))
        .route("/style.css", get(static_assets::style_css))
        .route("/health", get(|| async { "ok" }))
        .route("/api/login", post(auth::login))
        .route("/api/logout", post(auth::logout));

    let protected = Router::new()
        .route("/api/state", get(handlers::get_state))
        .route("/ws", get(ws::ws_handler))
        .route("/api/devices", post(handlers::create_device))
        .route(
            "/api/devices/:id",
            put(handlers::update_device).delete(handlers::delete_device),
        )
        .route("/api/devices/:id/status", get(handlers::get_status))
        .route(
            "/api/devices/:id/network",
            get(handlers::get_network).post(handlers::set_network),
        )
        .route(
            "/api/devices/:id/decode",
            get(handlers::get_decode).post(handlers::set_decode),
        )
        .route(
            "/api/devices/:id/system",
            get(handlers::get_system).post(handlers::set_system),
        )
        .route("/api/devices/:id/reboot", post(handlers::reboot_device))
        .route("/api/discovery/scan", get(handlers::scan_discovery))
        .route("/api/ndi/sources", get(handlers::get_ndi_sources))
        .route(
            "/api/settings",
            get(handlers::get_app_settings).put(handlers::set_app_settings),
        )
        .route(
            "/api/settings/push-discovery-server",
            post(handlers::push_discovery_server),
        )
        .route(
            "/api/groups/:tag/:tab",
            post(handlers::apply_group_settings),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ));

    public.merge(protected).with_state(state)
}
