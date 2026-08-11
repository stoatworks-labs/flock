use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use flock_web::{app, AppState, LoginGuard};

#[test]
fn router_builds_without_panicking() {
    let state = AppState {
        registry: Arc::new(
            flock_core::Registry::load_or_new(
                std::env::temp_dir().join("flock-smoke-registry.json"),
            )
            .unwrap(),
        ),
        provider: Arc::new(flock_device_mock::MockClientProvider::new()),
        discovery: Arc::new(flock_discovery::Discovery::new().unwrap()),
        app_settings: Arc::new(
            flock_core::AppSettingsStore::load_or_new(
                std::env::temp_dir().join("flock-smoke-settings.json"),
            )
            .unwrap(),
        ),
        admin_password: None,
        sessions: Arc::new(RwLock::new(HashSet::new())),
        login_guard: Arc::new(LoginGuard::new()),
    };
    let _ = app(state);
}
