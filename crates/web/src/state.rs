use std::sync::Arc;

use flock_core::{DeviceClientProvider, Registry};
use flock_discovery::Discovery;

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<Registry>,
    pub provider: Arc<dyn DeviceClientProvider>,
    pub discovery: Arc<Discovery>,
}
