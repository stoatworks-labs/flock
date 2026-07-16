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
  BirdUI's own prose which calls it "R-UDP"), `decode_ColorSpace`
  (`YUV`/`RGB`), `decode_NDIAudio` (`NDIAudioEn`/`NDIAudioDis`),
  `decode_ScreenSaverMode` (`CaptureSS`/`BlackSS`/`BirdDogSS`),
  `decode_TallyMode` (`TallyOn`/`TallyOff`/`VideoMode`). `flock-device-http`
  maps between these and flock's own settings shapes.
- **Setting the decode source is not just a text field — verified live,
  including a full write.** The visible "Decode Source Name" text input
  (`dec0_source_name`) is itself hidden (`display:none`) in the real UI and
  posting to it alone silently no-ops. The actual mechanism, reverse
  engineered from the page's own JS and confirmed by driving it end-to-end
  through flock against real hardware:
  1. The browser's source-picker dropdown is populated not from BirdUI's
     HTML at all, but from a **separate JSON API the device runs on port
     8080**: `GET http://<device-ip>:8080/List` returns
     `{"source name": "ip:port", ...}` for every NDI source it currently
     sees. `flock-device-http::fetch_source_list` calls this at write time
     to resolve a chosen name to its ip (see "NDI source routing model"
     below for why flock no longer also queries this per-device just to
     populate a picker).
  2. Applying a source requires POSTing `dec0_source_name`, `dec0_source_ip`
     (the resolved `ip:port`), **and** `dec0_change_source_button=dec0_change_source`
     together to `/videoset`. That button field is not cosmetic — omitting
     it (confirmed by testing) causes the server to silently ignore the
     source fields entirely, even though other fields in the same POST
     (colour space, tally, screensaver) do take effect. `set_decode_settings`
     always includes it.
  3. `screensaver_mode`'s true current value isn't marked via the normal
     `selected` attribute on its dropdown - BirdUI's own JS reads it from a
     separate hidden `<option id="dec1_sel" value="...">` marker instead,
     confirmed by watching that marker change from a Go-template nil-render
     artifact to the real value after a save. `scrape_attr_by_id` reads
     this directly rather than looking for `selected`.
- **Known limitations of the real client**, all documented in code:
  - `tally_mode` still reads back empty on a genuinely never-configured
    device (unlike screensaver_mode, no hidden-marker fallback is confirmed
    for it) — cosmetic, not functional.
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
  - The real device took 5–20+ seconds to respond to some requests in
    testing (embedded hardware, not a flock bug) - `/login` specifically has
    been observed alone taking 15-20s while a plain `GET /` on the same
    device answers in milliseconds. `flock-device-http` uses a 30-second
    reqwest timeout and retries `login()` once before giving up, since that
    was the specific hot spot seen to intermittently exceed even the
    original 20s timeout - confirmed fixed live, not just in theory.
  - `set_decode_settings` always re-submits `dec0_change_source_button`,
    even when the operator only changed an unrelated field like tally mode —
    the real UI itself has no way to change colour-space/audio/screensaver/
    tally without that same form, so this mirrors real usage, but means
    every decode save re-applies the source (a harmless no-op if it's
    unchanged, confirmed live, but worth knowing).

## Two unrelated kinds of discovery - keep them separate

`crates/discovery` answers two genuinely different questions, and folding
them together (as an earlier version of this crate did) actively misled the
UI: an NDI *sender* like a laptop's software output would show up in the
"devices to add" list even though it isn't a Play and adding it would just
fail (it doesn't run BirdUI).

- **`Discovery::scan()` — "which hosts are Play units I could add?"** A real
  Play **does not advertise itself over mDNS at all** — not under
  `_ndi._tcp`, `_http._tcp`, or `_workstation._tcp`. It only carries a plain
  mDNS hostname (e.g. `birddog-021d1.local`, an A record with no
  accompanying service PTR record), which is why `arp -a`/hostname
  resolution finds it but `dns-sd -B`/`ServiceDaemon::browse` never will -
  confirmed against real hardware. So this is purely `subnet_probe`: sweep
  the operator's local IPv4 subnet(s) (via `if-addrs`, capped at ~1024
  addresses per interface / 512 total) and check each live host's `GET /`
  for BirdUI's signature (a `BirdDogSession` cookie, visible even
  unauthenticated, checked from headers alone). Manual add-by-host remains
  available regardless, so gaps here never block getting a device under
  management.
- **`Discovery::ndi_sources()` — "which NDI senders exist, and where?"**
  This *is* genuinely mDNS `_ndi._tcp.local.` browsing (the pattern already
  proven in the Dante-BabelBox project) - NDI senders (cameras, software
  like Mitti, a Mac's own NDI output) do advertise themselves that way. Feeds
  `GET /api/ndi/sources`, which the Decode tab's source pickers use as
  autocomplete suggestions.

## NDI source routing model - control plane only, no media relay

Prompted by a good question: how do real NDI routing tools (a BirdDog
Keyboard, Vizrt's NDI Router) switch a receiver's source without the router
ever touching video? Answer, confirmed by how BirdUI's own JS works (see
above): **they never relay media.** NDI senders and receivers connect
peer-to-peer; a "router" only ever does two things over a control channel -
discover senders, then tell *the specific receiver* which one to connect to.
flock is built the same way and deliberately stays that way:

- flock discovers NDI sources itself, centrally, via open mDNS
  (`ndi_sources()` above) - once, instead of every managed Play doing its
  own local discovery and flock having to query each one's `:8080/List`
  separately just to know what's out there.
- Committing a chosen source to a specific Play still goes through that
  Play's own control API (the read-modify-write `/videoset` POST described
  above) - only that device's firmware can tell its own decoder what to
  connect to, exactly like a Keyboard or Vizrt Router would command it.
- flock never receives or re-transmits any video/audio itself. No NDI SDK
  dependency, no media relay engine - `crates/discovery` stays a thin mDNS
  wrapper.

**NDI Discovery Server** is the professional-grade version of centralized
discovery (useful across subnets, or when mDNS is blocked) - real NDI
senders/receivers connect to it over a persistent TCP 5959 socket. Its wire
protocol is proprietary and undocumented outside the actual NDI SDK (checked
- the only public material is "it uses TCP port 5959," not a spec a
from-scratch client could implement). flock can't be a client of it
without that SDK, so it doesn't try. What it *can* do, since every Play's
own Network settings already has this exact field
(`ndi_discovery_server_ips`/`_enabled`), is give one place to configure it
fleet-wide: `AppSettings.discovery_server` (Local App Settings panel,
`GET/PUT /api/settings`) plus `POST /api/settings/push-discovery-server`,
which loops every registered device and pushes that address into its
Network settings via the existing `DeviceClient` - the same mechanism batch
edit uses, just targeting the whole registry instead of one tag group
(since "all devices" isn't a real tag to batch against). flock's own source
list keeps coming from mDNS either way.

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
- `GET /api/discovery/scan` — runs the subnet probe (see Discovery above)
  and returns Play-candidate hosts not already in the registry.
- `GET /api/ndi/sources` — flock's own centralized NDI source list (mDNS),
  what the Decode tab's source pickers suggest from.
- `GET/POST /api/devices/:id/{network,decode,system}` — per-tab settings
  read/write, routed through `DeviceClient`.
- `POST /api/groups/:tag/{network,decode,system}` — batch edit: merges a
  partial JSON patch into every group member's current settings for that
  tab (see Batch edit above).
- `GET/PUT /api/settings` — flock's own app-level settings (currently just
  `discovery_server`). `POST /api/settings/push-discovery-server` pushes it
  to every registered device's Network settings (see NDI source routing
  model above).

## Docker + networking

mDNS discovery relies on UDP multicast (port 5353), which Docker's default
bridge network does not reliably forward; the subnet probe needs to reach
every host on the operator's LAN directly, which a container's own bridge
subnet isn't part of either. `docker-compose.yml` uses `network_mode: host`
so both mechanisms work out of the box; this trades away container network
isolation. If that tradeoff isn't acceptable for your deployment, switch to
bridge networking with a `ports:` mapping — neither discovery mechanism will
find anything, but manual add-by-host keeps working.
