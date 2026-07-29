# flock

Rust fleet-control service (core + discovery + device adapters + web UI). Discovers devices and drives them through HTTP adapters behind a common device trait.

## Commands
- Build: `cargo build` (workspace)
- Test: `cargo test`
- Lint: `cargo clippy --all-targets --all-features`
- Run: `cargo run -p flock`

## Layout (crates/)
- `core` — device model & shared traits
- `discovery` — device discovery
- `device-http` — HTTP device adapter
- `device-mock` — mock device for testing
- `web` — web UI
- `flock` — main binary

## Notes
- New device types implement the `core` device trait; use `device-mock` for tests without hardware.
- Multi-platform release CI; cross-compile macOS x86_64 on macos-14 (never macos-13).
- "Commit" = commit **and** push.
