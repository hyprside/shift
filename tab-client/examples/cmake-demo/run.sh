#!/usr/bin/env bash
set -euo pipefail


BUILD_DIR="${1:-/tmp/tab-client-demo-build}"

cmake -S "$(dirname "$0")" -B "${BUILD_DIR}"
cmake --build "${BUILD_DIR}"

echo "Built demo at ${BUILD_DIR}/tab_client_demo"
: "${SHIFT_SESSION_TOKEN:?Set SHIFT_SESSION_TOKEN before running the demo}"
exec "${BUILD_DIR}/tab_client_demo"
