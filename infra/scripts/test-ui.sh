#!/usr/bin/env bash
# Plays the demos with headless browsers against the running stack.
# Runs every web/e2e/*-game.mjs, or only TESTS="contracts-game.mjs".
# Uses the official Playwright image (no system libraries needed on the host).
# Screenshots land in web/e2e/shots/.
set -euo pipefail
cd "$(dirname "$0")/../.."
set -a; . infra/.env; set +a
PW=1.63.0 # keep in sync with web/e2e/package.json
(cd web && npm run -s build >/dev/null)
mkdir -p web/e2e/shots
# Run as the host user so nothing root-owned ends up in the repo.
# Host network: the browser sees localhost:8080 and the explorer on :5000, like a participant.
docker run --rm --network host -e ADMIN_TOKEN -e BASE_URL="http://localhost:${HTTP_PORT:-8080}" \
  --user "$(id -u):$(id -g)" -e HOME=/tmp -e npm_config_cache=/tmp/.npm \
  -v "$PWD/web/e2e:/work/e2e" -w /work/e2e "mcr.microsoft.com/playwright:v${PW}-noble" \
  sh -c "npm i --silent --no-audit --no-fund >/dev/null && for t in ${TESTS:-*-game.mjs}; do echo \"== \$t\"; node \"\$t\" || exit 1; done"
