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
# Staging note: the PUT API fallback below is PRODUCTION-ONLY (its bindings/vars
# are hardcoded for the prod worker). `deploy.sh staging` uses the standard
# `wrangler deploy --env staging` path only; if that fails, it reports and exits
# rather than falling back to the production-hardcoded PUT flow.
#
# Wrangler 4.x uses the /versions API which returns 500 (code 10013).
# This script works around it by:
#   1. Running `wrangler deploy` which uploads assets + bundles worker code
#   2. If /versions fails, extracting the assets JWT from the successful upload
#   3. Using the legacy PUT API with the assets JWT included in metadata

set -uo pipefail

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

cd "$(dirname "$0")"

PNP_FILE="$HOME/.pnp.cjs"
PNP_BACKUP="$HOME/.pnp.cjs.bak"
MOVED=false

WORKER_NAME="bethere"
WRANGLER_ENV_FLAG=""
if [ "${1:-}" = "staging" ]; then
  WORKER_NAME="bethere-staging"
  WRANGLER_ENV_FLAG="--env staging"
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
  if [ -n "$WRANGLER_ENV_FLAG" ]; then
    echo "🚀 Deploying to Cloudflare Workers (STAGING: ${WORKER_NAME})..."
  else
    echo "🚀 Deploying to Cloudflare Workers (production: ${WORKER_NAME})..."
  fi

  # ── Step 1: Try standard wrangler deploy ──
  # wrangler deploy uploads assets via assets-upload-session (which works),
  # then calls /versions (which fails with 10013).
  # The assets are already uploaded and cached by Cloudflare at this point.
  if CI=true npx wrangler deploy $WRANGLER_ENV_FLAG 2>&1; then
    echo "✅ Deployed via wrangler"
    restore_pnp
    exit 0
  fi

  # Staging has no PUT API fallback (that path is production-hardcoded; see
  # header note). Surface the failure clearly and stop.
  if [ -n "$WRANGLER_ENV_FLAG" ]; then
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
  MANIFEST=$(/opt/homebrew/bin/python3 -c "
import os, json, base64
import blake3

dist = '${DIST_DIR}'
manifest = {}
for root, dirs, files in os.walk(dist):
    for f in files:
        fp = os.path.join(root, f)
        rel = '/' + os.path.relpath(fp, dist)
        contents = open(fp, 'rb').read()
        b64 = base64.b64encode(contents).decode()
        ext = os.path.splitext(fp)[1].lstrip('.')
        h = blake3.blake3((b64 + ext).encode()).hexdigest()[:32]
        manifest[rel] = {'hash': h, 'size': os.path.getsize(fp)}
print(json.dumps({'manifest': manifest}))
")

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
  ASSETS_JWT=$(echo "$MANIFEST" | /opt/homebrew/bin/python3 -c "
import json, sys, base64, os
from urllib.request import Request, urlopen
from urllib.error import HTTPError

def status(msg):
    # Progress goes to stderr so it isn't captured into ASSETS_JWT.
    print(msg, file=sys.stderr)

manifest_body = json.load(sys.stdin)['manifest']
api_base = '${API_BASE}'
script = '${WORKER_NAME}'
oauth = '${OAUTH_TOKEN}'
dist = '${DIST_DIR}'

# Build hash → base64-content lookup (matches the manifest hashing).
content_by_hash = {}
for rel, info in manifest_body.items():
    fp = os.path.join(dist, rel.lstrip('/'))
    raw = open(fp, 'rb').read()
    content_by_hash[info['hash']] = base64.b64encode(raw).decode()

# 1. Initialize upload session.
init_req = Request(
    f'{api_base}/workers/scripts/{script}/assets-upload-session',
    data=json.dumps({'manifest': manifest_body}).encode(),
    headers={'Authorization': f'Bearer {oauth}', 'Content-Type': 'application/json'},
    method='POST',
)
with urlopen(init_req) as r:
    init = json.load(r)['result']
init_jwt = init['jwt']
buckets = init.get('buckets', [])
requested = [h for bucket in buckets for h in bucket]

if not requested:
    status(f'   Assets already up-to-date (no new files to upload)')
    print(init_jwt)
    sys.exit(0)

status(f'   Uploading {len(requested)} asset file(s)...')

# 2. Upload each requested file via multipart/form-data. The API returns a
#    fresh completion JWT in result.jwt on every successful upload.
completion_jwt = ''
upload_url = f'{api_base}/workers/assets/upload?base64=true'
for i, file_hash in enumerate(requested, 1):
    b64 = content_by_hash[file_hash]
    # Build multipart/form-data body by hand (field name = hash, value = b64).
    boundary = '----bethere' + os.urandom(8).hex()
    body = (
        f'--{boundary}\r\n'
        f'Content-Disposition: form-data; name=\"{file_hash}\"; filename=\"{file_hash}\"\r\n'
        f'Content-Type: application/octet-stream\r\n\r\n'
        f'{b64}\r\n'
        f'--{boundary}--\r\n'
    ).encode()
    req = Request(
        upload_url,
        data=body,
        headers={
            'Authorization': f'Bearer {init_jwt}',
            'Content-Type': f'multipart/form-data; boundary={boundary}',
        },
        method='POST',
    )
    try:
        with urlopen(req) as r:
            resp = json.load(r)
        new_jwt = resp.get('result', {}).get('jwt', '')
        if new_jwt:
            completion_jwt = new_jwt
        status(f'  Uploaded {i}/{len(requested)}')
    except HTTPError as e:
        status(f'  Upload {i}/{len(requested)} FAILED: {e.code} {e.read().decode()[:300]}')
        sys.exit(1)

if not completion_jwt:
    status('   No completion JWT returned by upload API')
    sys.exit(1)
print(completion_jwt)
")
  UPLOAD_EXIT=$?
  if [ "$UPLOAD_EXIT" -ne 0 ]; then
    echo "❌ Asset upload failed."
    restore_pnp
    exit 1
  fi

  # Defensive: strip the `cfwau_` prefix if present. The assets-upload-session
  # API prefixes upload-token JWTs with `cfwau_` (Cloudflare Workers Assets
  # Upload). Completion JWTs returned after upload don't carry it, but the init
  # JWT (used verbatim when nothing needs uploading) sometimes does. The legacy
  # PUT API rejects a prefixed JWT with 10021 "could not JSON decode header".
  ASSETS_JWT="${ASSETS_JWT#cfwau_}"

  echo "   Assets JWT obtained: ${ASSETS_JWT:0:20}..."

  # ── Step 4: Bundle and deploy worker code via PUT API with assets JWT ──
  DRY_DIR=$(mktemp -d)
  echo "📦 Bundling worker (dry-run)..."
  if ! npx wrangler deploy --dry-run --outdir "$DRY_DIR" 2>&1; then
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

  # Build metadata JSON with assets JWT + env vars from wrangler.toml
  #
  # IMPORTANT: Cloudflare's PUT /workers/scripts/{name} API expects env vars
  # as `plain_text` entries in the `bindings` array — NOT as a top-level
  # `vars` dict (that's the wrangler.toml format). Using `vars` silently
  # drops every env var, causing the worker to fall back to hardcoded defaults
  # (e.g. SERVER_URL → "https://event-checkin.workers.dev").
  METADATA=$(/opt/homebrew/bin/python3 -c "
import json

plain_text_vars = {
    'SERVER_URL': 'https://bethere.solana-thailand.workers.dev',
    'CLAIM_BASE_URL': 'https://bethere.solana-thailand.workers.dev/claim',
    'GOOGLE_SHEET_NAME': 'Attendees',
    'GOOGLE_STAFF_SHEET_NAME': 'staff',
    'PLATFORM_SHEET_ID': '1oF54ia6mquO_kB869aQxmz3RD8nDcTRWfXIX0VmndxM',
    'DEV_MODE': '0',
    'DEV_EMAIL': 'ratchapon.poc@gmail.com',
    'SUPER_ADMIN_EMAILS': 'ratchapon.poc@gmail.com,hackathon@colosseum.org',
    'EVENT_NAME': 'Solana x AI Builders: The Road to Mainnet #1 (Bangkok)',
    'EVENT_TAGLINE': 'Deep Dive into Rust, AI Agents, and the Solana Ecosystem',
    'EVENT_LINK': 'https://solana-thailand.github.io/genesis/events/road-to-mainnet-1-bangkok/',
    'EVENT_START_MS': '1777170600000',
    'EVENT_END_MS': '1777183200000',
    'EVENT_DEPOSIT_ENABLED': 'false',
    'EVENT_DEPOSIT_AMOUNT_USDC': '0',
    'EVENT_DEPOSIT_AMOUNT_THB': '0',
    'EVENT_PROMPTPAY_ID': '',
}

bindings = [
    {'type': 'kv_namespace', 'name': 'EVENTS', 'namespace_id': 'c8a6a87f9ed34ce0a3c8e48b84039214'},
    {'type': 'd1', 'name': 'DB', 'id': '98d09542-e7d8-4413-ac34-4276a50d126c'},
    {'type': 'r2_bucket', 'name': 'ASSETS_BUCKET', 'bucket_name': 'bethere-assets'},
    # DO binding not supported by PUT API (10021 unknown type) — wrangler deploy handles it
]
# Env vars MUST be plain_text bindings for the PUT API to accept them.
for name, value in plain_text_vars.items():
    bindings.append({'type': 'plain_text', 'name': name, 'text': value})

m = {
    'main_module': 'shim.js',
    'compatibility_date': '2024-09-23',
    'compatibility_flags': ['nodejs_compat'],
    'bindings': bindings,

    'assets': {
        'jwt': '${ASSETS_JWT}',
        'router_config': {
            'has_user_worker': True,
        },
        'asset_config': {
            'not_found_handling': 'single-page-application',
        }
    }
}
print(json.dumps(m))
")

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
    STARTUP_MS=$(echo "$BODY" | /opt/homebrew/bin/python3 -c "import json,sys; r=json.load(sys.stdin); print(r.get('result',{}).get('startup_time_ms','?'))" 2>/dev/null || echo "?")
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
  else
    echo "❌ Deploy failed (HTTP ${HTTP_CODE})"
    echo "$BODY" | /opt/homebrew/bin/python3 -m json.tool 2>/dev/null || echo "$BODY"
    restore_pnp
    exit 1
  fi
fi

restore_pnp
