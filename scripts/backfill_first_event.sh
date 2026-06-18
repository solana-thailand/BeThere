#!/usr/bin/env bash
# ============================================================================
# Backfill First Event: Google Sheet → D1 attendee sync
# ============================================================================
# The inaugural event's ~90 attendees live in Google Sheets but were never
# backfilled into D1 (the event predates the Phase 2a dual-write). This script
# triggers the existing `POST /api/events/{id}/sync-sheet` endpoint to import
# them, then verifies the count via the audience aggregation endpoint.
#
# The sheet's column layout differs from the current template (Luma-import
# format), but the header-based column mapping handles it — no code change
# needed. See handover notes + `worker/src/handlers/events/sync.rs`.
#
# Prerequisites:
#   - `cd worker && bash deploy.sh dev --remote` running (reads prod D1)
#   - Either:
#       (a) `worker/.dev.vars` present with `JWT_SECRET` (+ `SUPER_ADMIN_EMAILS`
#           or `DEV_EMAIL`) — the script auto-mints a 24h HS256 staff JWT, OR
#       (b) an explicit `AUTH_TOKEN` env var (log in at /staff, copy
#           `bethere_token` from localStorage).
#
# Usage:
#   EVENT_ID=solana-bkk bash scripts/backfill_first_event.sh              # auto-mint
#   EVENT_ID=solana-bkk bash scripts/backfill_first_event.sh --dry-run    # auto-mint
#   EVENT_ID=solana-bkk AUTH_TOKEN=eyJ... bash scripts/backfill_first_event.sh  # explicit
#
# Env vars:
#   EVENT_ID    (required) The event id to backfill
#   AUTH_TOKEN  (optional) Staff JWT bearer token; auto-minted from .dev.vars if absent
#   MINT_EMAIL  (optional) Email to mint the JWT for; defaults to first
#               SUPER_ADMIN_EMAILS (then DEV_EMAIL) in worker/.dev.vars
#   BASE_URL    (optional) Default: http://localhost:8787
# ============================================================================

set -euo pipefail

# --- Config ---
BASE_URL="${BASE_URL:-http://localhost:8787}"
EVENT_ID="${EVENT_ID:-}"
AUTH_TOKEN="${AUTH_TOKEN:-}"
DRY_RUN=false

EXPECTED_SHEET_ID="1FMQiTsHl1msFVpgcB4aymvxwtkLGR0ulo4UhckCAhdk"

# Parse flags
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=true ;;
    -h|--help)
      sed -n '2,26p' "$0"
      exit 0
      ;;
  esac
done

# --- Colors ---
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

PASS=0
FAIL=0

pass() { PASS=$((PASS + 1)); echo -e "  ${GREEN}✅ PASS${NC} $1"; }
fail() { FAIL=$((FAIL + 1)); echo -e "  ${RED}❌ FAIL${NC} $1"; }
info() { echo -e "  ${CYAN}ℹ️  INFO${NC} $1"; }
section() { echo -e "\n${CYAN}━━━ $1 ━━━${NC}"; }

# --- JSON helper ---
# Extract a dotted key path from JSON on stdin, e.g. json_get "['data']['total']"
json_get() {
  local expr="$1"
  python3 -c "import sys,json; d=json.load(sys.stdin); print(d$expr)" 2>/dev/null || echo "PARSE_ERROR"
}

# --- Whitespace helper ---
# Trim leading/trailing whitespace from stdin. Robust against values that
# contain internal spaces (only the edges are stripped). Used to normalize
# `.dev.vars` entries written as `KEY = "value"` (spaces around `=`).
trim() {
  local s
  s="$(cat)"
  # strip leading whitespace
  s="${s#"${s%%[![:space:]]*}"}"
  # strip trailing whitespace
  s="${s%"${s##*[![:space:]]}"}"
  printf '%s' "$s"
}

# --- Validation ---
section "Arguments"
if [[ -z "$EVENT_ID" ]]; then
  fail "EVENT_ID is required (set env var or pass as EVENT_ID=... bash $0)"
  echo "   Example: EVENT_ID=solana-bkk bash $0"
  exit 1
fi

# --- Resolve auth token: explicit AUTH_TOKEN wins; else auto-mint from .dev.vars ---
if [[ -z "$AUTH_TOKEN" ]]; then
  section "Auto-minting staff JWT from worker/.dev.vars"

  if [[ ! -f "worker/.dev.vars" ]]; then
    fail "AUTH_TOKEN not provided and worker/.dev.vars not found"
    echo "   Either:"
    echo "     1. Set AUTH_TOKEN env var (copy bethere_token from /staff localStorage), OR"
    echo "     2. Create worker/.dev.vars with JWT_SECRET (see .dev.vars.example)"
    exit 1
  fi

  JWT_SECRET=$(grep -E "^JWT_SECRET[[:space:]]*=" worker/.dev.vars | head -n1 | cut -d= -f2- | tr -d '"' | tr -d "'" | trim)
  if [[ -z "$JWT_SECRET" ]]; then
    fail "JWT_SECRET not found in worker/.dev.vars"
    exit 1
  fi

  # Pick the email to mint for: explicit MINT_EMAIL > first SUPER_ADMIN_EMAILS > DEV_EMAIL.
  # `trim` normalizes values written as `KEY = "val"` (spaces around `=`).
  MINT_EMAIL="${MINT_EMAIL:-}"
  if [[ -z "$MINT_EMAIL" ]]; then
    MINT_EMAIL=$(grep -E "^SUPER_ADMIN_EMAILS[[:space:]]*=" worker/.dev.vars | head -n1 | cut -d= -f2- | tr -d '"' | tr -d "'" | cut -d, -f1 | trim)
  fi
  if [[ -z "$MINT_EMAIL" ]]; then
    MINT_EMAIL=$(grep -E "^DEV_EMAIL[[:space:]]*=" worker/.dev.vars | head -n1 | cut -d= -f2- | tr -d '"' | tr -d "'" | trim)
  fi
  if [[ -z "$MINT_EMAIL" ]]; then
    fail "No email to mint JWT for — set MINT_EMAIL, or SUPER_ADMIN_EMAILS/DEV_EMAIL in .dev.vars"
    exit 1
  fi

  # Mint HS256 JWT (same algorithm/payload as the server, mirroring test_full_e2e.sh).
  # Secrets are passed via environment, NOT interpolated into the python source,
  # to avoid leaking into process args / shell history.
  AUTH_TOKEN=$(MINT_EMAIL="$MINT_EMAIL" JWT_SECRET="$JWT_SECRET" python3 -c "
import hmac, hashlib, base64, json, time, os
header_b64 = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9'
now = int(time.time())
payload_data = {
    'email': os.environ['MINT_EMAIL'],
    'sub': 'backfill-script',
    'iat': now,
    'exp': now + 86400,
}
payload = base64.urlsafe_b64encode(json.dumps(payload_data, separators=(',', ':')).encode()).rstrip(b'=').decode()
sign_input = f'{header_b64}.{payload}'
sig = hmac.new(os.environ['JWT_SECRET'].encode(), sign_input.encode(), hashlib.sha256).digest()
signature = base64.urlsafe_b64encode(sig).rstrip(b'=').decode()
print(f'{header_b64}.{payload}.{signature}')
")

  if [[ -z "$AUTH_TOKEN" ]]; then
    fail "Failed to mint JWT"
    exit 1
  fi
  pass "Minted JWT for $MINT_EMAIL (24h expiry)"
else
  pass "Using explicit AUTH_TOKEN from env"
fi
pass "EVENT_ID = $EVENT_ID"
pass "BASE_URL = $BASE_URL"
pass "DRY_RUN  = $DRY_RUN"

# --- 1. Health check ---
section "1. Health check"
# /api/health returns {"status":"ok","d1":{"connected":true,...}} — no top-level
# `ok` field. Check both `status` and `d1.connected` so a half-broken worker
# (D1 unreachable) is still flagged as unhealthy.
health=$(curl -s "$BASE_URL/api/health" || echo '{"status":"error"}')
health_status=$(echo "$health" | json_get "['status']")
d1_connected=$(echo "$health" | json_get "['d1']['connected']")
if [[ "$health_status" == "ok" && "$d1_connected" == "True" ]]; then
  pass "worker healthy at $BASE_URL (status=ok, d1.connected=true)"
else
  fail "worker not healthy — is `bash deploy.sh dev --remote` running?"
  echo "   status=$health_status d1.connected=$d1_connected"
  echo "   Response: $(echo "$health" | head -c 200)"
  exit 1
fi

# --- 2. Verify event config (sheet_id + sheet_name match the first event) ---
section "2. Verify event config"
event_resp=$(curl -s -H "Authorization: Bearer $AUTH_TOKEN" "$BASE_URL/api/events/$EVENT_ID")
event_success=$(echo "$event_resp" | json_get "['success']" || echo "")
if [[ "$event_success" != "True" ]]; then
  fail "could not fetch event '$EVENT_ID'"
  echo "   Response: $(echo "$event_resp" | head -c 300)"
  exit 1
fi

# `GET /api/events/{id}` wraps the EventConfig under `data.event` (see read.rs).
event_name=$(echo "$event_resp" | json_get "['data']['event']['name']")
event_status=$(echo "$event_resp" | json_get "['data']['event']['status']")
sheet_id=$(echo "$event_resp" | json_get "['data']['event']['sheet_id']")
sheet_name=$(echo "$event_resp" | json_get "['data']['event']['sheet_name']")

info "name        = $event_name"
info "status      = $event_status"
info "sheet_id    = $sheet_id"
info "sheet_name  = $sheet_name"

if [[ "$sheet_id" == "$EXPECTED_SHEET_ID" ]]; then
  pass "sheet_id matches the first-event production sheet"
else
  fail "sheet_id mismatch — expected $EXPECTED_SHEET_ID"
  echo "   This event may not be the inaugural event. Aborting to be safe."
  exit 1
fi

if [[ -z "$sheet_name" || "$sheet_name" == "PARSE_ERROR" ]]; then
  fail "sheet_name is empty or unreadable — the sync would read the wrong/empty tab"
  exit 1
fi
pass "sheet_name resolves to a tab ('$sheet_name')"

# --- 3. Pre-sync audience count (for before/after comparison) ---
section "3. Pre-sync audience count (this event)"
pre_resp=$(curl -s -H "Authorization: Bearer $AUTH_TOKEN" "$BASE_URL/api/contacts/audience?event_ids=$EVENT_ID")
pre_total=$(echo "$pre_resp" | json_get "['data']['total']")
if [[ "$pre_total" == "PARSE_ERROR" || -z "$pre_total" ]]; then
  info "audience endpoint returned no/empty data (likely 0 rows currently in D1, or endpoint not deployed yet)"
  pre_total=0
fi
info "distinct emails currently in D1 for this event: $pre_total"

if [[ "$DRY_RUN" == "true" ]]; then
  section "Dry run — stopping before sync"
  echo -e "  ${YELLOW}⏭️  SKIP${NC} sync-sheet call (re-run without --dry-run to execute)"
  exit 0
fi

# --- 4. Trigger the sheet → D1 backfill ---
section "4. Sync sheet → D1"
sync_resp=$(curl -s -X POST -H "Authorization: Bearer $AUTH_TOKEN" "$BASE_URL/api/events/$EVENT_ID/sync-sheet")
sync_success=$(echo "$sync_resp" | json_get "['success']" || echo "")
if [[ "$sync_success" != "True" ]]; then
  fail "sync-sheet call failed"
  echo "   Response: $(echo "$sync_resp" | head -c 400)"
  exit 1
fi

total_in_sheet=$(echo "$sync_resp" | json_get "['data']['total_in_sheet']")
synced=$(echo "$sync_resp" | json_get "['data']['synced']")
inserted=$(echo "$sync_resp" | json_get "['data']['inserted']")
updated=$(echo "$sync_resp" | json_get "['data']['updated']")
skipped=$(echo "$sync_resp" | json_get "['data']['skipped']")
errors=$(echo "$sync_resp" | json_get "['data']['errors']")

info "total_in_sheet = $total_in_sheet"
info "synced         = $synced"
info "  inserted      = $inserted"
info "  updated       = $updated"
info "skipped        = $skipped (empty-api_id / empty-email rows)"
info "errors         = $errors"

if [[ "$errors" -gt 0 ]]; then
  fail "$errors rows failed to sync — check worker logs: \`cd worker && npx wrangler tail\`"
else
  pass "no row errors"
fi

if [[ "$inserted" -eq 0 && "$updated" -eq 0 && "$total_in_sheet" -gt 0 ]]; then
  fail "sheet had $total_in_sheet rows but nothing was inserted/updated — sheet_name likely points at wrong tab, or D1 already in sync"
  exit 1
fi
pass "backfill complete"

# --- 5. Verify via audience aggregation ---
section "5. Post-sync audience count"
post_resp=$(curl -s -H "Authorization: Bearer $AUTH_TOKEN" "$BASE_URL/api/contacts/audience?event_ids=$EVENT_ID")
post_total=$(echo "$post_resp" | json_get "['data']['total']")
if [[ "$post_total" == "PARSE_ERROR" || -z "$post_total" ]]; then
  post_total=0
fi
info "distinct emails now in D1 for this event: $post_total"

delta=$((post_total - pre_total))
if [[ "$delta" -gt 0 ]]; then
  pass "+$delta new emails after sync"
elif [[ "$post_total" -gt 0 && "$post_total" -eq "$pre_total" ]]; then
  pass "audience already in sync ($post_total emails, idempotent re-run — no new rows needed)"
else
  fail "no increase in audience count (pre=$pre_total, post=$post_total)"
fi

# --- Summary ---
section "Summary"
echo -e "  ${BOLD}Event:${NC}       $event_name ($EVENT_ID)"
echo -e "  ${BOLD}Sheet:${NC}       $total_in_sheet rows"
echo -e "  ${BOLD}Synced:${NC}      $synced (inserted=$inserted, updated=$updated)"
echo -e "  ${BOLD}Audience:${NC}    $pre_total → $post_total distinct emails"
echo -e "  ${BOLD}Errors:${NC}      $errors"
echo ""
if [[ "$FAIL" -eq 0 ]]; then
  echo -e "  ${GREEN}✅ Backfill succeeded.${NC} The new emails now appear in 'Export Audience (All Events)'."
else
  echo -e "  ${RED}❌ Completed with $FAIL issue(s).${NC} Review the output above."
  exit 1
fi
