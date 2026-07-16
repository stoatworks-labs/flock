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
      login, HTML scraping, read-modify-write settings updates — validated
      by (a) live reads against real hardware and (b) offline unit tests
      against HTML fixtures captured from that hardware
      (`crates/device-http/tests/fixtures/`)
- [x] Core settings shapes (`NetworkSettings`/`DecodeSettings`/
      `SystemSettings`) revised to match confirmed real values (e.g.
      `NdiTransmitMethod` gained `Multicast` and fixed the `RUDP` spelling;
      dropped fields that don't exist on Play — `wifi_enabled`, `ui_mode`;
      added ones that do — `ndi_receive_method`, `color_space`,
      `ndi_audio_enabled`, `tally_mode`)

Deliberately **not** done, and why:
- **Real-device writes weren't exercised live** — only read operations
  (`status`/`network_settings`/`decode_settings`) were verified against the
  physical unit, to avoid risking its live configuration during automated
  development. Write logic is covered by the offline fixture tests plus a
  read-modify-write design that always preserves unknown fields (see
  architecture.md), but a real end-to-end write is still worth doing
  deliberately, once, watching the device.
- **Live WebSocket status** (`ws://<ip>:6790`) isn't wired up —
  `status()` polls and scrapes `/dashboard` per call instead. Works, just
  not as cheap/instant as subscribing to the socket would be.
- **No password rotation** — out of scope until explicitly requested (see
  architecture.md).
- **`screensaver_mode`/`tally_mode` may read back empty** on a real,
  never-configured device (the firmware doesn't mark a `selected` option
  for those dropdowns until a value's been saved once) — cosmetic, not a
  functional gap.

## Phase 3 — hardening

- [ ] Exercise a real write end-to-end against physical hardware (with the
      operator watching) and confirm the read-modify-write POST behaves as
      expected — the one thing Phase 2 deliberately left unverified live
- [ ] Subscribe to the real device's live status WebSocket instead of
      polling `/dashboard`
- [ ] Credential storage hardening (currently plaintext in registry.json -
      fine for a trusted LAN, not for anything more exposed)
- [ ] Real video preview (NDI/SRT frame grab) replacing the placeholder
- [ ] Auth for flock itself, TLS, multi-user, if ever run beyond a trusted
      LAN
