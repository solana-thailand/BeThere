#!/usr/bin/env bash
# deploy.sh — Deploy or run BeThere worker locally
# Handles Yarn PnP (~/.pnp.cjs) conflict with wrangler's esbuild bundler.
#
# Usage:
#   ./deploy.sh              # Deploy to production
#   ./deploy.sh dev          # Start dev server with remote KV (production data)
#   ./deploy.sh dev --local  # Start dev server with local SQLite KV (empty)

set -uo pipefail
cd "$(dirname "$0")"

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

move_pnp

if [ "${1:-}" = "dev" ]; then
  if [ "${2:-}" = "--local" ]; then
    echo "🔧 Starting local dev server (SQLite KV) on http://localhost:8787 ..."
    echo "   ⚠️  Local KV is empty — use --remote for real data or seed first."
    npx wrangler dev --port 8787 --local
  else
    echo "🔧 Starting dev server with remote KV on http://localhost:8787 ..."
    echo "   Using production KV namespace (read/write!)."
    echo "   Tip: Use bash scripts/seed_dev.sh to copy data first."
    npx wrangler dev --port 8787 --remote
  fi
else
  echo "🚀 Deploying to Cloudflare Workers..."
  npx wrangler deploy
fi

restore_pnp
