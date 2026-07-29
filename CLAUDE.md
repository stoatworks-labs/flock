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

## Diagnostics

Log via `tracing` as usual; `crates/diag` adds a rotating file, an in-memory ring and a
panic hook that writes a JSON crash report. Wire it as the **first** thing in `main`, and
**hold the returned guard** — dropping it (`let _ = diag::init(..)`) silently stops the log
file being written. Console output goes to stderr; stdout is reserved for program output.
See [docs/diagnostics.md](docs/diagnostics.md).
