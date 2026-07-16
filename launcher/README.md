# flock Launcher

A Bitfocus Companion–style **tray launcher** for flock: pick a network
interface + port, Start/Stop the server, open the web UI, and run it from the
macOS menu bar. Built with [Tauri v2](https://tauri.app); the shipped `.app`
**bundles the `flock` binary**, so it runs standalone.

![panel](docs/panel.png)

Download the latest `.dmg` from the repo's
[Releases](https://github.com/allansargeant/flock/releases).

> **Unsigned build.** On first launch macOS Gatekeeper will block it —
> right-click the app → **Open** → **Open**, once.

## What it does

- **GUI Interface** — every bindable IPv4 interface, plus "All interfaces (0.0.0.0)".
- **Port** — persisted between runs.
- **Start / Stop** — supervises the bundled `flock` child process.
- **Launch GUI** — opens `http://<host>:<port>/` in your browser.
- **Hide** to the tray; **Quit** stops the server and exits.

Host:port is injected by patching flock's own config (the top-level `bind` key)
and passing the rendered file as flock's positional config argument. The launcher
runs the server from its writable app-config dir, where flock persists its
`data/` (registry, app settings).

## Building from source

The launcher bundles the release `flock` binary. It's git-ignored (it ships in
the Release), so fetch it first:

```bash
cd launcher
./scripts/prepare.sh          # builds flock --release and copies it into src-tauri/bin/
npm install
npm run tauri build           # -> src-tauri/target/release/bundle/{macos,dmg}/
```

Run in dev:

```bash
npm run tauri dev
```

## How it relates to av-launcher

This is a self-contained copy of the reusable
[av-launcher](https://github.com/allansargeant/av-launcher) shell with flock's
config baked in. The Rust/JS shell is identical across the fleet; only
`src-tauri/launcher.toml`, the icon, and the bundled binary differ.
