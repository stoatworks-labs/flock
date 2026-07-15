//! Per-tab settings shapes, one struct per BirdUI panel (Dashboard/Network/
//! AV Setup's Decode Settings/System). Play is decode-only, so there is no
//! Encode panel. Field names follow the BirdUI User Guide as closely as the
//! public docs allow; the real device's exact REST field names are
//! unconfirmed (see docs/architecture.md) so expect to adjust these when
//! `DeviceClient` grows a real HTTP implementation.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub online: bool,
    pub ndi_stream_name: String,
    pub video_format: String,
    pub audio_status: String,
    pub video_resolution: String,
    pub video_frame_rate: String,
    pub average_bitrate_mbps: f32,
    pub firmware_version: String,
    pub system_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigMethod {
    Dhcp,
    Static,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NdiTransmitMethod {
    Tcp,
    Udp,
    #[serde(rename = "R-UDP")]
    RUdp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSettings {
    pub config_method: ConfigMethod,
    pub ip_address: String,
    pub subnet_mask: String,
    pub gateway_address: String,
    pub dhcp_timeout_secs: u32,
    pub fallback_ip_address: String,
    pub fallback_subnet_mask: String,
    pub birddog_name: String,
    pub wifi_enabled: bool,
    pub ndi_transmit_method: NdiTransmitMethod,
    pub multicast_net_prefix: String,
    pub multicast_net_mask: String,
    pub multicast_ttl: u8,
    pub ndi_discovery_server_enabled: bool,
    pub ndi_discovery_server_ips: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodeSettings {
    pub selected_source: Option<String>,
    pub available_sources: Vec<String>,
    pub failover_source: Option<String>,
    pub screensaver_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSettings {
    pub firmware_version: String,
    pub remote_ip_list: Vec<String>,
    pub ndi_group_list: Vec<String>,
    pub ui_mode: String,
}
