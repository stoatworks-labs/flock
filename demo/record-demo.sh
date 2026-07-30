#!/usr/bin/env bash
#
# Rebuild flock's hosted demo end to end.
#
# Runs flock against its own simulated Play devices (config/demo.toml, which
# sets provider = "mock"), records what that backend actually returns, and
# assembles the demo site from flock's real, unmodified UI plus the recording.
#
# The point of recording rather than hand-writing fixtures: the demo then shows
# what flock does, not what someone remembers it doing. Re-run this whenever the
# API or the UI changes, then publish with:
#
#   demo/deploy-pages.sh --dist demo/dist --label "flock demo"
set -euo pipefail

cd "$(dirname "$0")/.."

PORT=8099
BASE="http://127.0.0.1:$PORT"

echo "==> Building flock"
cargo build -q -p flock

echo "==> Starting flock with simulated devices"
rm -f data/registry-demo.json data/app_settings-demo.json
cargo run -q -p flock -- config/demo.toml >/tmp/flock-demo-record.log 2>&1 &
FLOCK_PID=$!
cleanup() { kill "$FLOCK_PID" 2>/dev/null || true; }
trap cleanup EXIT

for _ in $(seq 1 60); do
  curl -sf "$BASE/health" >/dev/null 2>&1 && break
  sleep 1
done
curl -sf "$BASE/health" >/dev/null || { echo "error: flock did not start; see /tmp/flock-demo-record.log" >&2; exit 1; }

echo "==> Recording"
node demo/record-fixtures.mjs \
  --base "$BASE" \
  --app "flock" --repo "https://github.com/stoatworks-labs/flock" \
  --get /api/state \
  --get /api/discovery/scan \
  --get /api/ndi/sources \
  --get /api/settings \
  --expand '/api/state:devices[].id:/api/devices/{}/status' \
  --expand '/api/state:devices[].id:/api/devices/{}/network' \
  --expand '/api/state:devices[].id:/api/devices/{}/decode' \
  --expand '/api/state:devices[].id:/api/devices/{}/system' \
  --expand '/api/state:devices[].id:/api/devices/{}/preview' \
  --ws /ws --ws-seconds 12 \
  --out demo/demo-fixtures.json

echo "==> Assembling the site"
demo/build-demo.sh \
  --src crates/web/static \
  --fixtures demo/demo-fixtures.json \
  --out demo/dist

echo
echo "Preview it exactly as Pages will serve it:"
echo "  demo/serve-demo.py --dir demo/dist --base /flock/"
