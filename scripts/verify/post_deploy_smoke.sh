#!/usr/bin/env bash
# Post-deploy smoke test — READS AND WRITES.
#
# Why this exists. On 2026-09-23 a production deploy passed every check in
# `docs/deploy_20260923_runbook.md` — health 200, a ticket payload, Content-Type
# headers — while **every THB slip upload in production returned 500**
# (`.issues/138`). A CHECK constraint added by one of that deploy's own
# migrations rejected what the application writes. Reads cannot see that, and
# nothing in the deploy path wrote anything.
#
# The outage lasted ~10 hours and was found by the owner, not by any gate. This
# is the gate.
#
# It exercises the paths that actually move money:
#
#   register → upload a slip → verify the slip → read the roster badge
#
# Every one of those is an INSERT or UPDATE. The slip upload is the centrepiece
# because it is the one that broke.
#
# Usage:
#   bash scripts/verify/post_deploy_smoke.sh                 # staging (default)
#   bash scripts/verify/post_deploy_smoke.sh --url <base>    # explicit target
#   bash scripts/verify/post_deploy_smoke.sh --url <prod> --i-know-this-writes
#
# Exit: 0 all green, 1 any failure.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

BASE_URL="https://bethere-staging.solana-thailand.workers.dev"
ALLOW_WRITES_ANYWHERE=false
TOKEN="${SMOKE_TOKEN:-dev-token}"
FIXTURE_ID="smoke-$(date +%s)"
CLEANED=false

usage() { sed -n '3,30p' "${BASH_SOURCE[0]}" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --url) [ $# -ge 2 ] || { echo "❌ --url needs a value" >&2; usage; }; BASE_URL="$2"; shift 2 ;;
    --i-know-this-writes) ALLOW_WRITES_ANYWHERE=true; shift ;;
    -h|--help) usage ;;
    *) echo "❌ Unknown argument: $1" >&2; usage ;;
  esac
done

# Fail closed on production. This script creates a real event, a real attendee
# and a real deposit row. On staging that is the point; on production it is
# litter in the table the organizer reads at the door.
if [ "$ALLOW_WRITES_ANYWHERE" = false ] && ! echo "$BASE_URL" | grep -q "staging\|localhost\|127.0.0.1"; then
  echo "❌ Refusing to write to a non-staging target: $BASE_URL" >&2
  echo "   This test REGISTERS an attendee and UPLOADS a slip. Against production" >&2
  echo "   that leaves rows in the roster and the deposit queue." >&2
  echo "   Run it against staging, or pass --i-know-this-writes and clean up." >&2
  exit 1
fi

pass() { printf '  \033[0;32m✅ PASS\033[0m %s\n' "$1"; }
fail() { printf '  \033[0;31m❌ FAIL\033[0m %s\n' "$1"; FAILED=1; }
info() { printf '  \033[0;36mℹ️ \033[0m %s\n' "$1"; }
step() { printf '\n\033[1m━━━ %s ━━━\033[0m\n' "$1"; }
FAILED=0

# HTTP status goes through a FILE, not a variable.
#
# The obvious `LAST_STATUS=...` inside this function is wrong when the caller
# writes `resp=$(api ...)`: command substitution runs a subshell, the
# assignment dies with it, and the caller reads the PREVIOUS call's status.
# That bug was in the first version of this script and made it report
# "slip upload → 200" for a call that actually returned 401 — a false green
# inside the gate written to prevent false greens. Caught only by running it
# against a deliberately broken build.
STATUS_FILE="$(mktemp)"
# shellcheck disable=SC2329  # invoked by the EXIT trap
cleanup_status_file() { rm -f "$STATUS_FILE"; }
trap cleanup_status_file EXIT

api() {
  # api <method> <path> [body] → body on stdout; status via last_status
  local method="$1" path="$2" body="${3:-}"
  local out
  if [ -n "$body" ]; then
    out=$(curl -s -w '\n%{http_code}' -X "$method" "$BASE_URL$path" \
      -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d "$body")
  else
    out=$(curl -s -w '\n%{http_code}' -X "$method" "$BASE_URL$path" \
      -H "Authorization: Bearer $TOKEN")
  fi
  printf '%s' "${out##*$'\n'}" > "$STATUS_FILE"
  printf '%s' "${out%$'\n'*}"
}

last_status() { cat "$STATUS_FILE" 2>/dev/null || echo "000"; }

# shellcheck disable=SC2329  # invoked by the EXIT trap below
cleanup() {
  [ "$CLEANED" = true ] && return 0
  CLEANED=true
  step "Cleanup"
  # Hard delete refuses unless the event is archived first.
  api PUT "/api/events/$FIXTURE_ID" '{"status":"archived"}' >/dev/null 2>&1 || true
  api DELETE "/api/events/$FIXTURE_ID/delete" >/dev/null 2>&1 || true
  if [ "$(last_status)" = "200" ]; then
    pass "fixture event $FIXTURE_ID deleted"
  else
    # Loud, not silent: a fixture left behind shows up on the organizer's
    # screen, and the next run's "already exists" failure would be confusing.
    printf '  \033[1;33m⚠️  WARN\033[0m fixture %s may remain (HTTP %s) — delete it by hand\n' \
      "$FIXTURE_ID" "$(last_status)"
  fi
}
trap cleanup EXIT

echo "Target: $BASE_URL"
echo "Fixture: $FIXTURE_ID"

# ── 1. Reads (the checks that passed while production was broken) ───────────
step "1. Reads"
api GET "/api/health" >/dev/null
if [ "$(last_status)" = "200" ]; then pass "/api/health → 200"; else fail "/api/health → $(last_status)"; fi

ct=$(curl -sI "$BASE_URL/" | tr -d '\r' | awk -F': ' 'tolower($1)=="content-type"{print $2}')
case "$ct" in
  text/html*) pass "/ → $ct" ;;
  *) fail "/ → ${ct:-<none>} (expected text/html)" ;;
esac

# ── 2. Writes — the part that was missing ───────────────────────────────────
step "2. Create a disposable in-person event with deposits on"
now_ms=$(( $(date +%s) * 1000 ))
api POST "/api/events" "$(cat <<JSON
{"name":"Post-deploy smoke","slug":"$FIXTURE_ID","tagline":"automated smoke test",
 "link":"","event_start_ms":$(( now_ms + 86400000 )),"event_end_ms":$(( now_ms + 172800000 )),
 "time_tba":false,"sheet_id":"smoke-no-sheet","event_format":"in_person",
 "deposit_enabled":true,"deposit_amount_thb":500}
JSON
)" >/dev/null
if [ "$(last_status)" = "200" ]; then pass "event created"; else fail "create event → $(last_status)"; fi

step "3. Upload a THB slip (admin path) — what 500'd on 2026-09-23"
# A 1x1 PNG. The bytes do not matter; the INSERT does. This is the exact call
# that failed for ~10 hours because `deposit_source` bound "" against a CHECK.
PNG='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='
ATT_ID=$(api GET "/api/attendees?event_id=$FIXTURE_ID" \
  | python3 -c "import sys,json;d=json.load(sys.stdin).get('data',{});a=(d.get('attendees') or [{}])[0];print(a.get('api_id',''))" 2>/dev/null || echo "")

if [ -z "$ATT_ID" ]; then
  info "no attendee on the fixture event — seeding one via walk-in"
  api POST "/api/walkin/register" \
    "{\"event_id\":\"$FIXTURE_ID\",\"name\":\"Smoke Tester\",\"email\":\"smoke-$FIXTURE_ID@example.com\"}" >/dev/null
  ATT_ID=$(api GET "/api/attendees?event_id=$FIXTURE_ID" \
    | python3 -c "import sys,json;d=json.load(sys.stdin).get('data',{});a=(d.get('attendees') or [{}])[0];print(a.get('api_id',''))" 2>/dev/null || echo "")
fi

if [ -z "$ATT_ID" ]; then
  fail "could not obtain an attendee to upload a slip for — the write path is UNTESTED"
else
  info "attendee $ATT_ID"
  resp=$(api POST "/api/deposit/thb/admin-upload" "$(cat <<JSON
{"event_id":"$FIXTURE_ID","attendee_id":"$ATT_ID","slip_url":"$PNG",
 "bank_account":"0000000000","bank_name":"SMOKE","account_name":"Smoke Tester"}
JSON
)")
  if [ "$(last_status)" = "200" ]; then
    pass "slip upload → 200 (the .issues/138 regression would fail here)"
  else
    fail "slip upload → $(last_status): $(printf '%s' "$resp" | head -c 160)"
  fi
fi

step "4. Roster reflects the write"
roster=$(api GET "/api/attendees?event_id=$FIXTURE_ID")
if [ "$(last_status)" = "200" ]; then
  badge=$(printf '%s' "$roster" | python3 -c "
import sys,json
d=json.load(sys.stdin).get('data',{})
a=(d.get('attendees') or [{}])[0]
print('thb_source=%r thb_verified=%r' % (a.get('thb_source'), a.get('thb_verified')))" 2>/dev/null || echo "unparsed")
  info "$badge"
  case "$badge" in
    *"thb_source='cash'"*) pass "roster sees the deposit (.issues/137 annotation live)" ;;
    *) fail "roster does not reflect the uploaded slip — $badge" ;;
  esac
else
  fail "roster read → $(last_status)"
fi

step "Result"
if [ "$FAILED" -eq 0 ]; then
  echo "  ✅ Writes work. Safe to promote."
  exit 0
fi
echo "  ❌ A write path is broken. DO NOT promote this build."
exit 1
