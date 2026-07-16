//! Real `DeviceClient` implementation talking to an actual BirdDog PLAY's
//! BirdUI over HTTP. Confirmed against firmware 1.0.18 - see
//! docs/architecture.md for how these routes/field names were derived and
//! what remains unconfirmed (dual decode-channel markup, analog audio,
//! genlock, HDMI OSD - none of that is wired up here, only what flock's own
//! settings tabs model).
//!
//! BirdUI is server-rendered HTML, not a JSON API, so every read is a GET +
//! HTML-scrape and every write is a read-modify-write: fetch the current
//! page, override the handful of fields flock actually manages, POST the
//! *entire* scraped field map back so anything flock doesn't model (e.g.
//! the shared template's hidden Encode fields, HDMI OSD timeout, genlock)
//! passes through unchanged instead of being silently cleared.

mod form;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use flock_core::{
    ConfigMethod, DecodeSettings, Device, DeviceClient, DeviceClientProvider, DeviceId,
    DeviceStatus, NdiTransmitMethod, NetworkSettings, SystemSettings,
};

const DEFAULT_PASSWORD: &str = "birddog";
/// Present on BirdUI's login page (`<form id="auth_form">`) - if a fetched
/// page contains this, the session is missing/expired and we got the login
/// page back instead of the content we asked for.
const LOGIN_MARKER: &str = "auth_form";
/// The browser's own decode-source picker doesn't come from BirdUI's HTML
/// at all - it's populated client-side via `$.getJSON` against a *separate*
/// JSON API the device runs on this port, returning `{"source name": "ip:port"}`
/// for every NDI source it currently sees (confirmed live: `GET /List` on
/// this port, not the web UI's port 80).
const SOURCE_LIST_PORT: u16 = 8080;
/// Placeholder text BirdUI renders for an unset source field.
const NONE_SOURCE: &str = "None";

pub struct HttpDeviceClient {
    base_url: String,
    source_list_url: String,
    password: String,
    http: reqwest::Client,
    logged_in: AtomicBool,
}

impl HttpDeviceClient {
    pub fn new(device: &Device) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .cookie_store(true)
            // The real device has been observed taking several seconds to
            // respond even to simple GETs - a short timeout here would
            // misreport a slow-but-healthy device as unreachable.
            .timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self {
            base_url: format!("http://{}", device.host),
            source_list_url: format!("http://{}:{SOURCE_LIST_PORT}/List", device.host),
            password: device
                .credentials
                .password
                .clone()
                .unwrap_or_else(|| DEFAULT_PASSWORD.to_string()),
            http,
            logged_in: AtomicBool::new(false),
        })
    }

    /// `{source name: "ip:port"}` for every NDI source the device currently
    /// sees, straight from its own discovery - not scraped from BirdUI HTML.
    async fn fetch_source_list(&self) -> anyhow::Result<HashMap<String, String>> {
        let text = self
            .http
            .get(&self.source_list_url)
            .send()
            .await?
            .text()
            .await?;
        let map: HashMap<String, String> = serde_json::from_str(&text)?;
        Ok(map)
    }

    async fn login(&self) -> anyhow::Result<()> {
        self.http
            .post(format!("{}/login", self.base_url))
            .form(&[("auth_password", self.password.as_str())])
            .send()
            .await?;
        self.logged_in.store(true, Ordering::Relaxed);
        Ok(())
    }

    async fn get_page(&self, path: &str) -> anyhow::Result<String> {
        if !self.logged_in.load(Ordering::Relaxed) {
            self.login().await?;
        }
        let url = format!("{}{}", self.base_url, path);
        let mut body = self.http.get(&url).send().await?.text().await?;
        if body.contains(LOGIN_MARKER) {
            self.login().await?;
            body = self.http.get(&url).send().await?.text().await?;
            if body.contains(LOGIN_MARKER) {
                anyhow::bail!(
                    "login failed for {} - check the device's BirdUI password",
                    self.base_url
                );
            }
        }
        Ok(body)
    }

    async fn post_form(&self, path: &str, fields: HashMap<String, String>) -> anyhow::Result<()> {
        if !self.logged_in.load(Ordering::Relaxed) {
            self.login().await?;
        }
        self.http
            .post(format!("{}{}", self.base_url, path))
            .multipart(form::to_multipart(fields))
            .send()
            .await?;
        Ok(())
    }

    /// Access Manager's remote-IP/NDI-group lists are only settable as an
    /// uploaded text file of quoted, comma-separated entries (per the
    /// BirdUI User Guide's own example) - there's no JSON equivalent.
    async fn upload_list_file(
        &self,
        path: &str,
        field_name: &str,
        items: &[String],
    ) -> anyhow::Result<()> {
        if !self.logged_in.load(Ordering::Relaxed) {
            self.login().await?;
        }
        let content = items
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(",");
        let part = reqwest::multipart::Part::text(content)
            .file_name("list.txt")
            .mime_str("text/plain")?;
        let form = reqwest::multipart::Form::new().part(field_name.to_string(), part);
        self.http
            .post(format!("{}{}", self.base_url, path))
            .multipart(form)
            .send()
            .await?;
        Ok(())
    }
}

fn parse_ndi_method(s: &str) -> NdiTransmitMethod {
    match s {
        "UDP" => NdiTransmitMethod::Udp,
        "Multicast" => NdiTransmitMethod::Multicast,
        "RUDP" => NdiTransmitMethod::RUdp,
        _ => NdiTransmitMethod::Tcp,
    }
}

fn ndi_method_str(m: NdiTransmitMethod) -> &'static str {
    match m {
        NdiTransmitMethod::Tcp => "TCP",
        NdiTransmitMethod::Udp => "UDP",
        NdiTransmitMethod::Multicast => "Multicast",
        NdiTransmitMethod::RUdp => "RUDP",
    }
}

fn split_ips(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn none_if_placeholder(s: String) -> Option<String> {
    if s.is_empty() || s.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(s)
    }
}

/// Resolves a desired source name to the (name, ip) pair BirdUI's own
/// "Apply Source" flow submits, using the device's live source list. Falls
/// back to the literal name with an empty ip if the device doesn't
/// currently see that source (e.g. it just went offline) rather than
/// silently dropping the request to clear the source.
fn resolve_source(wanted: &Option<String>, sources: &HashMap<String, String>) -> (String, String) {
    match wanted {
        None => (NONE_SOURCE.to_string(), NONE_SOURCE.to_string()),
        Some(name) => {
            let ip = sources
                .get(name)
                .cloned()
                .unwrap_or_else(|| NONE_SOURCE.to_string());
            (name.clone(), ip)
        }
    }
}

#[async_trait]
impl DeviceClient for HttpDeviceClient {
    async fn status(&self) -> anyhow::Result<DeviceStatus> {
        let html = self.get_page("/dashboard").await?;
        let text = form::scrape_text_by_id(
            &html,
            &[
                "vid_str_name",
                "vid_fmt",
                "aud_stat",
                "vid_res",
                "vid_fr",
                "dashboard_avahi_name",
                "dashboard_vid_status",
                "dashboard_fw_version",
            ],
        );
        let get = |k: &str| text.get(k).cloned().unwrap_or_default();
        Ok(DeviceStatus {
            online: get("dashboard_vid_status") == "active",
            ndi_stream_name: get("vid_str_name"),
            video_format: get("vid_fmt"),
            audio_status: get("aud_stat"),
            video_resolution: get("vid_res"),
            video_frame_rate: get("vid_fr"),
            // Hidden on Play's dashboard variant (`display:none` in the real
            // markup) - not reliably readable.
            average_bitrate_mbps: 0.0,
            firmware_version: get("dashboard_fw_version"),
            system_name: get("dashboard_avahi_name"),
        })
    }

    async fn network_settings(&self) -> anyhow::Result<NetworkSettings> {
        let html = self.get_page("/network").await?;
        let fields = form::scrape_form_fields(&html);
        let get = |k: &str| fields.get(k).cloned().unwrap_or_default();
        Ok(NetworkSettings {
            config_method: if get("net_method") == "static" {
                ConfigMethod::Static
            } else {
                ConfigMethod::Dhcp
            },
            ip_address: get("net_address"),
            subnet_mask: get("net_mask"),
            gateway_address: get("net_gate"),
            dhcp_timeout_secs: get("net_fallback_timeout").parse().unwrap_or(20),
            fallback_ip_address: get("net_fallback_address"),
            fallback_subnet_mask: get("net_fallback_mask"),
            birddog_name: get("net_avahi"),
            ndi_transmit_method: parse_ndi_method(&get("Txpm")),
            ndi_receive_method: parse_ndi_method(&get("Rxpm")),
            multicast_net_prefix: get("Txnetprefix"),
            multicast_net_mask: get("Txnetmask"),
            multicast_ttl: get("Txmcttl").parse().unwrap_or(1),
            ndi_discovery_server_enabled: get("NDIDisServ") == "NDIDisServEn",
            ndi_discovery_server_ips: split_ips(&get("NDIDisServIP")),
        })
    }

    async fn set_network_settings(&self, settings: NetworkSettings) -> anyhow::Result<()> {
        let html = self.get_page("/network").await?;
        let mut fields = form::scrape_form_fields(&html);
        fields.insert(
            "net_method".into(),
            match settings.config_method {
                ConfigMethod::Dhcp => "dhcp".into(),
                ConfigMethod::Static => "static".into(),
            },
        );
        fields.insert("net_address".into(), settings.ip_address);
        fields.insert("net_mask".into(), settings.subnet_mask);
        fields.insert("net_gate".into(), settings.gateway_address);
        fields.insert(
            "net_fallback_timeout".into(),
            settings.dhcp_timeout_secs.to_string(),
        );
        fields.insert("net_fallback_address".into(), settings.fallback_ip_address);
        fields.insert("net_fallback_mask".into(), settings.fallback_subnet_mask);
        fields.insert("net_avahi".into(), settings.birddog_name);
        fields.insert(
            "Txpm".into(),
            ndi_method_str(settings.ndi_transmit_method).to_string(),
        );
        fields.insert(
            "Rxpm".into(),
            ndi_method_str(settings.ndi_receive_method).to_string(),
        );
        fields.insert("Txnetprefix".into(), settings.multicast_net_prefix);
        fields.insert("Txnetmask".into(), settings.multicast_net_mask);
        fields.insert("Txmcttl".into(), settings.multicast_ttl.to_string());
        fields.insert(
            "NDIDisServ".into(),
            if settings.ndi_discovery_server_enabled {
                "NDIDisServEn".into()
            } else {
                "NDIDisServDis".into()
            },
        );
        fields.insert(
            "NDIDisServIP".into(),
            settings.ndi_discovery_server_ips.join(","),
        );
        self.post_form("/network", fields).await
    }

    async fn decode_settings(&self) -> anyhow::Result<DecodeSettings> {
        let html = self.get_page("/videoset").await?;
        let fields = form::scrape_form_fields(&html);
        let get = |k: &str| fields.get(k).cloned().unwrap_or_default();
        Ok(DecodeSettings {
            selected_source: none_if_placeholder(get("dec0_source_name")),
            // flock's own centralized discovery (GET /api/ndi/sources, see
            // crates/discovery) is what the UI suggests from now - this
            // struct no longer duplicates that per device by querying this
            // device's own :8080/List just to populate a picker.
            available_sources: vec![],
            failover_source: none_if_placeholder(get("dec0_fo_source_name")),
            // BirdUI's own JS reads this same hidden marker (not the
            // `selected` attribute) for the page's current value - see
            // docs/architecture.md.
            screensaver_mode: form::scrape_attr_by_id(&html, "dec1_sel", "value")
                .unwrap_or_default(),
            color_space: {
                let v = get("decode_ColorSpace");
                if v.is_empty() {
                    "YUV".to_string()
                } else {
                    v
                }
            },
            ndi_audio_enabled: get("decode_NDIAudio") == "NDIAudioEn",
            tally_mode: get("decode_TallyMode"),
        })
    }

    async fn set_decode_settings(&self, settings: DecodeSettings) -> anyhow::Result<()> {
        let html = self.get_page("/videoset").await?;
        let mut fields = form::scrape_form_fields(&html);

        // The real UI's "Apply Source" flow: look the chosen name up in the
        // device's own live source list to get its "ip:port", set both the
        // name and ip fields, and include the specific button field the
        // server gates source-change handling behind - confirmed live that
        // omitting this button field silently no-ops the source change even
        // though the other decode fields in the same POST do take effect.
        let sources = self.fetch_source_list().await.unwrap_or_default();
        let (source_name, source_ip) = resolve_source(&settings.selected_source, &sources);
        let (fo_name, fo_ip) = resolve_source(&settings.failover_source, &sources);
        fields.insert("dec0_source_name".into(), source_name);
        fields.insert("dec0_source_ip".into(), source_ip);
        fields.insert("dec0_fo_source_name".into(), fo_name);
        fields.insert("dec0_fo_source_ip".into(), fo_ip);
        fields.insert(
            "dec0_change_source_button".into(),
            "dec0_change_source".into(),
        );

        fields.insert("decode_ScreenSaverMode".into(), settings.screensaver_mode);
        fields.insert("decode_ColorSpace".into(), settings.color_space);
        fields.insert(
            "decode_NDIAudio".into(),
            if settings.ndi_audio_enabled {
                "NDIAudioEn".into()
            } else {
                "NDIAudioDis".into()
            },
        );
        fields.insert("decode_TallyMode".into(), settings.tally_mode);
        self.post_form("/videoset", fields).await
    }

    async fn system_settings(&self) -> anyhow::Result<SystemSettings> {
        let status = self.status().await?;
        Ok(SystemSettings {
            firmware_version: status.firmware_version,
            // Write-only on the real device (uploaded files, not readable
            // back through BirdUI) - see the doc comment on the struct.
            remote_ip_list: vec![],
            ndi_group_list: vec![],
        })
    }

    async fn set_system_settings(&self, settings: SystemSettings) -> anyhow::Result<()> {
        if !settings.remote_ip_list.is_empty() {
            self.upload_list_file("/settings", "update_configfile", &settings.remote_ip_list)
                .await?;
        }
        if !settings.ndi_group_list.is_empty() {
            self.upload_list_file("/settings", "update_groupfile", &settings.ndi_group_list)
                .await?;
        }
        Ok(())
    }

    async fn reboot(&self) -> anyhow::Result<()> {
        if !self.logged_in.load(Ordering::Relaxed) {
            self.login().await?;
        }
        // Best-effort: the device may drop the connection as it reboots,
        // which is the expected outcome, not a failure.
        let _ = self
            .http
            .get(format!("{}/reboot", self.base_url))
            .send()
            .await;
        Ok(())
    }
}

struct CachedClient {
    host: String,
    password: String,
    client: Arc<HttpDeviceClient>,
}

/// Hands out one `HttpDeviceClient` per device, rebuilding it if the
/// device's host or password changed since it was cached (e.g. edited via
/// flock's UI) so a stale session/target never lingers.
#[derive(Default)]
pub struct HttpClientProvider {
    clients: RwLock<HashMap<DeviceId, CachedClient>>,
}

impl HttpClientProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DeviceClientProvider for HttpClientProvider {
    fn client_for(&self, device: &Device) -> Arc<dyn DeviceClient> {
        let password = device
            .credentials
            .password
            .clone()
            .unwrap_or_else(|| DEFAULT_PASSWORD.to_string());
        {
            let clients = self.clients.read().expect("http provider lock poisoned");
            if let Some(cached) = clients.get(&device.id) {
                if cached.host == device.host && cached.password == password {
                    return cached.client.clone();
                }
            }
        }
        let client =
            Arc::new(HttpDeviceClient::new(device).expect("failed to build reqwest client"));
        self.clients
            .write()
            .expect("http provider lock poisoned")
            .insert(
                device.id,
                CachedClient {
                    host: device.host.clone(),
                    password,
                    client: client.clone(),
                },
            );
        client
    }
}
