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

    let (provider, discovery) = backends(&config.provider)?;

    let app_settings =
        flock_core::AppSettingsStore::load_or_new(config.app_settings_path.clone().into())?;

    if config.admin_password.is_some() {
        tracing::info!("admin_password is set - flock's own web UI requires login");
    }

    let state = AppState {
        registry: Arc::new(registry),
        provider,
        discovery: Arc::new(discovery),
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

/// The device provider and the discovery source, picked together from
/// `Config::provider` so they always agree: real hardware gets the real
/// network, and the simulated Plays get simulated discovery rather than
/// whatever the machine running flock can see (a mock-mode scan used to
/// sweep the real subnet and browse real mDNS - which the hosted demo then
/// recorded and published).
fn backends(
    provider: &str,
) -> anyhow::Result<(Arc<dyn flock_core::DeviceClientProvider>, Discovery)> {
    Ok(match provider {
        "http" => {
            tracing::warn!(
                "using the real HTTP device provider - see docs/architecture.md for what's confirmed/unconfirmed"
            );
            (
                Arc::new(flock_device_http::HttpClientProvider::new()),
                Discovery::new()?,
            )
        }
        _ => {
            tracing::info!(
                "using simulated devices and simulated discovery - nothing on the network is scanned"
            );
            (Arc::new(MockClientProvider::new()), Discovery::mock())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::backends;

    #[test]
    fn http_discovers_on_the_real_network() {
        let (_, discovery) = backends("http").unwrap();
        assert!(!discovery.is_mock());
    }

    #[test]
    fn mock_discovery_goes_with_the_mock_provider() {
        let (_, discovery) = backends("mock").unwrap();
        assert!(discovery.is_mock());
    }

    #[test]
    fn anything_but_http_stays_simulated() {
        // The device provider has always fallen back to the mock for an
        // unknown value; discovery has to fall back the same way.
        let (_, discovery) = backends("htttp").unwrap();
        assert!(discovery.is_mock());
    }
}
