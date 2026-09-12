#!/usr/bin/env bash
# deploy.sh — Deploy or run BeThere worker locally
# Handles Yarn PnP (~/.pnp.cjs) conflict with wrangler's esbuild bundler.
#
# Usage:
#   ./deploy.sh              # Deploy to production (default)
#   ./deploy.sh staging      # Deploy to staging (bethere-staging) — reads [env.staging]
#   ./deploy.sh dev          # Start dev server with remote KV (production data)
#   ./deploy.sh dev --local  # Start dev server with local SQLite KV (empty)
#
# §3.5 preflight gate (DEFAULT-ON, production-only):
#   Production requires a green flow-harness run within the last hour. The gate
#   reads the .last-green sentinel mtime (see worker/scripts/preflight.sh).
#   ./deploy.sh --force --reason "hotfix X"   # Bypass the gate (logs an audit entry)
#
# Staging note: the PUT API fallback below is PRODUCTION-ONLY. It reads the
# top-level [vars] and bindings from wrangler.toml and targets the prod account
# and worker name; it does not resolve [env.staging]. `deploy.sh staging` uses
# the standard `wrangler deploy --env staging` path only; if that fails, it
# reports and exits rather than falling back to the production PUT flow.
#
# Wrangler 4.x uses the /versions API which returns 500 (code 10013).
# This script works around it by:
#   1. Running `wrangler deploy` which uploads assets + bundles worker code
#   2. If /versions fails, extracting the assets JWT from the successful upload
#   3. Using the legacy PUT API with the assets JWT included in metadata

set -euo pipefail

# Source fnm (Node manager) for npx/node
# Use fnm's default node installation directly to avoid shell integration issues
FNM_NODE_BIN="$HOME/.local/share/fnm/node-versions/v24.16.0/installation/bin"
if [ ! -d "$FNM_NODE_BIN" ]; then
  # Fallback: find any fnm-managed node
  FNM_NODE_BIN="$(find "$HOME/.local/share/fnm/node-versions" -path "*/installation/bin" -type d 2>/dev/null | head -1)"
fi
if [ -n "$FNM_NODE_BIN" ] && [ -d "$FNM_NODE_BIN" ]; then
  export PATH="$FNM_NODE_BIN:$PATH"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

PNP_FILE="$HOME/.pnp.cjs"
PNP_BACKUP="$HOME/.pnp.cjs.bak"
MOVED=false

# ── Argument parsing ─────────────────────────────────────────────────────────
# Backward-compatible positional env (production | staging | dev) plus §3.5
# flags: --force (bypass the preflight gate, logs an audit entry) and
# --reason "..." (required for every bypass). The preflight gate always runs
# when deploy targets production.
DEPLOY_ENV="production"
DEPLOY_DEV_LOCAL=false
DEPLOY_FORCE=false
DEPLOY_FORCE_REASON=""

while [ $# -gt 0 ]; do
  case "$1" in
    staging)         DEPLOY_ENV="staging"; shift ;;
    dev)             DEPLOY_ENV="dev"; shift ;;
    --local)         DEPLOY_DEV_LOCAL=true; shift ;;
    --force)         DEPLOY_FORCE=true; shift ;;
    --reason)
      if [ $# -ge 2 ]; then
        DEPLOY_FORCE_REASON="$2"; shift 2
      else
        echo "❌ deploy.sh: --reason requires a value" >&2; exit 2
      fi
      ;;
    --reason=*)      DEPLOY_FORCE_REASON="${1#--reason=}"; shift ;;
    *)               echo "❌ deploy.sh: unknown argument: $1" >&2; exit 2 ;;
  esac
done

WORKER_NAME="bethere"
WRANGLER_ENV_ARGS=("--env=")
if [ "$DEPLOY_ENV" = "staging" ]; then
  WORKER_NAME="bethere-staging"
  WRANGLER_ENV_ARGS=("--env" "staging")
fi
ACCOUNT_ID="bb8f9ffa91e24d9ce850cbbc4fd45935"
DIST_DIR="../frontend-leptos/dist"

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

# ── wasm-bindgen version guard ───────────────────────────────────────────────
# wasm-bindgen has TWO halves that must match EXACTLY (its bindgen schema is
# unstable): the `wasm-bindgen` CRATE compiled into the .wasm during
# `cargo build`, and the `wasm-bindgen` CLI that post-processes it (invoked by
# wrangler's custom build in wrangler.toml). The crate is pinned by the tracked
# root Cargo.lock (CI enforces via --locked), but the CLI is a global binary
# installed separately with `cargo install` — no lockfile can pin it, so it
# drifts independently. When it does, the wasm build runs for ~2 minutes and
# then dies with a cryptic "schema version" error deep in wrangler output.
#
# This guard reads the crate version from Cargo.lock and compares it to the
# installed CLI up front, failing fast with the exact remediation command.
REPO_LOCK="$(cd "$SCRIPT_DIR/.." && pwd)/Cargo.lock"

check_wasm_bindgen_version() {
  # Non-fatal if we can't determine the expected version (don't block deploys
  # on a parsing edge case) — the build itself still enforces the real match.
  [ -f "$REPO_LOCK" ] || { echo "ℹ️  Cargo.lock not found ($REPO_LOCK) — skipping wasm-bindgen version guard."; return 0; }

  local want have
  want=$(awk '/^name = "wasm-bindgen"$/{f=1;next} f&&/^version = /{gsub(/[",]/,"");print $3;exit}' "$REPO_LOCK")
  [ -n "$want" ] || { echo "ℹ️  Could not parse wasm-bindgen version from Cargo.lock — skipping guard."; return 0; }

  if ! command -v wasm-bindgen >/dev/null 2>&1; then
    echo "❌ wasm-bindgen CLI not installed (crate pins $want)."
    echo "   Fix: cargo install -f wasm-bindgen-cli --version $want"
    return 1
  fi

  have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')
  if [ "$have" != "$want" ]; then
    echo "❌ wasm-bindgen CLI/crate version mismatch — the wasm build WILL fail:"
    echo "   crate (Cargo.lock): $want"
    echo "   CLI  (installed):   ${have:-unknown}"
    echo "   They must match exactly (unstable bindgen schema)."
    echo "   Fix: cargo install -f wasm-bindgen-cli --version $want"
    return 1
  fi

  echo "✅ wasm-bindgen CLI matches crate ($want)"
  return 0
}

# ── Post-deploy content-type verification ───────────────────────────────────
# Find a Python interpreter able to run the PUT-fallback generators.
# Sets PYTHON_BIN. Requires 3.11+ (tomllib) and the blake3 package.
PYTHON_BIN=""
resolve_python() {
  local candidate
  for candidate in "${DEPLOY_PYTHON:-}" /opt/homebrew/bin/python3 "$(command -v python3 || true)"; do
    [ -n "$candidate" ] && [ -x "$candidate" ] || continue
    if "$candidate" -c 'import sys, tomllib, blake3; sys.exit(0 if sys.version_info >= (3, 11) else 1)' 2>/dev/null; then
      PYTHON_BIN="$candidate"
      return 0
    fi
  done
  echo "❌ No usable Python for the PUT API fallback."
  echo "   Need Python 3.11+ (tomllib) with the blake3 package:"
  echo "     python3 -m pip install blake3"
  echo "   Override the interpreter with DEPLOY_PYTHON=/path/to/python3."
  return 1
}

# A 2026-07-26 deploy shipped `/` and the JS bundle as `application/octet-stream`
# (browsers downloaded a .dms file / blank page) yet returned HTTP 200 — the
# status-only smoke test missed it. Root cause was a poisoned content-addressed
# asset object on the CDN (see the BUILD_TAG note in frontend-leptos/src/lib.rs).
# This check curls the just-deployed origin and FAILS the deploy if the HTML
# shell or the hashed JS bundle is served as octet-stream, so it can never ship
# silently again.
verify_content_types() {
  local base="https://${WORKER_NAME}.solana-thailand.workers.dev"
  local index="${DIST_DIR}/index.html"
  [ -f "$index" ] || { echo "ℹ️  no ${index} — skipping content-type verification."; return 0; }

  local js
  js=$(grep -o 'event-checkin-frontend-[a-z0-9]*\.js' "$index" | head -1)

  echo "🔎 Verifying served Content-Type (edge propagation may lag a few seconds)..."
  local bad=0 ct expected
  for path in "/" "/$js"; do
    [ "$path" = "/" ] || [ -n "$js" ] || continue
    expected="text/html"
    [ "$path" = "/" ] || expected="text/javascript"
    # Retry a few times to ride out edge propagation right after deploy.
    for _ in 1 2 3 4 5; do
      ct=$(curl -s -D - -o /dev/null "${base}${path}" | tr -d '\r' | grep -i '^content-type:' | sed 's/[Cc]ontent-[Tt]ype: *//')
      echo "$ct" | grep -qi "^${expected}" && break
      sleep 4
    done
    if ! echo "$ct" | grep -qi "^${expected}"; then
      echo "   ❌ ${path} → ${ct:-<missing>} (expected ${expected})"
      bad=1
    else
      echo "   ✅ ${path} → ${ct}"
    fi
  done

  if [ "$bad" -ne 0 ]; then
    echo ""
    echo "❌ DEPLOY SERVED an invalid Content-Type — the site may download or render HTML for an asset." >&2
    echo "   Remediation:" >&2
    echo "     1. Roll back:  npx wrangler rollback <last-good-version-id>" >&2
    echo "     2. Bump BUILD_TAG in frontend-leptos/src/lib.rs (forces a fresh JS-glue" >&2
    echo "        content hash that was never poisoned), rebuild, and redeploy." >&2
    return 1
  fi
  echo "✅ Content-Type verification passed."
  return 0
}

# ── §3.5 Preflight gate (opt-in, production-only) ────────────────────────────
# Production deploys require a green flow-harness run within the last hour
# (PREFLIGHT_MAX_AGE_SECONDS). --force --reason bypasses the gate and appends a
# mandatory audit entry to worker/scripts/.preflight-bypass.log. Staging/dev
# deploys skip the gate.
SCRIPTS_DIR="$SCRIPT_DIR/scripts"
PREFLIGHT_SCRIPT="$SCRIPTS_DIR/preflight.sh"
PREFLIGHT_AUDIT_LOG="$SCRIPTS_DIR/.preflight-bypass.log"

# Append a structured audit entry when --force bypasses the gate.
log_preflight_bypass() {
  local ts user commit reason
  ts=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  user=$(whoami 2>/dev/null || echo "unknown")
  commit=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
  reason="${DEPLOY_FORCE_REASON:-<no reason given>}"
  printf '%s\tuser=%s\tcommit=%s\tenv=%s\treason=%s\n' \
    "$ts" "$user" "$commit" "$DEPLOY_ENV" "$reason" >> "$PREFLIGHT_AUDIT_LOG"
}

# Enforce the preflight gate. Returns 0 to proceed, non-zero to block.
run_preflight_gate() {
  # Only production deploys are gated.
  if [ "$DEPLOY_ENV" != "production" ]; then
    return 0
  fi

  # --force bypasses the gate but logs an audit entry (never silent).
  if [ "$DEPLOY_FORCE" = true ]; then
    if [ -z "$DEPLOY_FORCE_REASON" ]; then
      echo "❌ --force requires --reason \"<incident/change reason>\"." >&2
      echo "   Production preflight bypasses must be attributable and reviewable." >&2
      return 2
    fi
    log_preflight_bypass
    echo "⚠️  Preflight gate BYPASSED via --force (audit entry logged to .preflight-bypass.log)."
    if [ -n "$DEPLOY_FORCE_REASON" ]; then
      echo "    reason: $DEPLOY_FORCE_REASON"
    fi
    return 0
  fi

  if [ ! -f "$PREFLIGHT_SCRIPT" ]; then
    echo "❌ Production preflight gate is required but preflight.sh was not found:" >&2
    echo "   $PREFLIGHT_SCRIPT" >&2
    return 1
  fi

  echo "🔍 Running required preflight gate (env=production)..."
  if bash "$PREFLIGHT_SCRIPT"; then
    echo "✅ Preflight gate passed — proceeding with production deploy."
    return 0
  else
    local rc=$?
    echo "" >&2
    echo "❌ Preflight gate FAILED (exit $rc) — production deploy blocked." >&2
    echo "   Remediation:" >&2
    echo "     1. Run a green harness:  bash worker/scripts/preflight.sh run" >&2
    echo "     2. Or bypass with audit: bash worker/deploy.sh --force --reason \"<why>\"" >&2
    return 1
  fi
}

# Guard the wasm-bindgen CLI/crate match before any build work (dev, staging,
# and production all trigger the wasm build). Fail fast with the fix command
# instead of a cryptic schema error minutes into the build.
check_wasm_bindgen_version || { echo "Aborting: fix the wasm-bindgen CLI version above, then re-run."; exit 1; }

# Run the gate before any deploy work (fail fast, before touching ~/.pnp.cjs).
run_preflight_gate || { echo "Aborting production deploy."; exit 1; }

move_pnp

if [ "$DEPLOY_ENV" = "dev" ]; then
  if [ "$DEPLOY_DEV_LOCAL" = true ]; then
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
  if [ "$DEPLOY_ENV" = "staging" ]; then
    echo "🚀 Deploying to Cloudflare Workers (STAGING: ${WORKER_NAME})..."
  else
    echo "🚀 Deploying to Cloudflare Workers (production: ${WORKER_NAME})..."
  fi

  echo "🏗️  Building Leptos WASM frontend via build.sh..."
  (cd "../frontend-leptos" && bash build.sh)

  if [ -f "../frontend-leptos/_headers" ] && [ -d "${DIST_DIR}" ]; then
    cp -f "../frontend-leptos/_headers" "${DIST_DIR}/_headers"
    echo "📋 Copied _headers to ${DIST_DIR}/_headers for edge asset cache rules."
  fi

  # ── Step 1: Try standard wrangler deploy ──
  if CI=true npx wrangler deploy "${WRANGLER_ENV_ARGS[@]}" 2>&1; then
    echo "✅ Deployed via wrangler"
    if verify_content_types; then
      restore_pnp
      exit 0
    else
      restore_pnp
      exit 1
    fi
  fi

  # Staging has no PUT API fallback (that path is production-hardcoded; see
  # header note). Surface the failure clearly and stop.
  if [ "$DEPLOY_ENV" = "staging" ]; then
    echo "❌ wrangler deploy --env staging failed."
    echo "   Staging intentionally does not use the production PUT API fallback."
    echo "   Re-run once the Cloudflare versions API recovers, or check [env.staging] config."
    restore_pnp
    exit 1
  fi

  echo ""
  echo "⚠️  wrangler deploy failed (likely versions API bug 10013)"
  echo "   Falling back to: PUT API + asset re-upload..."
  echo ""

  # ── Step 1b: Resolve the interpreter the fallback generators need ──
  # The generators live in worker/scripts/deploy_fallback/ as tracked modules
  # (Issue #065). They need tomllib (3.11+) to read wrangler.toml and the
  # blake3 package to reproduce wrangler's asset hashes. Fail here with an
  # actionable message rather than half-way through an upload.
  if ! resolve_python; then
    restore_pnp
    exit 1
  fi
  export PYTHONPATH="$SCRIPT_DIR/scripts${PYTHONPATH:+:$PYTHONPATH}"

  # ── Step 2: Extract OAuth token ──
  WRANGLER_CONFIG="$HOME/Library/Preferences/.wrangler/config/default.toml"
  if [ ! -f "$WRANGLER_CONFIG" ]; then
    echo "❌ No wrangler auth config found. Run 'npx wrangler login' first."
    restore_pnp
    exit 1
  fi

  OAUTH_TOKEN=$(grep "oauth_token" "$WRANGLER_CONFIG" | head -1 | sed 's/.*= *"\(.*\)".*/\1/')
  if [ -z "$OAUTH_TOKEN" ]; then
    echo "❌ No OAuth token found. Run 'npx wrangler login' first."
    restore_pnp
    exit 1
  fi

  # Refresh token
  npx wrangler whoami >/dev/null 2>&1
  OAUTH_TOKEN=$(grep "oauth_token" "$WRANGLER_CONFIG" | head -1 | sed 's/.*= *"\(.*\)".*/\1/')

  API_BASE="https://api.cloudflare.com/client/v4/accounts/${ACCOUNT_ID}"

  # ── Step 3: Upload assets and get JWT ──
  echo "📤 Uploading static assets..."

  # Build asset manifest (path → { hash, size }) using BLAKE3 (matching wrangler's hashFile)
  if ! MANIFEST=$("$PYTHON_BIN" -m deploy_fallback.cli manifest --dist "$DIST_DIR"); then
    echo "❌ Could not build the asset manifest."
    restore_pnp
    exit 1
  fi

  # Initialize asset upload session, upload only the requested files, and
  # obtain the COMPLETION JWT (not the init/upload JWT).
  #
  # The assets upload flow (matching wrangler's syncAssets):
  #   1. POST .../assets-upload-session with the manifest → init JWT + buckets
  #   2. If buckets is non-empty, POST .../workers/assets/upload?base64=true
  #      as multipart/form-data (NOT JSON), one request per bucket, each field
  #      named by file hash with the base64 content as the value. Auth uses the
  #      init JWT. Each response's result.jwt is the running completion JWT.
  #   3. The final completion JWT is what the PUT API needs in assets.jwt.
  #
  # Previous bugs this fixes:
  #   - Uploaded ALL files every time instead of only the requested buckets.
  #   - Used Content-Type: application/json; the API requires multipart/form-data
  #     (uploads silently failed → no completion JWT → fell back to init JWT).
  #   - Did not strip the `cfwau_` prefix the new API adds to every JWT.
  if ! ASSETS_JWT=$(printf '%s' "$MANIFEST" \
    | CLOUDFLARE_OAUTH_TOKEN="$OAUTH_TOKEN" "$PYTHON_BIN" \
      -m deploy_fallback.cli upload \
      --dist "$DIST_DIR" --api-base "$API_BASE" --script "$WORKER_NAME"); then
    echo "❌ Asset upload failed."
    restore_pnp
    exit 1
  fi

  # The `cfwau_` prefix the upload API adds is stripped in
  # deploy_fallback/upload.py (strip_jwt_prefix) — the PUT API rejects a
  # prefixed JWT with 10021 "could not JSON decode header".

  echo "   Assets JWT obtained: ${ASSETS_JWT:0:20}..."

  # ── Step 4: Bundle and deploy worker code via PUT API with assets JWT ──
  DRY_DIR=$(mktemp -d)
  echo "📦 Bundling worker (dry-run)..."
  if ! npx wrangler deploy --env="" --dry-run --outdir "$DRY_DIR" 2>&1; then
    echo "❌ Dry-run bundling failed."
    rm -rf "$DRY_DIR"
    restore_pnp
    exit 1
  fi

  SHIM_JS=$(find "$DRY_DIR" -name "shim.js" -not -name "*.map" | head -1)
  WASM_FILE=$(find "$DRY_DIR" -name "*.wasm" | head -1)

  if [ -z "$SHIM_JS" ] || [ -z "$WASM_FILE" ]; then
    echo "❌ Could not find bundled files in dry-run output."
    ls -la "$DRY_DIR"
    rm -rf "$DRY_DIR"
    restore_pnp
    exit 1
  fi

  # Build metadata JSON: assets JWT + every [vars] entry and binding declared in
  # wrangler.toml. deploy_fallback/metadata.py owns the API shape (env vars must
  # be `plain_text` bindings, not a top-level `vars` dict) and prints a warning
  # for anything wrangler.toml declares that the PUT API cannot carry.
  METADATA=$(CLOUDFLARE_ASSETS_JWT="$ASSETS_JWT" "$PYTHON_BIN" \
    -m deploy_fallback.cli metadata --config wrangler.toml --main-module shim.js)

  echo "📤 Deploying worker code + assets binding..."
  RESPONSE=$(curl -s -w "\n%{http_code}" \
    -X PUT "${API_BASE}/workers/scripts/${WORKER_NAME}" \
    -H "Authorization: Bearer ${OAUTH_TOKEN}" \
    -F "metadata=${METADATA};type=application/json" \
    -F "shim.js=@${SHIM_JS};type=application/javascript+module" \
    -F "${WASM_FILE##*/}=@${WASM_FILE};type=application/wasm" \
    2>&1)

  HTTP_CODE=$(echo "$RESPONSE" | tail -1)
  BODY=$(echo "$RESPONSE" | sed '$d')

  rm -rf "$DRY_DIR"

  if [ "$HTTP_CODE" = "200" ]; then
    STARTUP_MS=$(echo "$BODY" | "$PYTHON_BIN" -c "import json,sys; r=json.load(sys.stdin); print(r.get('result',{}).get('startup_time_ms','?'))" 2>/dev/null || echo "?")
    echo "✅ Deployed successfully! (startup: ${STARTUP_MS}ms)"
    echo "   https://${WORKER_NAME}.solana-thailand.workers.dev"

    # Verify assets are served
    sleep 3
    JS_FILE=$(grep -o 'event-checkin-frontend-[a-z0-9]*\.js' "${DIST_DIR}/index.html" | head -1)
    JS_SIZE=$(curl -s -o /dev/null -w "%{size_download}" "https://${WORKER_NAME}.solana-thailand.workers.dev/${JS_FILE}")
    if [ "$JS_SIZE" -gt 10000 ]; then
      echo "   ✅ Frontend assets served correctly (${JS_SIZE} bytes)"
    else
      echo "   ⚠️  Frontend assets may not be served (got ${JS_SIZE} bytes, expected ~75000)"
      echo "   Try running this script again to re-upload assets."
    fi
    if ! verify_content_types; then
      restore_pnp
      exit 1
    fi
  else
    echo "❌ Deploy failed (HTTP ${HTTP_CODE})"
    echo "$BODY" | "$PYTHON_BIN" -m json.tool 2>/dev/null || echo "$BODY"
    restore_pnp
    exit 1
  fi
fi

restore_pnp
