//! A simulated BirdDog Play so flock can be built, demoed, and tested before
//! any real hardware is on the bench. Implements the same `DeviceClient`
//! surface a real HTTP-backed client will (Phase 2) - swapping providers is
//! the only change needed once that lands.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use flock_core::{
    ConfigMethod, DecodeSettings, Device, DeviceClient, DeviceClientProvider, DeviceId,
    DeviceStatus, NdiTransmitMethod, NetworkSettings, SystemSettings,
};

struct MockState {
    status: DeviceStatus,
    network: NetworkSettings,
    decode: DecodeSettings,
    system: SystemSettings,
}

pub struct MockDevice {
    state: RwLock<MockState>,
}

impl MockDevice {
    pub fn new(name: &str) -> Self {
        let stream_name = name.replace(' ', "-").to_lowercase();
        let state = MockState {
            status: DeviceStatus {
                online: true,
                ndi_stream_name: stream_name.clone(),
                video_format: "1080p60".to_string(),
                audio_status: "Mute".to_string(),
                video_resolution: "1920x1080".to_string(),
                video_frame_rate: "60".to_string(),
                average_bitrate_mbps: 134.0,
                firmware_version: "1.0.2".to_string(),
                system_name: stream_name.clone(),
            },
            network: NetworkSettings {
                config_method: ConfigMethod::Dhcp,
                ip_address: "192.168.100.100".to_string(),
                subnet_mask: "255.255.255.0".to_string(),
                gateway_address: "192.168.100.1".to_string(),
                dhcp_timeout_secs: 20,
                fallback_ip_address: "192.168.100.100".to_string(),
                fallback_subnet_mask: "255.255.255.0".to_string(),
                birddog_name: stream_name.clone(),
                ndi_transmit_method: NdiTransmitMethod::Tcp,
                ndi_receive_method: NdiTransmitMethod::Tcp,
                multicast_net_prefix: "239.255.0.0".to_string(),
                multicast_net_mask: "255.255.0.0".to_string(),
                multicast_ttl: 1,
                ndi_discovery_server_enabled: false,
                ndi_discovery_server_ips: vec![],
            },
            decode: DecodeSettings {
                source_type: "NDI".to_string(),
                selected_source: None,
                available_sources: vec![],
                failover_source: None,
                srt_connection_type: "caller".to_string(),
                srt_stream_name: None,
                srt_ip_address: None,
                srt_port: None,
                srt_latency_ms: 120,
                srt_encryption_enabled: false,
                srt_encryption_key_length: None,
                srt_passphrase: None,
                srt_stream_id: None,
                srt_available_sources: vec![],
                screensaver_mode: "BlackSS".to_string(),
                color_space: "YUV".to_string(),
                ndi_audio_enabled: false,
                tally_mode: "TallyOff".to_string(),
            },
            system: SystemSettings {
                firmware_version: "1.0.2".to_string(),
                remote_ip_list: vec![],
                ndi_group_list: vec![],
            },
        };
        Self {
            state: RwLock::new(state),
        }
    }
}

#[async_trait]
impl DeviceClient for MockDevice {
    async fn status(&self) -> anyhow::Result<DeviceStatus> {
        Ok(self
            .state
            .read()
            .expect("mock lock poisoned")
            .status
            .clone())
    }

    async fn network_settings(&self) -> anyhow::Result<NetworkSettings> {
        Ok(self
            .state
            .read()
            .expect("mock lock poisoned")
            .network
            .clone())
    }

    async fn set_network_settings(&self, settings: NetworkSettings) -> anyhow::Result<()> {
        self.state.write().expect("mock lock poisoned").network = settings;
        Ok(())
    }

    async fn decode_settings(&self) -> anyhow::Result<DecodeSettings> {
        Ok(self
            .state
            .read()
            .expect("mock lock poisoned")
            .decode
            .clone())
    }

    async fn set_decode_settings(&self, settings: DecodeSettings) -> anyhow::Result<()> {
        self.state.write().expect("mock lock poisoned").decode = settings;
        Ok(())
    }

    async fn system_settings(&self) -> anyhow::Result<SystemSettings> {
        Ok(self
            .state
            .read()
            .expect("mock lock poisoned")
            .system
            .clone())
    }

    async fn set_system_settings(&self, settings: SystemSettings) -> anyhow::Result<()> {
        self.state.write().expect("mock lock poisoned").system = settings;
        Ok(())
    }

    async fn reboot(&self) -> anyhow::Result<()> {
        // Real devices drop offline for a few seconds; nothing to simulate
        // here since flock only cares that the call succeeds.
        Ok(())
    }
}

/// Hands out one `MockDevice` per `DeviceId`, creating it lazily so devices
/// added at runtime (manual add, discovery) get a working backend with no
/// extra wiring.
#[derive(Default)]
pub struct MockClientProvider {
    devices: RwLock<HashMap<DeviceId, Arc<MockDevice>>>,
}

impl MockClientProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DeviceClientProvider for MockClientProvider {
    fn client_for(&self, device: &Device) -> Arc<dyn DeviceClient> {
        if let Some(existing) = self
            .devices
            .read()
            .expect("mock provider lock poisoned")
            .get(&device.id)
        {
            return existing.clone();
        }
        let mock = Arc::new(MockDevice::new(&device.name));
        self.devices
            .write()
            .expect("mock provider lock poisoned")
            .insert(device.id, mock.clone());
        mock
    }
}

/// A handful of canned devices so a fresh flock instance has something to
/// look at immediately, matching srt-router's demo-config precedent.
pub fn demo_devices() -> Vec<Device> {
    use flock_core::DeviceCredentials;

    vec![
        Device {
            id: DeviceId::new(),
            name: "Stage Cam Play".to_string(),
            host: "birddog-stage.local".to_string(),
            tags: vec!["stage".to_string(), "primary".to_string()],
            credentials: DeviceCredentials {
                password: Some("birddog".to_string()),
            },
            discovered: false,
        },
        Device {
            id: DeviceId::new(),
            name: "Lobby Play".to_string(),
            host: "birddog-lobby.local".to_string(),
            tags: vec!["lobby".to_string()],
            credentials: DeviceCredentials {
                password: Some("birddog".to_string()),
            },
            discovered: false,
        },
        Device {
            id: DeviceId::new(),
            name: "Backup Feed Play".to_string(),
            host: "birddog-backup.local".to_string(),
            tags: vec!["stage".to_string(), "backup".to_string()],
            credentials: DeviceCredentials {
                password: Some("birddog".to_string()),
            },
            discovered: false,
        },
    ]
}
