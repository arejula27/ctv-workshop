#!/usr/bin/env bash
# Wipes the chain, the indexes (electrs, mempool), the room registry, and starts from a fresh regtest.
# Keeps caddy-data (TLS certificates) to avoid Let's Encrypt rate limits.
# The explorer frontend is recreated too: its nginx caches the backend's old IP.
set -euo pipefail
cd "$(dirname "$0")/.."
docker compose stop server explorer mempool-api mempool-db electrs bitcoind
docker compose rm -f server explorer mempool-api mempool-db electrs bitcoind
docker volume rm -f ctv-workshop_bitcoind-data ctv-workshop_electrs-data ctv-workshop_server-data ctv-workshop_mempool-db
docker compose up -d
echo "Chain reset. The server re-seeds the chain on startup."
