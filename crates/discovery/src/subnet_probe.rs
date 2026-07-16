//! Active LAN probe, the actual way flock finds real BirdDog PLAY units.
//!
//! Confirmed against real hardware (see docs/architecture.md): a Play does
//! **not** advertise itself over mDNS under `_ndi._tcp`, `_http._tcp`, or
//! `_workstation._tcp` - the standard PTR-record service types this crate's
//! mDNS browse checks. It only carries a plain hostname (A record), which
//! is why nothing shows up via `dns-sd -B`/`ServiceDaemon::browse` no matter
//! how it's discovered elsewhere (e.g. `arp -a`). So instead of asking "who
//! advertises a service," flock actively sweeps the local subnet and asks
//! each live host directly: "does your `GET /` look like BirdUI?"
//!
//! The signature check is cheap: BirdUI's `/` always redirects to
//! `/dashboard` and sets a `BirdDogSession` cookie on that response, even
//! when unauthenticated - detectable from just the response headers,
//! without following the redirect or waiting on the (observed to be slow,
//! several-second) page render.

use std::net::Ipv4Addr;
use std::time::Duration;

use crate::DiscoveredHost;

const MAX_HOSTS_TO_PROBE: usize = 512;
const PROBE_CONCURRENCY: usize = 64;
const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);
const BIRDUI_COOKIE_SIGNATURE: &str = "BirdDogSession";

pub async fn probe_lan() -> anyhow::Result<Vec<DiscoveredHost>> {
    let candidates = local_ipv4_candidates();
    if candidates.is_empty() {
        return Ok(vec![]);
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(PROBE_TIMEOUT)
        .build()?;

    use futures::stream::StreamExt;
    let found = futures::stream::iter(candidates)
        .map(|ip| {
            let client = client.clone();
            async move { probe_one(&client, ip).await }
        })
        .buffer_unordered(PROBE_CONCURRENCY)
        .filter_map(|res| async move { res })
        .collect::<Vec<_>>()
        .await;

    Ok(found)
}

async fn probe_one(client: &reqwest::Client, ip: Ipv4Addr) -> Option<DiscoveredHost> {
    let url = format!("http://{ip}/");
    let resp = client.get(&url).send().await.ok()?;
    let is_birdui = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .any(|v| {
            v.to_str()
                .map(|s| s.contains(BIRDUI_COOKIE_SIGNATURE))
                .unwrap_or(false)
        });
    if !is_birdui {
        return None;
    }
    Some(DiscoveredHost {
        name: ip.to_string(),
        host: ip.to_string(),
        port: 80,
    })
}

/// Every non-loopback IPv4 host address on directly-attached local subnets,
/// skipping any subnet too large to sweep safely.
fn local_ipv4_candidates() -> Vec<Ipv4Addr> {
    let mut candidates = Vec::new();
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return candidates;
    };
    for iface in interfaces {
        if iface.is_loopback() {
            continue;
        }
        let if_addrs::IfAddr::V4(v4) = iface.addr else {
            continue;
        };
        let ip = u32::from(v4.ip);
        let mask = u32::from(v4.netmask);
        let network = ip & mask;
        let host_bits = 32 - mask.count_ones();
        if !(1..=10).contains(&host_bits) {
            // Bigger than a /22 (>1024 addresses) is too large to sweep
            // safely/quickly; a mask with no host bits isn't a LAN segment.
            continue;
        }
        let host_count = 1u32 << host_bits;
        for i in 1..host_count.saturating_sub(1) {
            candidates.push(Ipv4Addr::from(network | i));
            if candidates.len() >= MAX_HOSTS_TO_PROBE {
                return candidates;
            }
        }
    }
    candidates
}
