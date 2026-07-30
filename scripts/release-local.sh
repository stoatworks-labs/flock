#!/usr/bin/env bash
# release-local.sh — cut a full flock release from this Mac.
#
# Ships the flock server for six targets plus the Tauri tray launcher. The
# launcher's Windows bundles are built by driving the Parallels VM: Tauri's
# bundler only runs on its target OS.
#
#   scripts/release-local.sh                  build into dist-release/
#   scripts/release-local.sh --no-vm          skip the Windows launcher bundles
#   scripts/release-local.sh --upload         tag and publish the GitHub release
set -euo pipefail

RR_NAME="flock"
RR_SLUG="flock"
RR_IDENT="com.stoatworks.flock"
RR_EXTRA_FILES=("README.md" "LICENSE")
RR_EXTRA_DIRS=("config" "docs")
RR_LAUNCHER="launcher"
RR_APP_NAME="flock.app"
RR_SERVER_BIN="flock"

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/release-rust.sh"
