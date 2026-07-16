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
  "how flock actually talks to it." Two implementations exist:
  `flock-device-mock` (the simulated Play, default) and `flock-device-http`
  (a real HTTP client, confirmed against actual hardware — see below).
- **`DeviceClientProvider`** (trait) — resolves a `Device` to its
  `DeviceClient`. The binary picks the provider at startup from
  `Config::provider` (`"mock"` or `"http"`).

## Confirmed against real hardware

Everything in this section was verified by logging into an actual BirdDog
PLAY unit (firmware `BirdDog PLAY 1.0.18`) rather than inferred from
secondhand documentation. It supersedes the guesses this project started
with.

- **BirdUI is server-rendered HTML, not a JSON API.** Real pages:
  `/dashboard`, `/network`, `/settings`, `/videoset` (AV Setup), `/logout`.
  Login is `POST /login` with a single `auth_password` field; the session
  lives in a `BirdDogSession` cookie. `flock-device-http` logs in lazily,
  detects an expired/missing session by checking for the login page's
  `auth_form` marker in a fetched page, and retries once.
- **Every write is read-modify-write.** There's no JSON PATCH — flock GETs
  the current page, scrapes every input/select's current value
  (`crates/device-http/src/form.rs::scrape_form_fields`), overrides only the
  fields it manages, and POSTs the *entire* field map back as
  `multipart/form-data` (matching the real `<form enctype="multipart/form-data">`
  encoding). This preserves whatever flock doesn't model — e.g. the shared
  template's hidden Encode fields, HDMI OSD timeout, genlock source — instead
  of silently clearing them.
- **`/videoset` genuinely has both Encode and Decode markup**, with the
  Encode forms present in the DOM but `display:none` — confirms the shared
  firmware template covers encode-capable siblings too, and that dropping
  the Encode tab for Play was correct.
- **Real field names are messier than flock's own model** (by design — see
  `crates/core/src/settings.rs`'s doc comment): e.g. `net_method`/`net_address`/
  `net_avahi` for network, `Txpm`/`Rxpm` for transmit/receive preferred
  method (four options: `TCP`/`UDP`/`Multicast`/`RUDP` — no hyphen, unlike
  BirdUI's own prose which calls it "R-UDP"), `dec0_source_name`/
  `dec0_fo_source_name` for decode source/failover (plain uppercase-styled
  text inputs, not a discovered-source picker), `decode_ColorSpace`
  (`YUV`/`RGB`), `decode_NDIAudio` (`NDIAudioEn`/`NDIAudioDis`),
  `decode_ScreenSaverMode` (`CaptureSS`/`BlackSS`/`BirdDogSS`),
  `decode_TallyMode` (`TallyOn`/`TallyOff`/`VideoMode`). `flock-device-http`
  maps between these and flock's own settings shapes.
- **Known limitations of the real client**, all documented in code:
  - `screensaver_mode`/`tally_mode` read back empty on an unconfigured
    device — the real firmware doesn't mark a `selected` option for those
    two dropdowns in server-rendered HTML, so there's nothing to scrape
    until a value has actually been saved once.
  - Access Manager's `remote_ip_list`/`ndi_group_list` are **write-only** —
    real BirdUI only accepts them as an uploaded quoted-CSV text file
    (matching the BirdUI User Guide's own example format) and never renders
    the current list back, so `system_settings()` always reads these as
    empty against real hardware.
  - No password-rotation support — `SystemSettings` has no password field,
    deliberately; getting that wrong risks locking the operator out of their
    own hardware, so it's out of scope until explicitly asked for.
  - There's a live status **WebSocket at `ws://<device-ip>:6790`** pushing
    dashboard stats (CPU%, bandwidth%, device mode, source status) as JSON —
    not wired up yet. `status()` currently polls `/dashboard` and scrapes it
    per call; subscribing to this socket instead would be a nice follow-up
    for cheaper, snappier live status.
  - The real device took 5–15 seconds to respond to some requests in
    testing (embedded hardware, not a bug) — `flock-device-http` uses a
    20-second reqwest timeout accordingly.

## Discovery: why it's a subnet probe, not mDNS

The original plan (browse `_ndi._tcp.local.`) doesn't work: a real Play
**does not advertise itself over mDNS at all** — not under `_ndi._tcp`,
`_http._tcp`, or `_workstation._tcp`. It only carries a plain mDNS hostname
(e.g. `birddog-021d1.local`, an A record with no accompanying service PTR
record), which is why `arp -a`/hostname resolution finds it but
`dns-sd -B`/`ServiceDaemon::browse` never will, confirmed by testing against
real hardware.

`crates/discovery` now runs two mechanisms concurrently (`Discovery::scan`):
mDNS `_ndi._tcp` browsing (kept — harmless, and may surface other NDI gear),
plus `subnet_probe`, which sweeps the operator's local IPv4 subnet(s) (via
`if-addrs`, capped at ~1024 addresses per interface / 512 total) and checks
each live host's `GET /` for BirdUI's signature: a `BirdDogSession` cookie
on the response, visible even unauthenticated, checked from headers alone
without following the redirect or waiting on the slower `/dashboard` render.
Manual add-by-host remains available regardless, so gaps in either
mechanism never block getting a device under management.

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
- `GET /api/discovery/scan` — runs the mDNS browse and subnet probe (see
  Discovery below) and returns hosts not already in the registry.
- `GET/POST /api/devices/:id/{network,decode,system}` — per-tab settings
  read/write, routed through `DeviceClient`.
- `POST /api/groups/:tag/{network,decode,system}` — batch edit: merges a
  partial JSON patch into every group member's current settings for that
  tab (see Batch edit above).

## Docker + networking

mDNS discovery relies on UDP multicast (port 5353), which Docker's default
bridge network does not reliably forward; the subnet probe needs to reach
every host on the operator's LAN directly, which a container's own bridge
subnet isn't part of either. `docker-compose.yml` uses `network_mode: host`
so both mechanisms work out of the box; this trades away container network
isolation. If that tradeoff isn't acceptable for your deployment, switch to
bridge networking with a `ports:` mapping — neither discovery mechanism will
find anything, but manual add-by-host keeps working.
