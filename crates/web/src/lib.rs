mod error;
mod handlers;
mod state;
mod static_assets;
mod ws;

pub use flock_discovery::DiscoveredHost;
pub use state::AppState;

use axum::routing::{get, post, put};
use axum::Router;

/// Builds the full flock router: static frontend + REST API + live-push
/// websocket. Mirrors srt-router's `pub fn app(state) -> Router` shape so
/// the binary crate owns wiring/config and this crate stays a pure library.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(static_assets::index))
        .route("/app.js", get(static_assets::app_js))
        .route("/style.css", get(static_assets::style_css))
        .route("/health", get(|| async { "ok" }))
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
            "/api/devices/:id/encode",
            get(handlers::get_encode).post(handlers::set_encode),
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
        .with_state(state)
}
