//! Two unrelated kinds of LAN discovery live here - keep them separate,
//! they answer different questions:
//!
//! - **Device discovery** (`scan`): "which hosts are BirdDog Play units I
//!   could add to flock." A real Play doesn't advertise itself over mDNS at
//!   all (not under `_ndi._tcp`, `_http._tcp`, or `_workstation._tcp` -
//!   confirmed against real hardware, see docs/architecture.md), so this is
//!   an active subnet probe (`subnet_probe`) checking each host's `GET /`
//!   for BirdUI's signature. Manual add-by-host is always available
//!   regardless, so gaps here degrade gracefully.
//! - **NDI source discovery** (`ndi_sources`): "which NDI senders exist on
//!   the network right now, and at what address." This is the thing a Play
//!   itself needs to be *pointed at* to decode - genuinely found via
//!   standard mDNS `_ndi._tcp.local.` browsing (the pattern already proven
//!   in the Dante-BabelBox project), since NDI senders (cameras, software
//!   like Mitti, this Mac's own NDI output) do advertise themselves that
//!   way. This used to be folded into `scan`'s results, which was actively
//!   misleading: an NDI sender showing up in the "devices to add" list
//!   isn't a Play and adding it as one would just fail (it doesn't run
//!   BirdUI). Centralizing this list here is also what replaces having to
//!   query each individual Play's own `:8080/List` endpoint just to know
//!   what's available - see `crates/device-http` and docs/architecture.md.

mod subnet_probe;

use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::Serialize;
use std::time::Duration;

const NDI_SERVICE_TYPE: &str = "_ndi._tcp.local.";

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredHost {
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct NdiSource {
    pub name: String,
    /// "ip:port", the same shape BirdUI's own source-apply flow expects.
    pub address: String,
}

pub struct Discovery {
    daemon: ServiceDaemon,
}

impl Discovery {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            daemon: ServiceDaemon::new()?,
        })
    }

    /// Candidate BirdDog Play hosts not yet in the registry. Subnet-probe
    /// only - see the module doc for why mDNS isn't part of this.
    pub async fn scan(&self) -> anyhow::Result<Vec<DiscoveredHost>> {
        subnet_probe::probe_lan().await
    }

    /// Every NDI source currently visible over mDNS, deduped by name.
    pub async fn ndi_sources(&self, timeout: Duration) -> anyhow::Result<Vec<NdiSource>> {
        let receiver = self.daemon.browse(NDI_SERVICE_TYPE)?;
        let mut found = Vec::new();
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, receiver.recv_async()).await {
                Ok(Ok(ServiceEvent::ServiceResolved(info))) => {
                    if let Some(ip) = info.get_addresses_v4().into_iter().next() {
                        // get_fullname() is "Instance Name._ndi._tcp.local." -
                        // strip the service-type suffix so this matches the
                        // plain instance name BirdUI/NDI tools show (and
                        // that a Play's own :8080/List keys its map by).
                        let suffix = format!(".{}", info.get_type());
                        let name = info
                            .get_fullname()
                            .strip_suffix(&suffix)
                            .unwrap_or(info.get_fullname())
                            .to_string();
                        found.push(NdiSource {
                            name,
                            address: format!("{ip}:{}", info.get_port()),
                        });
                    }
                }
                Ok(Ok(_)) => continue,
                Ok(Err(_)) | Err(_) => break,
            }
        }

        let _ = self.daemon.stop_browse(NDI_SERVICE_TYPE);
        found.sort_by(|a, b| a.name.cmp(&b.name));
        found.dedup_by(|a, b| a.name == b.name);
        Ok(found)
    }
}
