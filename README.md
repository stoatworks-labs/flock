# flock

> **AI-assisted project.** This codebase was created with
> [Claude](https://claude.com/claude-code) (Anthropic), directed and
> reviewed by a human author. Treat this as an early-stage hobby project:
> it's been exercised end-to-end against both a simulated BirdDog Play
> device and a real one — including live reads and a real settings write
> (routing an actual NDI source to a physical unit's HDMI output) — see
> [Status](#status) below for exactly what that covers and what's still
> unverified.

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

**Overview** — a nested group tree on the left (a device can sit in more
than one group, and appears under each), preview + settings in the center,
discovery/add/local settings on the right:

![flock overview: nested group tree with devices under All devices/backup/lobby/primary/stage, no device selected yet](docs/screenshots/overview.png)

**Status** tab — the per-device dashboard:

![flock Status tab showing a device's dashboard summary](docs/screenshots/status.png)

**Network** tab — IP config, NDI transmit/receive preferred method,
multicast, and discovery server settings, all in one flat panel:

![flock Network tab with DHCP/static, NDI transmit method, and multicast fields](docs/screenshots/network.png)

**Decode** tab — source selection, failover, screensaver, colour space, NDI
audio, and tally (Play is decode-only, so there is no Encode tab):

![flock Decode tab showing NDI source and failover fields](docs/screenshots/decode.png)

**System** tab — firmware version and Access Manager lists:

![flock System tab showing firmware version and Access Manager fields](docs/screenshots/system.png)

**Batch edit** — groups are a nested tree in the left panel; click a
group's header to batch-edit every member at once, or expand it to drill
into an individual device. Every field starts blank/"leave unchanged"; only
fields you actually fill in are sent, merged into each member device's own
current settings rather than overwriting the whole group with a shared
template:

![flock batch-editing the Network tab for a two-device group, all fields blank except one changed field](docs/screenshots/batch.png)

## What it does

- **Device registry**: any number of BirdDog Play devices, each taggable
  into multiple groups (a device isn't locked to one group).
- **Discovery**: an active LAN subnet probe (the actual way a real Play is
  found — it doesn't advertise itself over mDNS at all), manual add-by-host
  as a fallback that always works, and a *separate*, centralized NDI source
  list (mDNS) that suggests values in the Decode tab — flock discovers NDI
  sources once, itself, instead of each Play searching independently, the
  same control-plane-only model real NDI routers (BirdDog's own Keyboard,
  Vizrt's NDI Router) use — see [docs/architecture.md](docs/architecture.md).
- **Full BirdUI parity for Play** (decode-only, so no Encode tab): Status
  (Dashboard), Network, Decode (NDI source + failover), and System
  (password/firmware/Access Manager/UI mode) — every field visible directly
  in its tab, nothing behind a submenu.
- **Nested groups, one click to batch-edit**: groups are a vertical tree in
  the left panel (a device can sit in more than one, appearing under each);
  click a group's header to apply a Network/Decode/System change to every
  member at once, or expand it to drill into an individual device. Blank
  fields mean "leave unchanged" — a batch save merges into each device's own
  current settings rather than clobbering the whole group with one shared
  template.
- **NDI Discovery Server, fleet-wide**: set it once in Local App Settings and
  push it to every registered Play's own Network settings in one click
  (flock can't itself query a Discovery Server — no public protocol spec —
  but every Play can, so this configures them to).
- **Live updates**: a WebSocket pushes registry/status changes to every open
  browser tab.
- **Runs in Docker**: `docker compose up` — see the networking note below.

## Status

**Phase 2 (current): validated against both a simulated and a real BirdDog
PLAY.** `crates/device-mock` stands in for hardware for quick iteration/demo;
`crates/device-http` is a real client confirmed against an actual PLAY unit
(firmware 1.0.18) — see [docs/architecture.md](docs/architecture.md)'s
"Confirmed against real hardware" section for exactly what that means and
its known limitations.

Working:
- Cargo workspace (`core`/`discovery`/`device-mock`/`device-http`/`web`/
  `flock`), `cargo build`/`clippy`/`test` all clean, including offline unit
  tests for the real HTML scraper against fixtures captured from actual
  hardware
- Three-pane UI: device list + tag-derived groups on the left, preview +
  tabbed settings in the center, discovery/add/remove/local settings on the
  right
- Every settings tab round-trips against the mock device, single-device or
  batched across a whole group
- Reads (`status`/`network_settings`/`decode_settings`) **and** a real
  settings write both verified live against physical hardware — routed an
  actual NDI source to a real Play's HDMI output through flock's own API,
  read the change back, switched it twice more. Along the way, found (by
  testing, not guessing) that the decode-source picker needed a separate
  JSON API on port 8080 and a specific button field the server silently
  ignores requests without — see [docs/architecture.md](docs/architecture.md)
- Subnet-probe + mDNS discovery scan + manual add/edit/remove
- `docker-compose.yml` with host networking (needed for the subnet probe and
  mDNS alike)

Not yet done:
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

Uses `network_mode: host` so both discovery mechanisms (the subnet probe and
mDNS) can reach the LAN from inside the container — see
[docs/architecture.md](docs/architecture.md#docker--networking) for the
tradeoff and the bridge-networking alternative if you'd rather keep
container isolation and rely on manual add only.

### Desktop app

Prefer not to touch the terminal? A small menu-bar app lets you pick the network
interface + port, Start/Stop the server, and open the web UI. The `flock` server
is bundled inside, so it's a single download — nothing to install or wire up.
Grab the `.dmg` from [Releases](https://github.com/allansargeant/flock/releases),
or see [launcher/](launcher/) to build it.

<p align="center"><img src="launcher/docs/panel.png" width="300" alt="flock desktop app"></p>

## Architecture

```mermaid
flowchart LR
    P1["BirdDog Play<br/>(real hardware)"] <-- HTTP/HTML --> DCH["device-http"]
    P2["device-mock<br/>(simulated Play)"]
    DCH --> DC["DeviceClient trait<br/>(real-hardware seam)"]
    P2 --> DC
    DC --> REG["Registry + tag groups<br/>(core)"]
    REG --> WEB["web (axum)"]
    WEB -- WebSocket --> UI["Browser fleet UI<br/>tabbed settings + batch edit"]
```

See [docs/architecture.md](docs/architecture.md) for the crate layout, the
`DeviceClient` trait that isolates real-hardware integration to one seam,
and the full list of what's confirmed against real hardware vs. still
unconfirmed/unimplemented.

## Roadmap / TODO

Full plan in [docs/roadmap.md](docs/roadmap.md). Next up:

- [ ] **Subscribe to the real device's live status WebSocket** instead of polling `/dashboard`.
- [ ] **Real live video preview** — an actual NDI/SRT frame grab (currently a placeholder).
- [ ] **Optional auth on flock itself** — currently trusted-LAN only, matching BirdUI's own model.
