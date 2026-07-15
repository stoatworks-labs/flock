use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub Uuid);

impl DeviceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for DeviceId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceMode {
    Encode,
    Decode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceCredentials {
    /// BirdUI password for this device. Never echoed back to the frontend in
    /// full - the API layer redacts it on read.
    pub password: Option<String>,
}

/// A registered BirdDog Play unit. This is pure metadata + how to reach it;
/// the actual control surface lives behind `DeviceClient` (see client.rs) so
/// swapping the mock implementation for a real HTTP one never touches this
/// struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub host: String,
    pub mode: DeviceMode,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub credentials: DeviceCredentials,
    /// True if this device was found via mDNS discovery rather than typed in
    /// manually - purely informational, shown as a badge in the UI.
    #[serde(default)]
    pub discovered: bool,
}

impl Device {
    pub fn redacted(mut self) -> Self {
        if self.credentials.password.is_some() {
            self.credentials.password = Some("********".to_string());
        }
        self
    }
}
