# Architecture

## Core model

`crates/core` is transport-agnostic, matching the shape of srt-router's
`crosspoint-core`: it knows nothing about HTTP, mDNS, or how a device is
actually reached.

- **`Device`** — pure metadata: id, name, host, tags, credentials, and
  whether it was discovered or added manually. Play is decode-only (NDI/SRT
  source → HDMI out), so there is no per-device encode/decode mode.
- **`Registry`** — an `Arc`-shareable, JSON-file-persisted map of devices.
  Groups are *derived*, not stored: `Registry::groups()` scans every
  device's `tags` and buckets ids by tag. That's what gives "one device, N
  groups" for free — there's no group entity to keep in sync. It's also what
  batch edit targets: applying a settings patch to a group just means
  applying it to every device id that tag currently maps to.
- **`DeviceClient`** (trait) — everything flock can ask a device to do:
  read/write its Status, Network, Decode, and System settings, plus reboot.
  This is the seam between "what flock knows about a device" and
  "how flock actually talks to it." `crates/device-mock` is the only
  implementation today; a real HTTP implementation is Phase 2 work and
  slots in without touching the registry, the web API, or the frontend.
- **`DeviceClientProvider`** (trait) — resolves a `Device` to its
  `DeviceClient`. The binary wires in whichever provider is active
  (`MockClientProvider` today).

## Why a trait instead of calling a real BirdDog API directly

The BirdDog RESTful API's public documentation doesn't currently resolve to
a working spec (the linked PDF 404s / redirects to the homepage as of this
writing), and the shape inferred from a community Python wrapper
(session `/login`, `GET /hostname`/`/operationmode`, `POST /videoset`,
`GET/POST /connectTo` for NDI decode source, `/reboot`) is not confirmed
against current firmware. Rather than hardcode a guess, every device
operation goes through `DeviceClient`, so the field names and endpoints only
need to be right in exactly one place (a future `flock-device-http` crate)
once there's real hardware to validate against — see
[roadmap.md](roadmap.md).

Similarly, the exact mDNS service type a real Play advertises for discovery
is unconfirmed - `crates/discovery` browses `_ndi._tcp.local.` (the one
documented, standard NDI service type) as a best-effort default. Manual
add-by-host always works independent of discovery, so an imperfect or
incomplete discovery result never blocks getting a device under management.

## Settings tabs mirror BirdUI panels

Per-device settings map directly onto the BirdUI User Guide's own panel
grouping - Dashboard → Status, Network, AV Setup's Decode Settings, and
System (password/firmware/Access Manager/UI mode) - deliberately excluding
both the Encode Settings panel (Play doesn't encode) and the camera-only
panels (Cam Control, AI Tracking, Exposure/White Balance/Picture/Colour
Matrix) that don't apply to a Play converter either. Every field for the
active tab renders flat in one bordered panel; there is no nested submenu
anywhere in the UI, mirroring the user's explicit requirement.

## Batch edit

Selecting a group and clicking "Batch edit" swaps the center panel into a
group-scoped version of the Network/Decode/System tabs (Status is dropped -
it's inherently per-device). Every field starts blank/"leave unchanged"
rather than prefilled from any one device, since prefilling from a single
member would misrepresent the others. Only fields the operator actually
touches are sent as a JSON patch to `POST /api/groups/:tag/:tab`; the
handler fetches each member device's own current settings, shallow-merges
the patch into that JSON, and writes the merged result back per device
(`crates/web/src/handlers.rs::apply_group_settings`). This means an
untouched field keeps whatever that specific device already had - a batch
edit narrows to "change this one thing everywhere" instead of "reset the
whole group to a shared template."

## Web layer

`crates/web` is axum, following srt-router's split:

- `GET /` , `/app.js`, `/style.css` — static frontend, `include_str!`'d into
  the binary, no bundler (same choice srt-router made).
- `GET /api/state` — full registry snapshot (devices + derived groups), used
  for first paint.
- `GET /ws` — one-way server→client push. Polls the registry every 750ms and
  sends a fresh snapshot only when it changed, so idle clients cost nothing
  extra and every open tab stays in sync.
- `POST/PUT/DELETE /api/devices[...]` — manual add/edit/remove.
- `GET /api/discovery/scan` — runs an mDNS browse and returns hosts not
  already in the registry.
- `GET/POST /api/devices/:id/{network,decode,system}` — per-tab settings
  read/write, routed through `DeviceClient`.
- `POST /api/groups/:tag/{network,decode,system}` — batch edit: merges a
  partial JSON patch into every group member's current settings for that
  tab (see Batch edit above).

## Docker + mDNS

mDNS discovery relies on UDP multicast (port 5353), which Docker's default
bridge network does not reliably forward. `docker-compose.yml` uses
`network_mode: host` so discovery works out of the box; this trades away
container network isolation. If that tradeoff isn't acceptable for your
deployment, switch to bridge networking with a `ports:` mapping - discovery
just won't find anything, and manual add-by-host keeps working.
