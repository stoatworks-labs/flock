use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub bind: String,
    pub registry_path: String,
    /// Seeds a handful of canned mock devices on first run so a fresh
    /// instance isn't an empty screen. Has no effect once registry.json
    /// already has content.
    pub seed_demo_devices: bool,
    /// "mock" (default) talks to the built-in simulated Play; "http" talks
    /// to real BirdDog PLAY hardware over its actual BirdUI. See
    /// docs/architecture.md for what's confirmed/unconfirmed about the real
    /// implementation before switching this on.
    pub provider: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8080".to_string(),
            registry_path: "data/registry.json".to_string(),
            seed_demo_devices: true,
            provider: "mock".to_string(),
        }
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
