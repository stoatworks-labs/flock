use serde::Deserialize;
use std::fmt;

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub bind: String,
    pub registry_path: String,
    pub app_settings_path: String,
    /// Seeds a handful of canned mock devices on first run so a fresh
    /// instance isn't an empty screen. Has no effect once registry.json
    /// already has content.
    pub seed_demo_devices: bool,
    /// "mock" (default) talks to the built-in simulated Play; "http" talks
    /// to real BirdDog PLAY hardware over its actual BirdUI. See
    /// docs/architecture.md for what's confirmed/unconfirmed about the real
    /// implementation before switching this on.
    pub provider: String,
    /// Unset (default) means flock's own web UI has no login gate at all -
    /// matching BirdUI's own trusted-LAN model. When set, every device and
    /// registry gets protected by a single shared session login - see
    /// docs/architecture.md's "flock's own auth" section.
    pub admin_password: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8080".to_string(),
            registry_path: "data/registry.json".to_string(),
            app_settings_path: "data/app_settings.json".to_string(),
            seed_demo_devices: true,
            provider: "mock".to_string(),
            admin_password: None,
        }
    }
}

/// Hand-rolled so `admin_password` never lands in a log line via `{:?}` -
/// `main.rs` logs the loaded config at startup.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("bind", &self.bind)
            .field("registry_path", &self.registry_path)
            .field("app_settings_path", &self.app_settings_path)
            .field("seed_demo_devices", &self.seed_demo_devices)
            .field("provider", &self.provider)
            .field(
                "admin_password",
                &self.admin_password.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        if std::path::Path::new(path).exists() {
            let raw = std::fs::read_to_string(path)?;
            Ok(toml::from_str(&raw)?)
        } else {
            Ok(Self::default())
        }
    }
}
