//! Optional login gate for flock's own web UI - separate from a device's
//! BirdUI password. Off by default (`AppState::admin_password` is `None`,
//! matching flock's historical trusted-LAN-only behavior); when a
//! `[web].admin_password` is configured, every route except the static
//! frontend, `/health`, and login/logout itself requires a valid session.
//!
//! Sessions are a random token in an in-memory set, handed out via a
//! `flock_session` cookie - no persistence, no expiry, no external crate:
//! a process restart naturally logs everyone out, which is an acceptable
//! tradeoff for a single-operator LAN tool.

use axum::extract::{Request, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

const COOKIE_NAME: &str = "flock_session";

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(admin_password) = &state.admin_password else {
        return Err(ApiError::BadRequest(
            "this flock instance has no admin_password configured - nothing to log into".into(),
        ));
    };
    if !constant_time_eq(&body.password, admin_password) {
        return Err(ApiError::Unauthorized("incorrect password".into()));
    }
    let token = new_session_token();
    state
        .sessions
        .write()
        .expect("session lock poisoned")
        .insert(token.clone());
    Ok((
        [(
            SET_COOKIE,
            format!("{COOKIE_NAME}={token}; HttpOnly; Path=/; SameSite=Strict"),
        )],
        Json(serde_json::json!({"ok": true})),
    ))
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(token) = session_token(&headers) {
        state
            .sessions
            .write()
            .expect("session lock poisoned")
            .remove(&token);
    }
    (
        [(SET_COOKIE, format!("{COOKIE_NAME}=; Max-Age=0; Path=/"))],
        Json(serde_json::json!({"ok": true})),
    )
}

/// Applied via `route_layer` to every route that needs a session - static
/// assets, `/health`, and login/logout are on a separate, un-layered router
/// (see `lib.rs`) so they stay reachable before a session exists.
pub async fn require_auth(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if state.admin_password.is_none() {
        return next.run(req).await;
    }
    let authed = session_token(req.headers())
        .map(|token| {
            state
                .sessions
                .read()
                .expect("session lock poisoned")
                .contains(&token)
        })
        .unwrap_or(false);
    if authed {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, "login required").into_response()
    }
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    raw.split(';')
        .map(str::trim)
        .find_map(|kv| kv.strip_prefix(&format!("{COOKIE_NAME}=")))
        .map(str::to_string)
}

fn new_session_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// Avoids a timing side-channel on the password comparison - cheap enough to
/// hand-roll without pulling in a dedicated constant-time-compare crate.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_regular_equality() {
        assert!(constant_time_eq("hunter2", "hunter2"));
        assert!(!constant_time_eq("hunter2", "hunter3"));
        assert!(!constant_time_eq("short", "shorter"));
        assert!(!constant_time_eq("", "x"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn session_token_parses_cookie_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            "other=1; flock_session=abc123; foo=bar".parse().unwrap(),
        );
        assert_eq!(session_token(&headers).as_deref(), Some("abc123"));
    }

    #[test]
    fn session_token_absent_without_cookie() {
        assert_eq!(session_token(&HeaderMap::new()), None);
    }
}
