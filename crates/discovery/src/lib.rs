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
//!
//! Both of those only run when flock drives real hardware. Under
//! `provider = "mock"` the binary builds `Discovery::mock()` instead, which
//! answers both questions from canned data (`mock`) and never touches the
//! network - the discovery counterpart of `crates/device-mock`.

mod mock;
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
    source: Source,
}

enum Source {
    /// The real network: subnet probe for Plays, mDNS browse for NDI senders.
    Network(ServiceDaemon),
    /// Canned answers from `mock`. Sends nothing anywhere.
    Mock,
}

impl Discovery {
    /// Discovery against the real network, for `provider = "http"`.
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            source: Source::Network(ServiceDaemon::new()?),
        })
    }

    /// Simulated discovery for `provider = "mock"`: canned, plausible hosts
    /// and NDI sources, with no subnet sweep and no mDNS daemon.
    pub fn mock() -> Self {
        Self {
            source: Source::Mock,
        }
    }

    /// True for `Discovery::mock()`.
    pub fn is_mock(&self) -> bool {
        matches!(self.source, Source::Mock)
    }

    /// Candidate BirdDog Play hosts not yet in the registry. Subnet-probe
    /// only - see the module doc for why mDNS isn't part of this.
    pub async fn scan(&self) -> anyhow::Result<Vec<DiscoveredHost>> {
        match &self.source {
            Source::Network(_) => subnet_probe::probe_lan().await,
            Source::Mock => Ok(mock::hosts()),
        }
    }

    /// Every NDI source currently visible over mDNS, deduped by name. The
    /// mock answers at once; `timeout` only bounds the real browse.
    pub async fn ndi_sources(&self, timeout: Duration) -> anyhow::Result<Vec<NdiSource>> {
        match &self.source {
            Source::Network(daemon) => browse_ndi_sources(daemon, timeout).await,
            Source::Mock => Ok(mock::ndi_sources()),
        }
    }
}

async fn browse_ndi_sources(
    daemon: &ServiceDaemon,
    timeout: Duration,
) -> anyhow::Result<Vec<NdiSource>> {
    let receiver = daemon.browse(NDI_SERVICE_TYPE)?;
    let mut found = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, receiver.recv_async()).await {
            Ok(Ok(ServiceEvent::ServiceResolved(info))) => {
                if let Some(ip) = info
                    .addresses
                    .iter()
                    .filter(|a| a.is_ipv4())
                    .map(|a| a.to_ip_addr())
                    .next()
                {
                    // get_fullname() is "Instance Name._ndi._tcp.local." -
                    // strip the service-type suffix so this matches the
                    // plain instance name BirdUI/NDI tools show (and
                    // that a Play's own :8080/List keys its map by).
                    let suffix = format!(".{}", info.ty_domain);
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

    let _ = daemon.stop_browse(NDI_SERVICE_TYPE);
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found.dedup_by(|a, b| a.name == b.name);
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::net::{Ipv4Addr, SocketAddrV4};

    /// The simulated Plays report this as their own subnet (device-mock's
    /// `NetworkSettings`), so the mock's addresses stay inside it.
    fn on_mock_subnet(ip: Ipv4Addr) -> bool {
        ip.octets()[..3] == [192, 168, 100]
    }

    #[tokio::test]
    async fn mock_scan_reports_hosts_the_way_the_probe_does() {
        let hosts = Discovery::mock().scan().await.unwrap();
        assert!(!hosts.is_empty());
        for h in &hosts {
            let ip: Ipv4Addr = h.host.parse().expect("a probe hit is an IPv4 address");
            assert!(on_mock_subnet(ip), "{ip} is off the mock subnet");
            // subnet_probe::probe_one names a hit by its IP, on port 80.
            assert_eq!(h.name, h.host);
            assert_eq!(h.port, 80);
        }
        let unique: HashSet<_> = hosts.iter().map(|h| &h.host).collect();
        assert_eq!(unique.len(), hosts.len());
    }

    #[tokio::test]
    async fn mock_scan_skips_nothing_the_seeded_registry_has() {
        // The seeded demo devices use .local hostnames; a mock scan hit that
        // matched one would be filtered out of /api/discovery/scan as
        // already added, leaving the demo's scan empty.
        let hosts = Discovery::mock().scan().await.unwrap();
        assert!(hosts.iter().all(|h| !h.host.ends_with(".local")));
    }

    #[tokio::test]
    async fn mock_ndi_sources_match_the_mdns_shape() {
        let sources = Discovery::mock()
            .ndi_sources(Duration::from_secs(3))
            .await
            .unwrap();
        assert!(!sources.is_empty());
        for s in &sources {
            let addr: SocketAddrV4 = s.address.parse().expect("address is ip:port");
            assert!(on_mock_subnet(*addr.ip()), "{addr} is off the mock subnet");
            // NDI instance names are "MACHINE (Source)", with no
            // ._ndi._tcp.local. suffix left on.
            assert!(s.name.ends_with(')') && s.name.contains(" ("), "{}", s.name);
            assert!(!s.name.contains(NDI_SERVICE_TYPE));
        }
        // The real browse sorts by name and dedups by name.
        let names: Vec<_> = sources.iter().map(|s| s.name.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names, sorted);
        let addresses: HashSet<_> = sources.iter().map(|s| &s.address).collect();
        assert_eq!(addresses.len(), sources.len());
    }

    #[tokio::test]
    async fn mock_ndi_sources_answer_without_waiting_out_the_timeout() {
        // The real browse always listens for the whole timeout; the mock has
        // nothing to listen to, so a long timeout must not hold it up.
        let answer = tokio::time::timeout(
            Duration::from_secs(1),
            Discovery::mock().ndi_sources(Duration::from_secs(60)),
        )
        .await
        .expect("mock answered within a second");
        assert!(!answer.unwrap().is_empty());
    }
}
