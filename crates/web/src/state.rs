use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use flock_core::{AppSettingsStore, DeviceClientProvider, Registry};
use flock_discovery::Discovery;

use crate::auth::LoginGuard;

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<Registry>,
    pub provider: Arc<dyn DeviceClientProvider>,
    pub discovery: Arc<Discovery>,
    pub app_settings: Arc<AppSettingsStore>,
    /// `None` means flock's own web UI has no login gate at all (today's
    /// default, matching BirdUI's own trusted-LAN model). `Some` requires a
    /// valid session (see `auth.rs`) for every route except the static
    /// frontend, `/health`, and `/api/login`/`/api/logout` themselves.
    pub admin_password: Option<String>,
    /// Valid session tokens, in memory only - a restart naturally logs
    /// everyone out, which is an acceptable tradeoff for a LAN tool over the
    /// complexity of persisting/expiring sessions.
    pub sessions: Arc<RwLock<HashSet<String>>>,
    /// Failed-login tracking for `POST /api/login` - see `LoginGuard`.
    pub login_guard: Arc<LoginGuard>,
}
