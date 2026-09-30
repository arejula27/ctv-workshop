#!/usr/bin/env bash
# Plays the vault game with two headless browsers against the running stack.
# Uses the official Playwright image (no system libraries needed on the host).
# Screenshots land in web/e2e/shots/.
set -euo pipefail
cd "$(dirname "$0")/../.."
set -a; . infra/.env; set +a
PW=1.63.0 # keep in sync with web/e2e/package.json
(cd web && npm run -s build >/dev/null)
mkdir -p web/e2e/shots
# Run as the host user so nothing root-owned ends up in the repo.
docker run --rm --network ctv-workshop_default -e ADMIN_TOKEN \
  --user "$(id -u):$(id -g)" -e HOME=/tmp -e npm_config_cache=/tmp/.npm \
  -v "$PWD/web/e2e:/work/e2e" -w /work/e2e "mcr.microsoft.com/playwright:v${PW}-noble" \
  sh -c "npm i --silent --no-audit --no-fund >/dev/null && node vault-game.mjs"
