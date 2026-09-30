#!/usr/bin/env bash
# Runs ctv-core's regtest integration tests against the compose stack.
set -euo pipefail
cd "$(dirname "$0")/../.."
set -a; . infra/.env; set +a
export CTV_RPC_URL="http://127.0.0.1:${RPC_DEBUG_PORT:-18543}"
export CTV_RPC_USER="$RPC_USER" CTV_RPC_PASS="$RPC_PASS"
cargo test -p ctv-core --test 'regtest_*' "$@" -- --nocapture
