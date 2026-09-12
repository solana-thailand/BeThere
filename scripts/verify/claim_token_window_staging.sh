#!/usr/bin/env bash
#
# Issue 071 — end-to-end validation of the claim-token replay window, in BOTH
# directions, against a deployed staging Worker.
#
# The unit tests in `worker/src/claim/ttl.rs` prove the policy logic. They
# cannot prove that the deployed Worker reads CLAIM_TOKEN_TTL_SECS, that the
# var survived the non-inheritable `[env.staging.vars]` trap, or that the D1
# row's stored timestamp format is one the parser accepts. A green unit suite
# with an inert deployment looks exactly like a working one, so this script
# exercises the real HTTP path.
#
# It asserts three things in order:
#   1. a fresh check-in resolves — the window does not over-block
#   2. the same token backdated past the TTL becomes INDISTINGUISHABLE from a
#      token that never existed — the window applies, and leaks no oracle
#   3. restoring the timestamp resolves again — the change was the cause
#
# Step 2 is the one that matters. Without it a completely disabled window still
# passes step 1, which is the failure mode this script exists to catch.
#
# Step 2 deliberately compares against a control request for a random unknown
# token rather than asserting a hard-coded 404. The security property Issue 071
# claims is that an expired token is not distinguishable from a typo, and that
# is a statement about two responses being equal, not about a status code. The
# codes legitimately differ per environment: production resolves a D1 miss
# through the Google Sheets fallback and 404s, while staging has an empty
# PLATFORM_SHEET_ID, so the same miss surfaces as a 500. Asserting 404 fails on
# staging for a reason that has nothing to do with the window - and asserting
# only the status code would miss a body that named the expiry.
#
# STAGING ONLY. It mutates `checked_in_at` for one attendee and restores it on
# exit, including on failure and on Ctrl-C. Never point it at production.
#
# Usage: bash scripts/verify/claim_token_window_staging.sh
set -euo pipefail

BASE_URL="${BASE_URL:-https://bethere-staging.solana-thailand.workers.dev}"
DB_NAME="${DB_NAME:-bethere-db-staging}"
WRANGLER_DIR="${WRANGLER_DIR:-worker}"

PASS=0
FAIL=0
ATTENDEE_ID=""
ORIGINAL_CHECKED_IN=""

pass() { PASS=$((PASS + 1)); echo "PASS  $1"; }
fail() { FAIL=$((FAIL + 1)); echo "FAIL  $1"; }
info() { echo "      $1"; }

# Run a statement against staging D1 and print the first result row as JSON.
d1() {
    (cd "$WRANGLER_DIR" && npx wrangler d1 execute "$DB_NAME" --remote \
        --env staging --json --command "$1" 2>/dev/null) | jq -c '.[0].results[0] // {}'
}

# HTTP status of a claim lookup. Non-mutating: GET never mints.
claim_status() {
    curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/api/claim/$1"
}

# Status and body together, so an oracle hiding in the body is caught too.
claim_response() {
    local body status
    body=$(curl -s -w '\n%{http_code}' "$BASE_URL/api/claim/$1")
    status=$(echo "$body" | tail -n1)
    echo "$status|$(echo "$body" | sed '$d')"
}

# shellcheck disable=SC2317,SC2329  # invoked via trap, not statically reachable
restore() {
    case "$ATTENDEE_ID" in
        "") return ;;
    esac
    case "$ORIGINAL_CHECKED_IN" in
        "") return ;;
    esac
    info "restoring checked_in_at for the test attendee"
    d1 "UPDATE attendees SET checked_in_at = '$ORIGINAL_CHECKED_IN' WHERE id = '$ATTENDEE_ID';" \
        > /dev/null || echo "WARNING: restore failed — fix $ATTENDEE_ID by hand"
}
trap restore EXIT INT TERM

echo "Issue 071 claim-token window — staging validation"
echo "base: $BASE_URL"
echo

# The window is anchored on check-in, so the subject must be checked in.
row=$(d1 "SELECT id, claim_token, checked_in_at FROM attendees
          WHERE checked_in_at IS NOT NULL AND TRIM(checked_in_at) <> ''
            AND claim_token IS NOT NULL AND TRIM(claim_token) <> ''
          ORDER BY checked_in_at DESC LIMIT 1;")

ATTENDEE_ID=$(echo "$row" | jq -r '.id // empty')
TOKEN=$(echo "$row" | jq -r '.claim_token // empty')
ORIGINAL_CHECKED_IN=$(echo "$row" | jq -r '.checked_in_at // empty')

if [ -z "$ATTENDEE_ID" ] || [ -z "$TOKEN" ]; then
    fail "no checked-in attendee with a claim token in $DB_NAME — nothing to validate"
    exit 1
fi
info "subject: attendee $ATTENDEE_ID, checked in at $ORIGINAL_CHECKED_IN"
echo

# 1 — inside the window.
code=$(claim_status "$TOKEN")
case "$code" in
    200) pass "fresh check-in resolves (HTTP $code)" ;;
    *)   fail "fresh check-in should resolve but returned HTTP $code"
         info "the window may be over-blocking, or staging is down — stopping"
         exit 1 ;;
esac

# 2 — outside the window. 400 days clears any plausible TTL, including the
# 180-day production migration window, so this stays valid if staging is ever
# aligned to production.
backdated="$(date -u -v-400d '+%Y-%m-%dT%H:%M:%S.000+00:00' 2>/dev/null \
    || date -u -d '400 days ago' '+%Y-%m-%dT%H:%M:%S.000+00:00')"
info "backdating checked_in_at to $backdated"
d1 "UPDATE attendees SET checked_in_at = '$backdated' WHERE id = '$ATTENDEE_ID';" > /dev/null

# Control: a token that never existed. Whatever this environment does for an
# unknown token is exactly what an expired token must do.
control=$(claim_response "unknown-control-$RANDOM$RANDOM")
expired=$(claim_response "$TOKEN")
control_code=${control%%|*}
expired_code=${expired%%|*}
info "unknown-token control responds HTTP $control_code"

case "$expired_code" in
    200) fail "backdated check-in still resolves (HTTP $expired_code) — THE WINDOW IS NOT APPLIED"
         info "check CLAIM_TOKEN_TTL_SECS in [env.staging.vars]; wrangler vars do not inherit"
         info "also check the stored timestamp parses as RFC 3339 — the parser fails OPEN"
         info "a seed writing SQLite datetime('now') reproduces exactly this" ;;
    *)
        case "$expired" in
            "$control") pass "expired token is byte-identical to an unknown token (HTTP $expired_code) — no oracle" ;;
            *) fail "expired token is DISTINGUISHABLE from an unknown token"
               info "expired: $expired_code | control: $control_code"
               info "an attacker can tell a real-but-expired token from a typo"
               info "expired body: $(echo "${expired#*|}" | head -c 200)"
               info "control body: $(echo "${control#*|}" | head -c 200)" ;;
        esac ;;
esac

# 3 — restoring must undo it, proving the timestamp was the cause and not some
# unrelated failure that happened to 404.
d1 "UPDATE attendees SET checked_in_at = '$ORIGINAL_CHECKED_IN' WHERE id = '$ATTENDEE_ID';" > /dev/null
code=$(claim_status "$TOKEN")
case "$code" in
    200) pass "restoring the timestamp resolves again (HTTP $code)" ;;
    *)   fail "restore did not bring the token back (HTTP $code) — check $ATTENDEE_ID" ;;
esac

echo
echo "passed: $PASS   failed: $FAIL"
[ "$FAIL" -eq 0 ]
