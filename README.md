# flock

> **AI-assisted project.** This codebase was created with
> [Claude](https://claude.com/claude-code) (Anthropic), directed and
> reviewed by a human author. Treat this as an early-stage hobby project:
> Phase 1 has been built and exercised end-to-end against a simulated
> BirdDog Play device (see [Status](#status) below), but **has not yet been
> run against real BirdDog hardware**. Review before relying on it for
> anything live.

A single web UI for managing any number of [BirdDog Play](https://birddog.tv/play-overview/)
NDI/SRT decoders — a fleet control panel for devices that otherwise only
have their own individual [BirdUI](https://birddog.tv/birdui-overview/) web
interface. Discover Play units on the LAN or add them manually, tag each
into (multiple) groups, see/change every BirdUI setting for a selected
device from one un-nested, tabbed view, or push a setting to an entire
group at once.

## Screenshots

*Real screenshots of flock running locally against the three seeded
`device-mock` devices (see [Status](#status)) — not mockups.*

**Overview** — device list grouped by tag on the left (a device can sit in
more than one group), preview + settings in the center, discovery/add/local
settings on the right:

![flock overview: three devices grouped by tag, no device selected yet](docs/screenshots/overview.png)

**Status** tab — the per-device dashboard:

![flock Status tab showing a device's dashboard summary](docs/screenshots/status.png)

**Network** tab — Ethernet/Wi-Fi, NDI transmit method, multicast, and
discovery server settings, all in one flat panel:

![flock Network tab with DHCP/static, NDI transmit method, and multicast fields](docs/screenshots/network.png)

**Decode** tab — NDI source selection and failover (Play is decode-only, so
there is no Encode tab):

![flock Decode tab showing NDI source and failover fields](docs/screenshots/decode.png)

**System** tab — firmware, Access Manager lists, and UI mode:

![flock System tab showing firmware version and Access Manager fields](docs/screenshots/system.png)

**Batch edit** — select a group chip, click "Batch edit", and every field
starts blank/"leave unchanged"; only fields you actually fill in are sent,
merged into each member device's own current settings rather than
overwriting the whole group with a shared template:

![flock batch-editing the Network tab for a two-device group, all fields blank except one changed field](docs/screenshots/batch.png)

## What it does

- **Device registry**: any number of BirdDog Play devices, each taggable
  into multiple groups (a device isn't locked to one group).
- **Discovery**: mDNS scan for candidate devices on the LAN, plus manual
  add-by-host as a fallback that always works.
- **Full BirdUI parity for Play** (decode-only, so no Encode tab): Status
  (Dashboard), Network, Decode (NDI source + failover), and System
  (password/firmware/Access Manager/UI mode) — every field visible directly
  in its tab, nothing behind a submenu.
- **Batch edit by group**: apply a Network/Decode/System change to every
  device in a group at once. Blank fields mean "leave unchanged" — a batch
  save merges into each device's own current settings rather than
  clobbering the whole group with one shared template.
- **Live updates**: a WebSocket pushes registry/status changes to every open
  browser tab.
- **Runs in Docker**: `docker compose up` — see the networking note below.

## Status

**Phase 1 (current): mock-first, built and passing locally.** A simulated
BirdDog Play (`crates/device-mock`) stands in for real hardware so the whole
app — registry, groups, all four settings tabs, batch edit, discovery,
Docker — can be built and demoed before any physical unit is on the bench.

Working:
- Cargo workspace (`core`/`discovery`/`device-mock`/`web`/`flock`),
  `cargo build`/`clippy`/`test` all clean
- Three-pane UI: device list + tag-derived groups on the left, preview +
  tabbed settings in the center, discovery/add/remove/local settings on the
  right
- Every settings tab round-trips against the mock device, single-device or
  batched across a whole group
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
