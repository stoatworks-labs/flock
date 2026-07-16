//! LAN discovery of candidate BirdDog Play hosts.
//!
//! Two independent mechanisms feed into `Discovery::scan`:
//! - An mDNS browse for `_ndi._tcp.local.` (via `mdns-sd`, the pattern
//!   already proven in the Dante-BabelBox project) - catches genuine NDI
//!   sources on the network. Confirmed *not* to catch a real Play itself
//!   (see `subnet_probe` and docs/architecture.md) but kept since it's
//!   harmless and may catch other NDI gear worth surfacing.
//! - An active subnet probe (`subnet_probe`) that's the actual way a real
//!   Play gets found: it doesn't advertise any mDNS service, so flock
//!   sweeps the local subnet and checks each host's `GET /` for BirdUI's
//!   signature instead.
//!
//! Manual add in the UI is always available regardless of what either
//! mechanism finds, so gaps here degrade gracefully rather than blocking
//! device setup.

mod subnet_probe;

use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::Serialize;
use std::time::Duration;

const SERVICE_TYPE: &str = "_ndi._tcp.local.";

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredHost {
    pub name: String,
    pub host: String,
    pub port: u16,
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

    /// Runs the mDNS browse (bounded by `timeout`) and the subnet probe
    /// concurrently and returns the merged, deduped result. Safe to call
    /// repeatedly.
    pub async fn scan(&self, timeout: Duration) -> anyhow::Result<Vec<DiscoveredHost>> {
        let (mdns_result, probe_result) =
            tokio::join!(self.scan_mdns(timeout), subnet_probe::probe_lan());

        let mut found = mdns_result.unwrap_or_else(|e| {
            tracing::warn!("mDNS scan failed: {e:#}");
            vec![]
        });
        found.extend(probe_result.unwrap_or_else(|e| {
            tracing::warn!("subnet probe failed: {e:#}");
            vec![]
        }));

        found.sort_by(|a, b| a.host.cmp(&b.host));
        found.dedup_by(|a, b| a.host == b.host);
        Ok(found)
    }

    async fn scan_mdns(&self, timeout: Duration) -> anyhow::Result<Vec<DiscoveredHost>> {
        let receiver = self.daemon.browse(SERVICE_TYPE)?;
        let mut found = Vec::new();
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, receiver.recv_async()).await {
                Ok(Ok(ServiceEvent::ServiceResolved(info))) => {
                    found.push(DiscoveredHost {
                        name: info.get_fullname().to_string(),
                        host: info.get_hostname().trim_end_matches('.').to_string(),
                        port: info.get_port(),
                    });
                }
                Ok(Ok(_)) => continue,
                Ok(Err(_)) | Err(_) => break,
            }
        }

        let _ = self.daemon.stop_browse(SERVICE_TYPE);
        Ok(found)
    }
}
