#!/usr/bin/env bash
# deploy.sh — Deploy or run BeThere worker locally
# Handles Yarn PnP (~/.pnp.cjs) conflict with wrangler's esbuild bundler.
#
# Usage:
#   ./deploy.sh              # Deploy to production
#   ./deploy.sh dev          # Start dev server with remote KV (production data)
#   ./deploy.sh dev --local  # Start dev server with local SQLite KV (empty)
#
# Wrangler 4.x uses the /versions API which returns 500 (code 10013).
# This script works around it by:
#   1. Running `wrangler deploy` which uploads assets + bundles worker code
#   2. If /versions fails, extracting the assets JWT from the successful upload
#   3. Using the legacy PUT API with the assets JWT included in metadata

set -uo pipefail
cd "$(dirname "$0")"

PNP_FILE="$HOME/.pnp.cjs"
PNP_BACKUP="$HOME/.pnp.cjs.bak"
MOVED=false

WORKER_NAME="bethere"
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
  echo "🚀 Deploying to Cloudflare Workers..."

  # ── Step 1: Try standard wrangler deploy ──
  # wrangler deploy uploads assets via assets-upload-session (which works),
  # then calls /versions (which fails with 10013).
  # The assets are already uploaded and cached by Cloudflare at this point.
  if npx wrangler deploy 2>&1; then
    echo "✅ Deployed via wrangler"
    restore_pnp
    exit 0
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
  MANIFEST=$(python3 -c "
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

  # Initialize asset upload session
  INIT_RESPONSE=$(curl -s -X POST \
    "${API_BASE}/workers/scripts/${WORKER_NAME}/assets-upload-session" \
    -H "Authorization: Bearer ${OAUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d "$MANIFEST")

  # Extract JWT and buckets
  ASSETS_JWT=$(echo "$INIT_RESPONSE" | python3 -c "import json,sys; r=json.load(sys.stdin); print(r.get('result',{}).get('jwt',''))" 2>/dev/null)
  BUCKETS=$(echo "$INIT_RESPONSE" | python3 -c "
import json, sys
r = json.load(sys.stdin)
buckets = r.get('result',{}).get('buckets',[])
count = sum(len(b) for b in buckets)
print(count)
" 2>/dev/null)

  if [ -z "$ASSETS_JWT" ]; then
    echo "❌ Failed to get assets JWT"
    echo "$INIT_RESPONSE" | python3 -m json.tool 2>/dev/null || echo "$INIT_RESPONSE"
    restore_pnp
    exit 1
  fi

  if [ "$BUCKETS" = "0" ]; then
    echo "   Assets already up-to-date (no new files to upload)"
  else
    echo "   Uploading ${BUCKETS} asset file(s)..."

    # Upload each file that needs uploading
    python3 -c "
import os, json, base64, subprocess
import blake3

dist = '${DIST_DIR}'
token = '${ASSETS_JWT}'
api = '${API_BASE}/workers/assets/upload?base64=true'

# Build file lookup by hash
manifest = {}
for root, dirs, files in os.walk(dist):
    for f in files:
        fp = os.path.join(root, f)
        contents = open(fp, 'rb').read()
        b64 = base64.b64encode(contents).decode()
        ext = os.path.splitext(fp)[1].lstrip('.')
        h = blake3.blake3((b64 + ext).encode()).hexdigest()[:32]
        manifest[h] = base64.b64encode(contents).decode()

# For each file, upload via the API
count = 0
total = len(manifest)
for file_hash, content_b64 in manifest.items():
    count += 1
    payload = json.dumps({file_hash: content_b64})
    result = subprocess.run([
        'curl', '-s', '-X', 'POST', api,
        '-H', f'Authorization: Bearer {token}',
        '-H', 'Content-Type: application/json',
        '-d', payload
    ], capture_output=True, text=True)

    try:
        resp = json.loads(result.stdout)
        new_jwt = resp.get('result', {}).get('jwt', '')
        if new_jwt:
            # Update the JWT for subsequent uploads
            with open('/tmp/bethere_assets_jwt.txt', 'w') as jf:
                jf.write(new_jwt)
        print(f'  Uploaded {count}/{total}')
    except:
        print(f'  Upload {count}/{total} response: {result.stdout[:200]}')
"

    # Read the final JWT after all uploads
    if [ -f /tmp/bethere_assets_jwt.txt ]; then
      ASSETS_JWT=$(cat /tmp/bethere_assets_jwt.txt)
      rm -f /tmp/bethere_assets_jwt.txt
    fi
  fi

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
  METADATA=$(python3 -c "
import json
m = {
    'main_module': 'shim.js',
    'compatibility_date': '2024-09-23',
    'compatibility_flags': ['nodejs_compat'],
    'bindings': [
        {'type': 'kv_namespace', 'name': 'QUIZ', 'namespace_id': 'faf9eebaa53d46f9a82c1f6db6dfbc05'},
        {'type': 'kv_namespace', 'name': 'EVENTS', 'namespace_id': 'c8a6a87f9ed34ce0a3c8e48b84039214'},
        {'type': 'd1', 'name': 'DB', 'id': '98d09542-e7d8-4413-ac34-4276a50d126c'},
        {'type': 'r2_bucket', 'name': 'ASSETS_BUCKET', 'bucket_name': 'bethere-assets'}
    ],
    'vars': {
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
    },
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
    STARTUP_MS=$(echo "$BODY" | python3 -c "import json,sys; r=json.load(sys.stdin); print(r.get('result',{}).get('startup_time_ms','?'))" 2>/dev/null || echo "?")
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
    echo "$BODY" | python3 -m json.tool 2>/dev/null || echo "$BODY"
    restore_pnp
    exit 1
  fi
fi

restore_pnp
