# Developing flock

Build, test and extension guide. For the mental model read [`AGENTS.md`](../AGENTS.md); for
the HTTP surface [`API.md`](API.md).

---

## Build and run

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features
cargo run -p flock
```

Configuration: `config/example.toml`.

---

## Layout

```
crates/
  core/          Device model and shared traits
    device.rs      The trait every adapter implements
    registry.rs    Known devices (persisted as registry.json)
    settings.rs / app_settings.rs
    client.rs
    crypto.rs      Credential encryption at rest
  discovery/     Device discovery
    subnet_probe.rs
  device-http/   The real BirdDog adapter
    form.rs        BirdUI form handling
  device-mock/   Mock device - use this
  web/           HTTP API + UI
    auth.rs        Optional session gate
  flock/         Main binary
```

---

## Develop against `device-mock`

**This is the rule that matters.** flock writes settings to real decoders; a bad write during
development can black out a screen someone is watching, and real devices aren't always to
hand.

`device-mock` isn't a toy — it's how the simulated half of the end-to-end verification was
done. **Keep it in step with the `core` device trait when you extend that trait**, or tests
quietly stop covering the real path while still passing.

---

## Adding a device type

Implement `core`'s device trait. Everything above the trait — the API, the UI, group
operations — is device-agnostic and should stay that way.

`device-http` is the reference implementation. Note it deals with **BirdUI's HTML forms**
(`form.rs`) rather than a documented API, so it's inherently sensitive to firmware changes.

---

## Security — the parts you must not weaken

### Credentials on disk (`core/src/crypto.rs`)
`registry.json` used to hold every device's BirdUI password in plain text. It doesn't now.

- The key is generated on first run and lives in its own file beside the registry.
- **On Unix it is chmod 600.** It's the only thing between `registry.json` and every device's
  plaintext password. Don't relax it; don't commit it.
- Encrypted values carry a **prefix** so decryption can tell them from a plaintext password
  written before this module existed — no real BirdUI password will legitimately start with
  it, which makes migration unambiguous. Keep that prefix.
- The key never leaves the process.

### Redaction (`Device::redacted`)
Separate concern from encryption: redaction governs what the **API returns**, encryption
governs what sits on **disk**. A new endpoint that returns a device must return the redacted
form. Don't log passwords either.

### The login gate (`web/src/auth.rs`)
Off by default (`admin_password` is `None`), matching the historical trusted-LAN model. When
configured, everything except the static frontend, `/health` and login/logout requires a
session.

Deliberate trade-offs, documented so they aren't "fixed" into something worse:

- **Sessions are an in-memory set with no persistence and no expiry.** A restart logs everyone
  out — acceptable, and arguably desirable, for a single-operator LAN tool. Adding persistence
  means storing session tokens somewhere, which is a bigger security surface than the problem.
- **The login guard is process-wide, not per-client** (5 failures, 30s lockout). There is only
  one password, so "which caller" carries no useful meaning. Making it per-IP would let an
  attacker rotate source addresses and defeat it entirely.

---

## Testing

```bash
cargo test
```

Use `device-mock` for anything that would otherwise need hardware.

**What's proven:** end-to-end against both a simulated device and a real BirdDog Play,
including a real settings write that routed an actual NDI source to a physical unit's HDMI
output.

**What isn't:** every other write path, and long-run fleet behaviour. Don't let the real-write
result be generalised into "verified against hardware" without qualification.

---

## Releasing

Ships as its own desktop app via **av-launcher** (Tauri v2 tray shell, server embedded).

**macOS Gatekeeper trap:** for an unsigned `.app` bundling helper binaries, approving the app
does **not** unquarantine the payload — helpers are SIGKILLed silently. It presents as "the
app opens but the server never starts".

Multi-platform release CI; cross-compile macOS x86_64 on **`macos-14`**, never `macos-13`.

"Commit" means commit **and** push.
