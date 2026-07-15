//! LAN discovery of candidate BirdDog Play hosts via mDNS, reusing the
//! `mdns-sd` crate/pattern already proven in the Dante-BabelBox project.
//!
//! The exact mDNS service type a real Play advertises is unconfirmed (see
//! docs/architecture.md) - `_ndi._tcp.local.` is the one standard, documented
//! service type in the research this project is based on. Manual add in the
//! UI is always available regardless of what discovery finds, so an
//! incorrect/incomplete service type here degrades gracefully rather than
//! blocking device setup.

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

    /// Browses for up to `timeout` and returns whatever responded in that
    /// window. Safe to call repeatedly (each call re-browses).
    pub async fn scan(&self, timeout: Duration) -> anyhow::Result<Vec<DiscoveredHost>> {
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
        found.sort_by(|a, b| a.name.cmp(&b.name));
        found.dedup_by(|a, b| a.host == b.host);
        Ok(found)
    }
}
