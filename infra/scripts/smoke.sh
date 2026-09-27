#!/usr/bin/env bash
# End-to-end check of the stack (F1 acceptance).
#   BASE_URL      default http://localhost:8080
#   ADMIN_TOKEN   required
#   SKIP_ESPLORA  set to 1 to skip the esplora checks (native dev without docker)
set -euo pipefail
BASE_URL=${BASE_URL:-http://localhost:8080}
: "${ADMIN_TOKEN:?set ADMIN_TOKEN}"
AUTH="Authorization: Bearer $ADMIN_TOKEN"
J="Content-Type: application/json"

ok()   { echo "  ✓ $*"; }
fail() { echo "  ✗ $*"; exit 1; }
jget() { python3 -c "import json,sys; print(json.load(sys.stdin)$1)"; }

echo "status"
h0=$(curl -fsS "$BASE_URL/api/status" | jget "['height']") && ok "height $h0" || fail "status"

echo "admin auth"
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "$J" -d '{"blocks":1}' "$BASE_URL/api/admin/mine")
[ "$code" = 401 ] && ok "rejects without token" || fail "expected 401, got $code"

echo "pause miner"
curl -fsS -X PUT -H "$AUTH" -H "$J" -d '{"paused":true}' "$BASE_URL/api/admin/miner" >/dev/null && ok "paused"

echo "faucet"
# Any valid regtest address works; this one is a throwaway P2TR.
ADDR=${ADDR:-bcrt1prjeuqqwwz5dw5xw7zqad8fcc4aexj7hpltfvjhsut9zk6denjqeq6zvmdy}
resp=$(curl -fsS -X POST -H "$J" -d "{\"address\":\"$ADDR\"}" "$BASE_URL/api/faucet") || fail "faucet"
txid=$(echo "$resp" | jget "['txid']"); ok "txid $txid"
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "$J" -d "{\"address\":\"$ADDR\"}" "$BASE_URL/api/faucet")
[ "$code" = 429 ] && ok "cooldown enforced" || fail "expected 429, got $code"
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "$J" -d '{"address":"bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"}' "$BASE_URL/api/faucet")
[ "$code" = 400 ] && ok "rejects mainnet address" || fail "expected 400, got $code"

echo "time jump"
h1=$(curl -fsS "$BASE_URL/api/status" | jget "['height']")
h2=$(curl -fsS -X POST -H "$AUTH" -H "$J" -d '{"blocks":10}' "$BASE_URL/api/admin/jump" | jget "['height']")
[ "$h2" -eq $((h1 + 10)) ] && ok "height $h1 → $h2" || fail "expected $((h1 + 10)), got $h2"

if [ "${SKIP_ESPLORA:-}" != 1 ]; then
  echo "esplora"
  for _ in $(seq 1 20); do
    tip=$(curl -fsS "$BASE_URL/esplora/blocks/tip/height" 2>/dev/null || echo 0)
    [ "$tip" -ge "$h2" ] && break; sleep 1
  done
  [ "$tip" -ge "$h2" ] && ok "indexed tip $tip" || fail "esplora tip $tip < $h2"
  conf=$(curl -fsS "$BASE_URL/esplora/tx/$txid/status" | jget "['confirmed']")
  [ "$conf" = True ] && ok "faucet tx confirmed in esplora" || fail "tx not confirmed in esplora"
fi

echo "resume miner"
curl -fsS -X PUT -H "$AUTH" -H "$J" -d '{"paused":false}' "$BASE_URL/api/admin/miner" >/dev/null && ok "resumed"
echo "all good"
