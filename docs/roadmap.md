# Roadmap

## Phase 1 — mock-first, end-to-end (done)

- [x] Cargo workspace: `core` (domain model + `DeviceClient` trait),
      `discovery`, `device-mock` (simulated Play), `device-http` (real
      client), `web` (axum REST + WS + static frontend), `flock` (binary)
- [x] Registry with JSON persistence and tag-derived groups (device in
      multiple groups)
- [x] Three-pane web UI: left device list/groups, center preview + tabbed
      settings (Status/Network/Decode/System — Play is decode-only, no
      Encode tab — nothing nested), right discovery/manual-add/remove/local
      app settings
- [x] Every settings tab wired end-to-end against `device-mock`
- [x] Batch edit: select a group, apply a Network/Decode/System patch to
      every member at once, blank fields left unchanged per-device
- [x] Docker Compose (host networking) + CI (fmt/clippy/test) +
      docker-publish (ghcr.io)
- [x] Verified in-browser: grouping, tab round-trips, live WS updates,
      discovery → add → remove, Docker Compose run

## Phase 2 — real hardware (done)

- [x] Logged into an actual BirdDog PLAY unit (firmware 1.0.18) and
      confirmed real routes/field names/login flow — see
      docs/architecture.md's "Confirmed against real hardware" section
- [x] Discovered the Play doesn't advertise any mDNS service at all
      (not `_ndi._tcp`, `_http._tcp`, or `_workstation._tcp`) — replaced
      mDNS-only discovery with a subnet probe (`crates/discovery/src/subnet_probe.rs`)
      that checks each LAN host's `GET /` for BirdUI's session-cookie
      signature; mDNS browsing is kept alongside it for other NDI gear
- [x] `flock-device-http`: real `DeviceClient` implementation — session
      login, HTML scraping, read-modify-write settings updates
- [x] Core settings shapes (`NetworkSettings`/`DecodeSettings`/
      `SystemSettings`) revised to match confirmed real values (e.g.
      `NdiTransmitMethod` gained `Multicast` and fixed the `RUDP` spelling;
      dropped fields that don't exist on Play — `wifi_enabled`, `ui_mode`;
      added ones that do — `ndi_receive_method`, `color_space`,
      `ndi_audio_enabled`, `tally_mode`)
- [x] **Exercised a real write end-to-end against physical hardware**,
      routing an actual NDI source (Mitti) to the Play's HDMI output through
      flock's own API. First attempt silently no-op'd — see below — second,
      corrected attempt confirmed working live, including reading the
      result back and switching sources twice more.
- [x] Found and fixed the real decode-source mechanism, which is
      substantially different from the first guess: the source list comes
      from a separate JSON API on **port 8080** (`GET /List`), and applying
      a source requires a specific `dec0_change_source_button` field in the
      POST — silently ignored without it, discovered by directly testing
      against the real unit. See docs/architecture.md for the full
      writeup and `crates/device-http/tests/fixtures/videoset_after_apply.html`
      for the fixture proving it.
- [x] **Centralized NDI source discovery at flock, matching how real NDI
      routers (BirdDog Keyboard, Vizrt Router) work** — pure control plane,
      no media relay. Split `crates/discovery` into device discovery
      (subnet probe) and NDI source discovery (mDNS, properly resolving
      name+ip:port now); added `GET /api/ndi/sources` and wired it into the
      Decode tab's source pickers as autocomplete, replacing the old
      per-device `:8080/List` query for display purposes (still used at
      write time to resolve a chosen name). Also added an `AppSettings`
      store (`discovery_server`) with a Local App Settings field and a
      "push to all devices" action that writes it into every registered
      Play's own Network settings — honestly scoped to that, since flock
      can't itself speak the NDI Discovery Server's proprietary protocol
      (checked: no public spec exists outside the NDI SDK).

- [x] **Automatic retry + longer timeout for the real device's observed
      cold-start behavior.** Hit live while dogfooding a running instance:
      `/login` alone was seen taking 15-20s, occasionally past the 20s
      client timeout, even though plain `GET /` on the same device answered
      in milliseconds. Bumped the client timeout to 30s and added a
      one-retry loop specifically around `login()` (the observed hot spot)
      rather than blindly retrying every request. Confirmed fixed live:
      the same device that was failing repeatedly loaded status
      successfully afterward.

Deliberately **not** done, and why:
- **Live WebSocket status** (`ws://<ip>:6790`) isn't wired up —
  `status()` polls and scrapes `/dashboard` per call instead. Works, just
  not as cheap/instant as subscribing to the socket would be.
- **No password rotation** — out of scope until explicitly requested (see
  architecture.md).
- **`tally_mode` may read back empty** on a real, never-configured device
  (no hidden-marker fallback confirmed for it, unlike `screensaver_mode`)
  — cosmetic, not a functional gap.

## Phase 3 — hardening

- [ ] Subscribe to the real device's live status WebSocket instead of
      polling `/dashboard`
- [ ] Credential storage hardening (currently plaintext in registry.json -
      fine for a trusted LAN, not for anything more exposed)
- [ ] Real video preview (NDI/SRT frame grab) replacing the placeholder
- [ ] Auth for flock itself, TLS, multi-user, if ever run beyond a trusted
      LAN
