#!/usr/bin/env bash
# Build the flock release binary and copy it (plus its config template) into the
# launcher's bundled-resources dir. Run before `npm run tauri build`.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"      # launcher/
REPO="$(cd "$HERE/.." && pwd)"                # repo root

( cd "$REPO" && cargo build --release -p flock )
mkdir -p "$HERE/src-tauri/bin"
cp "$REPO/target/release/flock" "$HERE/src-tauri/bin/flock"
cp "$REPO/config/example.toml"  "$HERE/src-tauri/bin/server-config.toml"
chmod +x "$HERE/src-tauri/bin/flock"
echo "prepared src-tauri/bin/{flock, server-config.toml}"
