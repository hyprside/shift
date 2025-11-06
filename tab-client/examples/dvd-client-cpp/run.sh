#!/usr/bin/env bash
set -euo pipefail


BUILD_DIR="${1:-/tmp/tab-dvd-client-build}"

cmake -S "$(dirname "$0")" -B "${BUILD_DIR}"
cmake --build "${BUILD_DIR}"

echo "Built dvd client at ${BUILD_DIR}/tab_dvd_client"
: "${SHIFT_SESSION_TOKEN:?Set SHIFT_SESSION_TOKEN to a pending session token first}"
exec "${BUILD_DIR}/tab_dvd_client"
