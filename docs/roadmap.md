# Roadmap

## Phase 1 — mock-first, end-to-end (current)

- [x] Cargo workspace: `core` (domain model + `DeviceClient` trait),
      `discovery` (mdns-sd), `device-mock` (simulated Play), `web`
      (axum REST + WS + static frontend), `flock` (binary)
- [x] Registry with JSON persistence and tag-derived groups (device in
      multiple groups)
- [x] Three-pane web UI: left device list/groups, center preview + tabbed
      settings (Status/Network/Encode/Decode/System, nothing nested), right
      discovery/manual-add/remove/local app settings
- [x] Every settings tab wired end-to-end against `device-mock`
- [x] mDNS discovery scan (`_ndi._tcp.local.`) + manual add fallback
- [x] Docker Compose (host networking) + CI (fmt/clippy/test) +
      docker-publish (ghcr.io)
- [ ] Verified in-browser: grouping, tab round-trips, live WS updates,
      discovery → add → remove, Docker Compose run

## Phase 2 — real hardware

- [ ] Confirm the actual mDNS service type BirdDog Play advertises (or
      whether it only shows up via NDI's own discovery record) and adjust
      `crates/discovery` accordingly
- [ ] `flock-device-http`: a real `DeviceClient` implementation against
      actual BirdUI/REST endpoints (session login, field names) validated
      against physical hardware - the unconfirmed guesses in
      `docs/architecture.md` get corrected here
- [ ] Real video preview (NDI/SRT frame grab) replacing the Phase 1
      placeholder
- [ ] Credential storage hardening (currently plaintext in registry.json -
      fine for a trusted LAN, not for anything more exposed)

## Phase 3 — hardening

- [ ] Auth for flock itself, TLS, multi-user, if ever run beyond a trusted
      LAN
