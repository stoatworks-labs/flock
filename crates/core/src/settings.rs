//! Per-tab settings shapes, one struct per BirdUI panel (Dashboard/Network/
//! AV Setup's Decode Settings/System). Play is decode-only, so there is no
//! Encode panel. Field *values* (enum variants, option strings) are
//! confirmed against a real BirdDog PLAY unit (firmware 1.0.18) - see
//! docs/architecture.md for how. Field *names* here are flock's own, chosen
//! to read cleanly; `flock-device-http` maps them to the real device's
//! actual form field names (which are considerably messier - e.g.
//! `dec0_source_name`, `Txpm`/`Rxpm`).

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

/// Confirmed against the real device's Network page: the transmit (`Txpm`)
/// and receive (`Rxpm`) preferred-method dropdowns both offer exactly these
/// four options (the real markup has a harmless duplicate "TCP" entry that
/// isn't a distinct value). Note "RUDP" has no hyphen in the real form value
/// even though BirdUI's own prose calls it "R-UDP".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NdiTransmitMethod {
    #[serde(rename = "TCP")]
    Tcp,
    #[serde(rename = "UDP")]
    Udp,
    Multicast,
    #[serde(rename = "RUDP")]
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
    pub ndi_transmit_method: NdiTransmitMethod,
    /// Play is primarily a receiver - this is its own preferred-method
    /// setting for the NDI/SRT source it's decoding, distinct from
    /// `ndi_transmit_method`. Real device exposes both (`Txpm`/`Rxpm`).
    pub ndi_receive_method: NdiTransmitMethod,
    pub multicast_net_prefix: String,
    pub multicast_net_mask: String,
    pub multicast_ttl: u8,
    pub ndi_discovery_server_enabled: bool,
    pub ndi_discovery_server_ips: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodeSettings {
    pub selected_source: Option<String>,
    /// Real device has no discovered-source picker for this field - it's a
    /// free-text NDI source name. Kept for a future NDI-SDK-backed picker;
    /// against real hardware this is always empty.
    pub available_sources: Vec<String>,
    pub failover_source: Option<String>,
    /// Real option values: "CaptureSS" (captured frame), "BlackSS", "BirdDogSS".
    pub screensaver_mode: String,
    /// Real option values: "YUV", "RGB".
    pub color_space: String,
    pub ndi_audio_enabled: bool,
    /// Real option values: "TallyOn", "TallyOff", "VideoMode".
    pub tally_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSettings {
    /// Read-only in practice: the real device only exposes this on the
    /// Dashboard page, not as a settable System field.
    pub firmware_version: String,
    /// Real device's Access Manager accepts these only as an uploaded text
    /// file (quoted, comma-separated) - there is no way to read the current
    /// list back through the web UI, so `flock-device-http` always reports
    /// this as empty on read. Round-tripping (fetch-then-resubmit) isn't
    /// possible against real hardware; treat writes as authoritative-replace.
    pub remote_ip_list: Vec<String>,
    pub ndi_group_list: Vec<String>,
}
