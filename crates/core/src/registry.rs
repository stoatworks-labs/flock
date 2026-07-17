use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::RwLock;

use crate::crypto::CredentialCipher;
use crate::device::{Device, DeviceId};

/// Durable store of device metadata. Persisted as a single JSON file - no
/// database, matching the scale of a LAN device list and mirroring
/// srt-router's optional flat-file persistence rather than introducing new
/// infrastructure.
///
/// Every in-memory `Device` holds its BirdUI password in plain text - that's
/// what `DeviceClient` implementations and the rest of the app expect. Only
/// the on-disk JSON is encrypted, via `cipher`, at the `save`/`load_or_new`
/// boundary - see `crypto.rs`.
pub struct Registry {
    path: PathBuf,
    devices: RwLock<HashMap<DeviceId, Device>>,
    cipher: CredentialCipher,
}

impl Registry {
    /// Loads devices from `path` if it exists, otherwise starts empty. The
    /// file is created on first write. The encryption key lives in
    /// `credentials.key` next to `path`, generated on first run.
    pub fn load_or_new(path: PathBuf) -> anyhow::Result<Self> {
        let key_path = path
            .parent()
            .map(|p| p.join("credentials.key"))
            .unwrap_or_else(|| PathBuf::from("credentials.key"));
        let cipher = CredentialCipher::load_or_create(&key_path)?;

        let devices = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            if raw.trim().is_empty() {
                HashMap::new()
            } else {
                let list: Vec<Device> = serde_json::from_str(&raw)?;
                list.into_iter()
                    .map(|mut d| {
                        if let Some(stored) = d.credentials.password.take() {
                            d.credentials.password = Some(cipher.decrypt_or_pass_through(&stored)?);
                        }
                        Ok::<_, anyhow::Error>((d.id, d))
                    })
                    .collect::<anyhow::Result<HashMap<_, _>>>()?
            }
        } else {
            HashMap::new()
        };
        Ok(Self {
            path,
            devices: RwLock::new(devices),
            cipher,
        })
    }

    fn save(&self) -> anyhow::Result<()> {
        let mut list: Vec<Device> = self
            .devices
            .read()
            .expect("registry lock poisoned")
            .values()
            .cloned()
            .collect();
        for device in &mut list {
            if let Some(plaintext) = &device.credentials.password {
                device.credentials.password = Some(self.cipher.encrypt(plaintext)?);
            }
        }
        let raw = serde_json::to_string_pretty(&list)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, raw)?;
        Ok(())
    }

    pub fn list(&self) -> Vec<Device> {
        let devices = self.devices.read().expect("registry lock poisoned");
        let mut list: Vec<Device> = devices.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    pub fn get(&self, id: &DeviceId) -> Option<Device> {
        self.devices
            .read()
            .expect("registry lock poisoned")
            .get(id)
            .cloned()
    }

    pub fn upsert(&self, device: Device) -> anyhow::Result<()> {
        {
            let mut devices = self.devices.write().expect("registry lock poisoned");
            devices.insert(device.id, device);
        }
        self.save()
    }

    pub fn remove(&self, id: &DeviceId) -> anyhow::Result<Option<Device>> {
        let removed = {
            let mut devices = self.devices.write().expect("registry lock poisoned");
            devices.remove(id)
        };
        if removed.is_some() {
            self.save()?;
        }
        Ok(removed)
    }

    /// Groups are computed from tags, never stored separately - this is what
    /// lets one device belong to multiple groups for free.
    pub fn groups(&self) -> BTreeMap<String, Vec<DeviceId>> {
        let devices = self.devices.read().expect("registry lock poisoned");
        let mut groups: BTreeMap<String, Vec<DeviceId>> = BTreeMap::new();
        for device in devices.values() {
            for tag in &device.tags {
                groups.entry(tag.clone()).or_default().push(device.id);
            }
        }
        for members in groups.values_mut() {
            members.sort_by_key(|id| id.0);
        }
        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DeviceCredentials;

    fn sample_device(name: &str, tags: &[&str]) -> Device {
        Device {
            id: DeviceId::new(),
            name: name.to_string(),
            host: "192.168.1.50".to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            credentials: DeviceCredentials::default(),
            discovered: false,
        }
    }

    #[test]
    fn device_can_belong_to_multiple_groups() {
        let dir = tempdir();
        let registry = Registry::load_or_new(dir.join("registry.json")).unwrap();
        let device = sample_device("cam-1", &["stage", "backup"]);
        let id = device.id;
        registry.upsert(device).unwrap();

        let groups = registry.groups();
        assert_eq!(groups["stage"], vec![id]);
        assert_eq!(groups["backup"], vec![id]);
    }

    #[test]
    fn persists_across_reload() {
        let dir = tempdir();
        let path = dir.join("registry.json");
        {
            let registry = Registry::load_or_new(path.clone()).unwrap();
            registry.upsert(sample_device("cam-1", &["stage"])).unwrap();
        }
        let reloaded = Registry::load_or_new(path).unwrap();
        assert_eq!(reloaded.list().len(), 1);
    }

    #[test]
    fn password_never_touches_disk_in_plaintext() {
        let dir = tempdir();
        let path = dir.join("registry.json");
        let mut device = sample_device("cam-1", &["stage"]);
        device.credentials.password = Some("super-secret".to_string());

        let registry = Registry::load_or_new(path.clone()).unwrap();
        registry.upsert(device).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("super-secret"));

        // In-memory access still sees the plaintext password unchanged.
        let devices = registry.list();
        assert_eq!(
            devices[0].credentials.password.as_deref(),
            Some("super-secret")
        );

        // And it round-trips correctly across a reload against the same key.
        let reloaded = Registry::load_or_new(path).unwrap();
        assert_eq!(
            reloaded.list()[0].credentials.password.as_deref(),
            Some("super-secret")
        );
    }

    #[test]
    fn migrates_a_legacy_plaintext_registry_on_next_save() {
        let dir = tempdir();
        let path = dir.join("registry.json");
        let mut device = sample_device("cam-1", &["stage"]);
        device.credentials.password = Some("old-plaintext".to_string());
        std::fs::write(&path, serde_json::to_string(&vec![&device]).unwrap()).unwrap();

        // Loading a pre-encryption registry.json still yields the plaintext
        // password in memory...
        let registry = Registry::load_or_new(path.clone()).unwrap();
        assert_eq!(
            registry.list()[0].credentials.password.as_deref(),
            Some("old-plaintext")
        );

        // ...and the very next save encrypts it going forward.
        registry
            .upsert(registry.list().into_iter().next().unwrap())
            .unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("old-plaintext"));
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("flock-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
