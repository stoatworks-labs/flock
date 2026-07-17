mod config;

use std::sync::Arc;

use flock_device_mock::{demo_devices, MockClientProvider};
use flock_discovery::Discovery;
use flock_web::AppState;

use config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("flock=info".parse()?),
        )
        .init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config/flock.toml".to_string());
    let config = Config::load(&config_path)?;
    tracing::info!(?config, "loaded config");

    let registry = flock_core::Registry::load_or_new(config.registry_path.clone().into())?;
    if config.seed_demo_devices && registry.list().is_empty() {
        tracing::info!("registry is empty, seeding demo devices");
        for device in demo_devices() {
            registry.upsert(device)?;
        }
    }

    let provider: Arc<dyn flock_core::DeviceClientProvider> = match config.provider.as_str() {
        "http" => {
            tracing::warn!(
                "using the real HTTP device provider - see docs/architecture.md for what's confirmed/unconfirmed"
            );
            Arc::new(flock_device_http::HttpClientProvider::new())
        }
        _ => Arc::new(MockClientProvider::new()),
    };

    let app_settings =
        flock_core::AppSettingsStore::load_or_new(config.app_settings_path.clone().into())?;

    if config.admin_password.is_some() {
        tracing::info!("admin_password is set - flock's own web UI requires login");
    }

    let state = AppState {
        registry: Arc::new(registry),
        provider,
        discovery: Arc::new(Discovery::new()?),
        app_settings: Arc::new(app_settings),
        admin_password: config.admin_password.clone(),
        sessions: Arc::new(std::sync::RwLock::new(std::collections::HashSet::new())),
    };

    let app = flock_web::app(state);
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!(bind = %config.bind, "flock listening");
    axum::serve(listener, app).await?;
    Ok(())
}
