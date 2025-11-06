#!/usr/bin/env bash
set -euo pipefail

: "${SHIFT_SESSION_TOKEN:?Set SHIFT_SESSION_TOKEN to a pending session token first}"

BUILD_DIR="${1:-/tmp/tab-session-client-build}"

cmake -S "$(dirname "$0")" -B "${BUILD_DIR}"
cmake --build "${BUILD_DIR}"

echo "Built session client at ${BUILD_DIR}/tab_session_client"
exec "${BUILD_DIR}/tab_session_client"
