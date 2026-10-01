#!/usr/bin/env bash
# Native Bitcoin Inquisition regtest for running the contract tests without Docker.
#   infra/scripts/native-regtest.sh start   # download (checksum-verified), start, seed
#   infra/scripts/native-regtest.sh test    # run the regtest tests against it
#   infra/scripts/native-regtest.sh stop
set -euo pipefail
cd "$(dirname "$0")/../.."

VERSION=29.4
SHA256=7ad3cb745575f7c21916cf3c294a95f59e11e5bf6e37c4a47b92697cf4723e22 # same as infra/bitcoind/Dockerfile
DIR="${XDG_CACHE_HOME:-$HOME/.cache}/ctv-workshop"
BIN="$DIR/bitcoin-$VERSION-inq/bin"
DATA="$DIR/regtest-data"
PORT=28543 # not 18443/18543: other regtests and the compose stack use those
CLI=("$BIN/bitcoin-cli" -regtest -rpcuser=ctv -rpcpassword=ctv -rpcport=$PORT)

case "${1:-}" in
start)
  if [ ! -x "$BIN/bitcoind" ]; then
    mkdir -p "$DIR"
    tarball="$DIR/inq.tar.gz"
    curl -fsSL -o "$tarball" \
      "https://github.com/bitcoin-inquisition/bitcoin/releases/download/v$VERSION-inq/bitcoin-$VERSION-inq-x86_64-linux-gnu.tar.gz"
    echo "$SHA256  $tarball" | sha256sum -c -
    tar -xzf "$tarball" -C "$DIR" && rm "$tarball"
  fi
  mkdir -p "$DATA"
  "$BIN/bitcoind" -conf="$PWD/infra/bitcoind/bitcoin.conf" -datadir="$DATA" \
    -rpcuser=ctv -rpcpassword=ctv -rpcport=$PORT -port=$((PORT + 1)) -printtoconsole=0 -daemon
  until "${CLI[@]}" getblockcount >/dev/null 2>&1; do sleep 0.5; done
  "${CLI[@]}" loadwallet faucet >/dev/null 2>&1 || "${CLI[@]}" createwallet faucet >/dev/null 2>&1 || true
  height=$("${CLI[@]}" getblockcount)
  [ "$height" -lt 200 ] && "${CLI[@]}" -rpcwallet=faucet -generate $((200 - height)) >/dev/null
  echo "native regtest up at 127.0.0.1:$PORT (height $("${CLI[@]}" getblockcount))"
  ;;
test)
  shift
  CTV_RPC_URL="http://127.0.0.1:$PORT" CTV_RPC_USER=ctv CTV_RPC_PASS=ctv \
    cargo test -p ctv-core --test 'regtest_*' "$@" -- --nocapture
  ;;
stop)
  "${CLI[@]}" stop
  ;;
*)
  echo "usage: $0 start|test|stop" >&2
  exit 1
  ;;
esac
