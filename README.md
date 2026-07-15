# flock

> **AI-assisted project.** This codebase was created with
> [Claude](https://claude.com/claude-code) (Anthropic), directed and
> reviewed by a human author. Treat this as an early-stage hobby project:
> Phase 1 has been built and exercised end-to-end against a simulated
> BirdDog Play device (see [Status](#status) below), but **has not yet been
> run against real BirdDog hardware**. Review before relying on it for
> anything live.

A single web UI for managing any number of [BirdDog Play](https://birddog.tv/play-overview/)
NDI/SRT converters — a fleet control panel for devices that otherwise only
have their own individual [BirdUI](https://birddog.tv/birdui-overview/) web
interface. Discover Play units on the LAN or add them manually, tag each
into (multiple) groups, and see/change every BirdUI setting for a selected
device from one un-nested, tabbed view.

## What it does

- **Device registry**: any number of BirdDog Play devices, each taggable
  into multiple groups (a device isn't locked to one group).
- **Discovery**: mDNS scan for candidate devices on the LAN, plus manual
  add-by-host as a fallback that always works.
- **Full BirdUI parity for Play**: Status (Dashboard), Network, Encode
  (primary NDI HX/UVC + secondary SRT/RTMP-RTSP), Decode (NDI source +
  failover), and System (password/firmware/Access Manager/UI mode) — every
  field visible directly in its tab, nothing behind a submenu.
- **Live updates**: a WebSocket pushes registry/status changes to every open
  browser tab.
- **Runs in Docker**: `docker compose up` — see the networking note below.

## Status

**Phase 1 (current): mock-first, built and passing locally.** A simulated
BirdDog Play (`crates/device-mock`) stands in for real hardware so the whole
app — registry, groups, all five settings tabs, discovery, Docker — can be
built and demoed before any physical unit is on the bench.

Working:
- Cargo workspace (`core`/`discovery`/`device-mock`/`web`/`flock`),
  `cargo build`/`clippy`/`test` all clean
- Three-pane UI: device list + tag-derived groups on the left, preview +
  tabbed settings in the center, discovery/add/remove/local settings on the
  right
- Every settings tab round-trips against the mock device
- mDNS discovery scan + manual add/edit/remove
- `docker-compose.yml` with host networking for mDNS

Not yet done:
- Never run against a real BirdDog Play — the mDNS service type it actually
  advertises and the real REST field names are unconfirmed (see
  [docs/architecture.md](docs/architecture.md))
- Live video preview is a placeholder (needs an actual NDI/SRT frame grab)
- No auth on flock itself — meant for a trusted LAN, same trust model as the
  device's own BirdUI

## Quick start

```bash
cargo run -p flock
```

Then open `http://localhost:8080`. On first run with an empty registry it
seeds three demo devices so there's something to look at immediately.

### Docker

```bash
docker compose up --build
```

Uses `network_mode: host` so mDNS discovery (UDP multicast) works from
inside the container — see [docs/architecture.md](docs/architecture.md#docker--mdns)
for the tradeoff and the bridge-networking alternative if you'd rather keep
container isolation and rely on manual add only.

## Architecture

See [docs/architecture.md](docs/architecture.md) for the crate layout, the
`DeviceClient` trait that isolates real-hardware integration to one seam,
and why several BirdDog-specific details (mDNS service type, REST field
names) are marked as unconfirmed pending real hardware.

## Roadmap

See [docs/roadmap.md](docs/roadmap.md).
