use std::sync::Arc;

use flock_core::{AppSettingsStore, DeviceClientProvider, Registry};
use flock_discovery::Discovery;

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<Registry>,
    pub provider: Arc<dyn DeviceClientProvider>,
    pub discovery: Arc<Discovery>,
    pub app_settings: Arc<AppSettingsStore>,
}
