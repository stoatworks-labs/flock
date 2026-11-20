//! Simulated discovery for `provider = "mock"` - the discovery counterpart of
//! `crates/device-mock`.
//!
//! A mock-mode flock (development, tests, the hosted demo's recording) talks
//! to simulated Plays, so a real subnet sweep or a real mDNS browse would only
//! mix the wrong world into it: hosts it can't control (adding one just gets
//! another simulated device) and NDI senders from whichever network the
//! machine happens to be on. Worse, the hosted demo records these responses,
//! so a real scan would publish that network's addresses. Here both answers
//! are canned and produced by code; nothing is sent anywhere.
//!
//! The shapes follow the real paths exactly: a scan hit is named by its IP on
//! port 80, as `subnet_probe::probe_one` reports one, and NDI sources are
//! plain `MACHINE (Source)` instance names at `ip:port`, sorted by name, as
//! the mDNS browse returns them. The addresses sit in 192.168.100.0/24, the
//! subnet the simulated Plays report as their own (see device-mock's
//! `NetworkSettings`).

use crate::{DiscoveredHost, NdiSource};

/// Two Plays on the simulated subnet that aren't in the registry yet. The
/// seeded demo devices use `.local` hostnames, so neither is filtered out of
/// a scan as already added.
const HOSTS: &[&str] = &["192.168.100.41", "192.168.100.42"];

/// `(instance name, ip:port)`, already sorted by name. NDI senders take
/// ports from 5961 upward, one per source on the same machine.
const NDI_SOURCES: &[(&str, &str)] = &[
    ("CAM-1 (Stage Left)", "192.168.100.21:5961"),
    ("CAM-2 (Stage Right)", "192.168.100.22:5961"),
    ("GFX (Lower Thirds)", "192.168.100.31:5961"),
    ("PLAYBACK (Preview)", "192.168.100.32:5962"),
    ("PLAYBACK (Program)", "192.168.100.32:5961"),
];

pub(crate) fn hosts() -> Vec<DiscoveredHost> {
    HOSTS
        .iter()
        .map(|ip| DiscoveredHost {
            name: ip.to_string(),
            host: ip.to_string(),
            port: 80,
        })
        .collect()
}

pub(crate) fn ndi_sources() -> Vec<NdiSource> {
    NDI_SOURCES
        .iter()
        .map(|(name, address)| NdiSource {
            name: name.to_string(),
            address: address.to_string(),
        })
        .collect()
}
