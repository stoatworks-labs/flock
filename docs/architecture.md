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

## Credentials are encrypted at rest, transparently

Every `Device.credentials.password` is plaintext *in memory* — that's what
`DeviceClient` implementations and the API layer's `Device::redacted()` (for
responses) both expect, and changing that would ripple everywhere. What
changed is what `Registry` writes to disk: `crates/core/src/crypto.rs`
encrypts each password with AES-256-GCM right at the `save()`/`load_or_new()`
boundary, so `registry.json` itself never holds a plaintext password,
whether that file ends up in a backup, a synced folder, or a support
bundle.

- The key is generated on first run and stored in `credentials.key`, a
  sibling of `registry.json` (so `docker-compose`'s existing registry
  volume mount covers it with no config changes) - chmod `600` on unix.
  Losing this file makes existing stored passwords undecryptable; the
  registry itself still loads fine; the operator just has to re-enter
  passwords.
- Each encrypted value is stored as `flock-enc-v1:<nonce-hex>:<ciphertext-hex>`.
  That explicit prefix is what makes a **legacy plaintext `registry.json`
  written before this existed** load correctly with zero migration step:
  `decrypt_or_pass_through` just returns anything without the prefix
  unchanged, and the very next save encrypts it going forward.
- This only covers **credentials at rest** — the existing API-layer
  redaction (`Device::redacted()`, still showing `"********"` to the
  frontend) and flock's own auth (below) are unrelated, separate concerns.

## flock's own auth is optional, off by default

Every concern above is about a *device's* BirdUI password. This is the
separate question of who can reach *flock itself* — historically nobody-gated
at all, matching BirdUI's own trusted-LAN model (whoever's on the LAN has
full control). `admin_password` in `config/flock.toml` turns on a single
shared login for flock's own web UI/API, for anyone who wants more than bare
network trust without standing up real multi-user auth (out of scope for a
single-operator LAN tool — see Phase 3 in docs/roadmap.md).

- **Off by default** (`Config::admin_password: Option<String>` is `None`) —
  every route behaves exactly as before. Setting it is the only opt-in step.
- **One shared password, one session cookie.** `crates/web/src/auth.rs`:
  `POST /api/login` compares the given password against `admin_password`
  (constant-time, hand-rolled — cheap enough to not need a dedicated crate)
  and, on success, hands out a `flock_session` cookie (`HttpOnly`,
  `SameSite=Strict`) backed by a random token in an in-memory `HashSet`.
  `POST /api/logout` clears both. There's no per-user account model, no
  password reset flow, no persistence for sessions — a process restart logs
  everyone out, which is an acceptable tradeoff for what this is.
- **Middleware, not per-handler checks.** `crates/web/src/lib.rs::app()`
  splits routes into two sub-routers: `public` (static frontend, `/health`,
  `/api/login`, `/api/logout`) and `protected` (everything else — every
  `/api/*` device/registry/settings route, and `/ws`), with
  `axum::middleware::from_fn_with_state(state, auth::require_auth)` applied
  only to `protected` via `route_layer` (which, unlike a blanket `.layer()`,
  only covers routes defined directly on that router — not ones merged in
  afterward). `require_auth` is a no-op pass-through whenever
  `admin_password` is `None`.
- **Frontend gate is a single 401 check, not a separate "is auth enabled"
  endpoint.** `app.js`'s `init()` does a bare `GET /api/state`: a 401 means a
  session is required (show the login screen), anything else means there's
  no gate at all (start the app immediately) — the same request that would
  happen anyway doubles as the auth probe.
- **Login is rate-limited, process-wide, not per-client.** `LoginGuard`
  (in `auth.rs`, held on `AppState` like everything else) locks out further
  `POST /api/login` attempts for 30s after 5 failures — even a *correct*
  password is rejected with `429` while locked, confirmed live. It's
  process-wide rather than per-IP deliberately: there's only one password to
  guess in the first place, so there's no useful notion of "which caller" to
  scope a limit to. A success resets the counter.
- **`Config`'s hand-rolled `Debug` impl redacts `admin_password`** — it's
  logged at startup (`tracing::info!(?config, ...)`) alongside every other
  setting, and a derived `Debug` would have put the plaintext password in
  the log.

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

## SRT decode support - confirmed live, apply mechanism partially unreliable

The firmware `1.0.18` unit this project was originally verified against had
**no SRT UI at all** in `/videoset`. A firmware update applied mid-
development added a second "Source Selection" mode (NDI vs SRT vs a third,
unmodeled "CloudConnect") with its own connection-type/stream-name/IP/port/
latency/encryption/passphrase/stream-ID fields. Once the device was reachable
again, every field name below was confirmed against real `/videoset` HTML
(`crates/device-http/tests/fixtures/videoset_srt.html`) - and live testing
turned up two real, non-obvious problems along the way.

**Confirmed field names** (`decode_settings()`/`set_decode_settings()` in
`crates/device-http/src/lib.rs`): `decode_SourceSelection` ("NDI"/
"CloudConnect"/"SRT" - flock only models the first and third),
`conntypemode` ("caller"/"listener"/"rendezvous"), `StreamName`,
`IPAddress`, `Port`, `latency`, `Encryption` ("true"/"false"), `pbkeylen`
(numeric key size as a string: `"0"`=None/unset, `"16"`=AES-128,
`"24"`=AES-192, `"32"`=AES-256), `passphrase`, `streamid`.

**Problem 1 - `StreamName` collides with a hidden, dead field.** `/videoset`'s
Encode tab (`display:none` on Play - confirmed dead/unused, per "Confirmed
against real hardware" above) reuses the exact same field name for its own
NDI transmit stream name. `scrape_form_fields` flattens the whole page into
one map with no per-form scoping, so this collision is invisible to a read
(decode's own value happens to win, by document-order luck) but **not** to a
write: confirmed live, posting a value under `StreamName` updates the dead
Encode field instead of the SRT decode field, which never changes. Fix:
`set_decode_settings` no longer writes `StreamName` (or any of the other SRT
manual-entry fields - see Problem 2, they don't work either) to `/videoset`
at all.

**Problem 2 - the SRT panel's manual-entry fields aren't a working write
path in the first place.** Reverse engineering BirdUI's own inline JS
(`/videoset`'s `postsrtJsonFile()`/`Add_Dest_Data()`) showed that "Stream
Name"/"IP Address"/"Port"/etc. are pure client-side staging for building a
"Connection URL" preview string - the *actual* apply is a separate call:
`POST {StreamName, StreamIP}` (form-encoded; a variable named `outputData`
sits right next to the real `$.ajax` call looking like a JSON body, but is
dead code, never passed to the request) to `http://<device-ip>:8080/
writesrtsource` - a JSON API on the same port `fetch_source_list` already
uses for NDI's `/List`. `HttpDeviceClient::apply_srt_source` implements this,
including `build_srt_connection_url` which mirrors BirdUI's own URL-building
branches exactly (`listener` mode uses the device's own IP, not the typed-in
one; only `caller` mode includes `streamid`; `pbkeylen`/`passphrase` are
only appended when encryption is on).

**This apply call is unconfirmed to actually work, and is deliberately
non-fatal.** Live testing: `:8080/writesrtsource` accepted the TCP
connection and then never responded at all, timing out; `:8080/writesrt`
(meant to list already-applied SRT sources, for a future `srt_available_
sources`) 404s on the same device. Whether this is a firmware bug, a missing
precondition this codebase hasn't identified, or something else entirely is
unknown. Given that, `apply_srt_source` is wrapped in a 5-second
`tokio::time::timeout` and never propagates an error - a failure here only
logs a `tracing::warn!` and must never block or fail the rest of a
decode-settings save (confirmed live: `decode_SourceSelection`, screensaver/
colour-space/audio/tally all still save correctly and quickly even when the
SRT apply call times out).

**Net effect on flock's own SRT support right now**: switching a device
between NDI/SRT mode works and is confirmed. Manually typing in SRT
connection details and having them actually take effect on the device is
**not yet working** - the mechanism is understood and implemented, but the
device itself isn't cooperating in testing. `srt_available_sources` remains
unimplemented (the `:8080/writesrt` 404 blocks it too, for now).

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

## Left panel: nested group tree

Groups are a vertical tree, not a horizontal chip list - each group header
has an expand arrow revealing its members (a device in multiple groups
appears nested under each one, since groups are derived from tags, not
exclusive membership; see `Registry::groups()` above). Clicking a group's
header directly enters batch-edit for that group in one step; clicking the
arrow only toggles expansion without changing selection; clicking a nested
device selects it for the normal single-device view. "All devices" is a
synthetic pseudo-group (not a real tag) at the top - its header just clears
selection/batch-mode instead of offering to batch-edit it, since there's no
tag to batch against. All of this lives in one function,
`renderGroupTree()` in `crates/web/static/app.js`, replacing an earlier
two-step design (a horizontal chip list to filter, plus a separate "Batch
edit" button) that took an extra click and didn't show group membership at
a glance.

## Batch edit

Clicking a group's header (see above) swaps the center panel into a
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
- `GET /health` — plain healthcheck, used by `docker-compose.yml`.
- `POST /api/login`, `POST /api/logout` — flock's own optional auth (see
  "flock's own auth" above); public regardless of whether `admin_password`
  is set, since a login route can't itself require being logged in.
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
