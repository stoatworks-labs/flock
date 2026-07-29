mod config;

use std::sync::Arc;

use flock_device_mock::{demo_devices, MockClientProvider};
use flock_discovery::Discovery;
use flock_web::AppState;

use config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Before the config is read, so a fault while parsing it has somewhere to
    // be recorded.
    let _diag = diag::init(
        diag::Options::new("flock", "FLOCK", env!("CARGO_PKG_VERSION"))
            .with_default_filter("flock=info"),
    )?;

    // No clap here, so the flag is checked by hand rather than restructuring
    // the whole binary around an argument parser it does not otherwise need.
    if std::env::args().any(|a| a == "--collect-diagnostics") {
        println!("{}", diag::collect_diagnostics()?.display());
        return Ok(());
    }

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config/flock.toml".to_string());
    let config = Config::load(&config_path)?;
    tracing::info!(?config, "loaded config");
    diag::set_config(&config);

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
        login_guard: Arc::new(flock_web::LoginGuard::new()),
    };

    let app = flock_web::app(state);
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!(bind = %config.bind, "flock listening");
    axum::serve(listener, app).await?;
    Ok(())
}
