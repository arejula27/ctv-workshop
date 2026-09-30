#!/usr/bin/env bash
# Wipes the chain, the index and the room registry, and starts from a fresh regtest.
# Keeps caddy-data (TLS certificates) to avoid Let's Encrypt rate limits.
set -euo pipefail
cd "$(dirname "$0")/.."
docker compose stop server electrs bitcoind
docker compose rm -f server electrs bitcoind
docker volume rm -f ctv-workshop_bitcoind-data ctv-workshop_electrs-data ctv-workshop_server-data
docker compose up -d
echo "Chain reset. The server re-seeds the chain on startup."
