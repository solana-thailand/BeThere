#!/usr/bin/env bash
# retry_deploy.sh — Retry wrangler deploy until Cloudflare API recovers (10013).
#
# Cloudflare's Workers versions API is returning error 10013 (degraded performance).
# DO bindings + migrations can ONLY be deployed via `wrangler deploy` (versions API).
# This script retries until it succeeds.
#
# Usage:
#   ./retry_deploy.sh                          # Retry every 5 min, unlimited
#   ./retry_deploy.sh --interval 60            # Retry every 60s
#   ./retry_deploy.sh --max-attempts 20        # Give up after 20 tries
#   ./retry_deploy.sh --uncomment-do           # Uncomment DO binding before deploy
#   ./retry_deploy.sh --notify                 # macOS notification on success/failure
#   ./retry_deploy.sh -h                       # Show help

set -euo pipefail
cd "$(dirname "$0")"

# ── Defaults ──
INTERVAL=300       # seconds between retries (5 min)
MAX_ATTEMPTS=0     # 0 = unlimited
UNCOMMENT_DO=false
NOTIFY=false
WORKER_URL="https://bethere.solana-thailand.workers.dev"

# ── Parse flags ──
show_help() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Retry wrangler deploy until Cloudflare API error 10013 clears.

Options:
  --interval SECONDS   Wait between retries (default: 300 = 5 min)
  --max-attempts N     Max retry attempts (default: 0 = unlimited)
  --uncomment-do       Uncomment DO binding + migration in wrangler.toml
  --notify             Show macOS notification on success/failure
  -h, --help           Show this help message

Examples:
  ./retry_deploy.sh                          # Default: retry every 5 min forever
  ./retry_deploy.sh --interval 60 --notify   # Retry every minute, notify on done
  ./retry_deploy.sh --uncomment-do --max-attempts 50
EOF
  exit 0
}

while [ $# -gt 0 ]; do
  case "$1" in
    --interval)
      INTERVAL="$2"
      shift 2
      ;;
    --max-attempts)
      MAX_ATTEMPTS="$2"
      shift 2
      ;;
    --uncomment-do)
      UNCOMMENT_DO=true
      shift
      ;;
    --notify)
      NOTIFY=true
      shift
      ;;
    -h|--help)
      show_help
      ;;
    *)
      echo "❌ Unknown flag: $1"
      echo "   Run '$(basename "$0") --help' for usage."
      exit 1
      ;;
  esac
done

# ── Yarn PnP conflict handling ──
PNP_FILE="$HOME/.pnp.cjs"
PNP_BACKUP="$HOME/.pnp.cjs.bak"
MOVED=false

move_pnp() {
  if [ -f "$PNP_FILE" ] && [ ! -f "$PNP_BACKUP" ]; then
    echo "📦 Temporarily moving ~/.pnp.cjs (Yarn PnP conflict)..."
    mv "$PNP_FILE" "$PNP_BACKUP"
    MOVED=true
  fi
}

restore_pnp() {
  if [ "$MOVED" = true ] && [ -f "$PNP_BACKUP" ]; then
    echo "↩  Restoring ~/.pnp.cjs..."
    mv "$PNP_BACKUP" "$PNP_FILE"
  fi
}

trap restore_pnp EXIT INT TERM

# ── macOS notification helper ──
notify() {
  local title="$1"
  local message="$2"
  if [ "$NOTIFY" = true ]; then
    osascript -e "display notification \"$message\" with title \"$title\"" 2>/dev/null || true
  fi
}

# ── Uncomment DO binding + migration in wrangler.toml ──
uncomment_do_bindings() {
  echo "📝 Uncommenting DO binding + migration in wrangler.toml..."

  # Uncomment the DO binding and migration lines (lines starting with "# [[durable_objects...")
  # and the subsequent "# name", "# class_name", "# tag", "# new_sqlite_classes" lines
  sed -i '' \
    -e 's/^# \[\[durable_objects\.bindings\]\]/[[durable_objects.bindings]]/' \
    -e 's/^# name = "EVENT_DO"/name = "EVENT_DO"/' \
    -e 's/^# class_name = "EventDurableObject"/class_name = "EventDurableObject"/' \
    -e 's/^# \[\[migrations\]\]/[[migrations]]/' \
    -e 's/^# tag = "v1"/tag = "v1"/' \
    -e 's|^# new_sqlite_classes = \["EventDurableObject"\]|new_sqlite_classes = ["EventDurableObject"]|' \
    wrangler.toml

  echo "📝 Adding EventDurableObject export to shim template..."

  # Add the export line after "export default imports;" in the shim heredoc
  sed -i '' '/^export default imports;$/a\
export { EventDurableObject } from "./event_checkin_worker_bg.js";
' wrangler.toml

  echo "✅ DO binding + shim export uncommented"
}

# ── Verify production endpoints ──
verify_production() {
  echo "🔍 Verifying production endpoints..."

  # Check /api/health — expect d1.connected=true
  local health_response
  health_response=$(curl -s "${WORKER_URL}/api/health" 2>/dev/null || echo "")

  if [ -z "$health_response" ]; then
    echo "   ⚠️  /api/health returned empty response"
    return 1
  fi

  # Check for d1.connected=true
  if echo "$health_response" | grep -q '"connected":true\|"connected": true'; then
    echo "   ✅ /api/health — D1 connected"
  else
    echo "   ⚠️  /api/health — D1 not connected or unexpected response:"
    echo "   $(echo "$health_response" | head -c 200)"
  fi

  # Verify EVENT_DO binding appears in the deploy output (stored in DEPLOY_OUTPUT)
  if echo "$DEPLOY_OUTPUT" | grep -q "EVENT_DO"; then
    echo "   ✅ EVENT_DO binding present in deploy output"
  else
    echo "   ⚠️  EVENT_DO binding NOT found in deploy output"
  fi

  return 0
}

# ── Main retry loop ──
main() {
  move_pnp

  # Uncomment DO binding before first deploy attempt if requested
  if [ "$UNCOMMENT_DO" = true ]; then
    uncomment_do_bindings
  fi

  local attempt=0

  echo "🚀 Starting deployment retry loop..."
  echo "   Interval: ${INTERVAL}s | Max attempts: $([ "$MAX_ATTEMPTS" -eq 0 ] && echo "unlimited" || echo "$MAX_ATTEMPTS")"
  echo ""

  while true; do
    attempt=$((attempt + 1))

    echo "🔄 Attempt #${attempt} — $(date '+%Y-%m-%d %H:%M:%S')"

    # Capture deploy output (both stdout and stderr)
    DEPLOY_OUTPUT=""
    if DEPLOY_OUTPUT=$(npx wrangler deploy 2>&1); then
      echo "✅ Deploy succeeded on attempt #${attempt}!"
      echo ""
      echo "$DEPLOY_OUTPUT"
      echo ""

      # Verify production endpoints
      verify_production || true

      echo ""
      echo "🎉 Deployment complete after ${attempt} attempt(s)"
      notify "Deploy Success" "Succeeded on attempt #${attempt}"
      restore_pnp
      exit 0
    fi

    # Deploy failed — check error type
    if echo "$DEPLOY_OUTPUT" | grep -q "10013"; then
      # Error 10013 — API degraded, retry
      echo "   ❌ Error 10013 (API degraded) — will retry in ${INTERVAL}s"
      echo ""

      # Check max attempts
      if [ "$MAX_ATTEMPTS" -gt 0 ] && [ "$attempt" -ge "$MAX_ATTEMPTS" ]; then
        echo "❌ Reached max attempts ($MAX_ATTEMPTS) — giving up"
        notify "Deploy Failed" "Max attempts ($MAX_ATTEMPTS) reached"
        restore_pnp
        exit 1
      fi

      echo "⏳ Waiting ${INTERVAL}s before next attempt..."
      sleep "$INTERVAL"
    else
      # Different error — don't retry
      echo "❌ Deploy failed with non-retryable error:"
      echo ""
      echo "$DEPLOY_OUTPUT"
      notify "Deploy Failed" "Non-retryable error on attempt #${attempt}"
      restore_pnp
      exit 1
    fi
  done
}

main
