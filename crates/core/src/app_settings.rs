use std::path::PathBuf;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

/// flock's own app-level settings - not per-device, persisted separately
/// from the device registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    /// The operator's NDI Discovery Server address (e.g. `192.168.1.5`), if
    /// they run one. flock itself can't speak that server's proprietary
    /// wire protocol (no public spec, no NDI SDK dependency here - see
    /// docs/architecture.md), so this doesn't change how flock discovers
    /// NDI sources (still open mDNS). What it *does* do: give one place to
    /// push this address out to every managed Play's own Network settings
    /// (`ndi_discovery_server_ips`), which do speak it natively, instead of
    /// configuring each device by hand.
    pub discovery_server: Option<String>,
}

pub struct AppSettingsStore {
    path: PathBuf,
    settings: RwLock<AppSettings>,
}

impl AppSettingsStore {
    pub fn load_or_new(path: PathBuf) -> anyhow::Result<Self> {
        let settings = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            if raw.trim().is_empty() {
                AppSettings::default()
            } else {
                serde_json::from_str(&raw)?
            }
        } else {
            AppSettings::default()
        };
        Ok(Self {
            path,
            settings: RwLock::new(settings),
        })
    }

    pub fn get(&self) -> AppSettings {
        self.settings
            .read()
            .expect("app settings lock poisoned")
            .clone()
    }

    pub fn set(&self, settings: AppSettings) -> anyhow::Result<()> {
        {
            let mut current = self.settings.write().expect("app settings lock poisoned");
            *current = settings;
        }
        let raw = serde_json::to_string_pretty(&self.get())?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, raw)?;
        Ok(())
    }
}
