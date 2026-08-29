#!/usr/bin/env bash
# Runs the API and the web app together and shuts both down on Ctrl-C.
# PostgreSQL is expected to be running already (`make db-up`).
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ -f .env ]]; then
  # shellcheck disable=SC1091
  set -a && source .env && set +a
fi

: "${HOMECLOUD_DATABASE_URL:?set HOMECLOUD_DATABASE_URL, for example by running: make setup}"

pids=()
cleanup() {
  trap - INT TERM EXIT
  for pid in "${pids[@]}"; do
    kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
}
trap cleanup INT TERM EXIT

echo "starting api on ${HOMECLOUD_LISTEN_ADDR:-127.0.0.1:8080}"
cargo run --bin homecloud-api &
pids+=("$!")

echo "starting web on http://127.0.0.1:3000"
pnpm --filter @homecloud/web dev &
pids+=("$!")

wait -n
