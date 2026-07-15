use async_trait::async_trait;
use std::sync::Arc;

use crate::device::Device;
use crate::settings::{
    DecodeSettings, DeviceStatus, EncodeSettings, NetworkSettings, SystemSettings,
};

/// Everything flock can do to a single Play device. Implemented today by
/// `flock-device-mock`; a real HTTP implementation talking to actual BirdUI/
/// REST endpoints is Phase 2 (see docs/roadmap.md) and slots in behind this
/// same trait without touching the registry, API layer, or frontend.
#[async_trait]
pub trait DeviceClient: Send + Sync {
    async fn status(&self) -> anyhow::Result<DeviceStatus>;

    async fn network_settings(&self) -> anyhow::Result<NetworkSettings>;
    async fn set_network_settings(&self, settings: NetworkSettings) -> anyhow::Result<()>;

    async fn encode_settings(&self) -> anyhow::Result<EncodeSettings>;
    async fn set_encode_settings(&self, settings: EncodeSettings) -> anyhow::Result<()>;

    async fn decode_settings(&self) -> anyhow::Result<DecodeSettings>;
    async fn set_decode_settings(&self, settings: DecodeSettings) -> anyhow::Result<()>;

    async fn system_settings(&self) -> anyhow::Result<SystemSettings>;
    async fn set_system_settings(&self, settings: SystemSettings) -> anyhow::Result<()>;

    async fn reboot(&self) -> anyhow::Result<()>;
}

/// Resolves a `Device` record to the client that actually talks to it.
/// Exists so the registry only ever deals in serializable metadata while a
/// separate provider (mock today, real-HTTP later) owns live connections.
pub trait DeviceClientProvider: Send + Sync {
    fn client_for(&self, device: &Device) -> Arc<dyn DeviceClient>;
}
