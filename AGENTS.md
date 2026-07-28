# AGENTS.md — bringing an LLM up to speed on flock

Orientation for an AI assistant (or a new human) picking this project up cold. `CLAUDE.md`
holds the short command reference; this file explains the model and the traps.

---

## 1. What this is

A single web UI for managing **any number of BirdDog Play NDI/SRT decoders** — a fleet
control panel for devices that otherwise only offer their own individual BirdUI web page, one
device at a time.

Rust service: discovery, device adapters behind a common trait, and a web UI.

## 2. Status — unusually well-evidenced, so state it precisely

flock has been exercised end-to-end against **both a simulated BirdDog Play device and a real
one** — including live reads and a **real settings write** (routing an actual NDI source to a
physical unit's HDMI output).

That is a stronger position than most projects in this fleet, and worth stating accurately
rather than hedging. But it is still described by its author as an early-stage hobby project,
and the README's Status section defines exactly which operations are covered and which remain
unverified. **Read that section before claiming any particular operation works on hardware** —
"a real write succeeded" is not the same as "every write path is verified".

## 3. Layout

```
crates/
  core/          Device model and shared traits
    device.rs      The device trait every adapter implements
    registry.rs    Known devices
    settings.rs / app_settings.rs
    client.rs
    crypto.rs      Credential handling
  discovery/     Device discovery
    subnet_probe.rs
  device-http/   HTTP device adapter (the real BirdDog path)
    form.rs        BirdUI form handling
  device-mock/   Mock device - use this for tests without hardware
  web/           Web UI
  flock/         Main binary
config/example.toml
```

## 4. The architectural rule

**New device types implement the `core` device trait. Use `device-mock` for tests that don't
have hardware.**

`device-mock` is not a toy — it's the reason this project can be developed and tested without
a rack of decoders, and it's how the simulated half of the end-to-end verification was done.
Keep it in step with the trait when you extend it, otherwise tests quietly stop covering the
real path.

## 5. Commands

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features
cargo run -p flock
```

## 6. A caution specific to this project

flock writes settings to **real, physical decoders on a live network**. A bad write doesn't
just fail a test — it can black out an output that someone is watching. Treat write paths
with more care than read paths, and prefer `device-mock` until a change is understood.

Credential handling lives in `crates/core/src/crypto.rs`; device passwords are involved. Don't
log them, and don't move them into config files that get committed.

## 7. Conventions

- Ships as its own desktop app via **av-launcher** (Tauri v2 tray shell, server embedded).
  Note the macOS Gatekeeper trap common to all av-launcher apps: for an unsigned `.app`
  bundling helper binaries, approving the app does **not** unquarantine its payload — the
  helpers get SIGKILLed silently.
- Multi-platform release CI; cross-compile macOS x86_64 on `macos-14` — never `macos-13`.
- "Commit" means commit **and** push.
