# flock — desktop app

A small menu-bar desktop app for flock: pick a network interface + port,
Start/Stop the server, open the web UI, and run it from the macOS menu bar.
Built with [Tauri v2](https://tauri.app). The `flock` server is **bundled inside
the app** — it's a single download, nothing to install or wire up.

![panel](docs/panel.png)

Download the `.dmg` from
[Releases](https://github.com/allansargeant/flock/releases).

> **Unsigned build.** On first launch macOS Gatekeeper will block it —
> right-click the app → **Open** → **Open**, once.

## What it does

- **Network interface** — every bindable IPv4 interface, plus "All interfaces (0.0.0.0)".
- **Port** — persisted between runs.
- **Start / Stop** — supervises the bundled `flock` server process.
- **Open** — opens `http://<host>:<port>/` (the fleet web UI) in your browser.
- **Hide** to the menu bar; **Quit** stops the server and exits.

The panel is themed to match flock's own web UI. Host:port is injected by
patching flock's config (top-level `bind`, passed as flock's positional config
arg); the server runs from a writable app-data dir where it persists `data/`.

## Prefer the terminal / Docker?

The same server is a plain binary — `cargo run -p flock` / `docker compose up`
(see the repo root README), and the release also attaches the standalone
`flock` binary.

## Building from source

The desktop build bundles the release `flock` binary (git-ignored — it ships in
the Release), so fetch it first:

```bash
cd launcher
./scripts/prepare.sh          # builds flock --release, copies it into src-tauri/bin/
npm install
npm run tauri build           # -> src-tauri/target/release/bundle/{macos,dmg}/
```

The panel/tray shell is a copy of the reusable
[av-launcher](https://github.com/allansargeant/av-launcher); only
`src-tauri/launcher.toml` (config + theme), the icon, and the bundled binary
are app-specific.
