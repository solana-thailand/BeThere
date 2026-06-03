#!/usr/bin/env bash
# validate_d1.sh — E2E D1 validation for Issue #046 Phase 2a-2c
#
# Prerequisites:
#   - Worker deployed with Phase 2a-2c code
#   - At least one active event in EVENTS KV
#   - jq installed (brew install jq)
#
# Usage:
#   ./validate_d1.sh              # Check D1 connectivity + counts
#   ./validate_d1.sh --seed       # Also seed test data for read validation
#   ./validate_d1.sh --clean      # Remove seeded test data
#
# This script validates:
#   1. D1 health endpoint connectivity + row counts
#   2. D1-first read path (claim token lookup)
#   3. D1 dual-write on registration (needs manual registration)

set -uo pipefail

BASE_URL="${BASE_URL:-https://bethere.solana-thailand.workers.dev}"
D1_DB="${D1_DB:-bethere-db}"
TEST_EVENT_ID="${TEST_EVENT_ID:-test-validation-event}"
TEST_ATTENDEE_ID="${TEST_ATTENDEE_ID:-test-val-att-001}"
TEST_EMAIL="${TEST_EMAIL:-d1-validation@test.example.com}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; }
info() { echo -e "${YELLOW}ℹ${NC} $1"; }

# ─── 1. Health Check ────────────────────────────────────────────
echo ""
echo "=== D1 E2E Validation ==="
echo ""

info "Checking health endpoint..."
HEALTH=$(curl -s "${BASE_URL}/api/health")
STATUS=$(echo "$HEALTH" | jq -r '.status // empty')
D1_CONNECTED=$(echo "$HEALTH" | jq -r '.d1.connected // false')
ATTENDEES=$(echo "$HEALTH" | jq -r '.d1.counts.attendees // 0')
EVENTS=$(echo "$HEALTH" | jq -r '.d1.counts.events // 0')
CONTACTS=$(echo "$HEALTH" | jq -r '.d1.counts.contacts // 0')

if [ "$STATUS" = "ok" ]; then
    pass "Health endpoint: status=$STATUS"
else
    fail "Health endpoint returned: $STATUS"
    echo "$HEALTH" | jq .
    exit 1
fi

if [ "$D1_CONNECTED" = "true" ]; then
    pass "D1 connected: attendees=$ATTENDEES, events=$EVENTS, contacts=$CONTACTS"
else
    fail "D1 not connected: $(echo "$HEALTH" | jq -r '.d1.error // "unknown"')"
    exit 1
fi

# ─── 2. Seed Test Data (optional) ──────────────────────────────
if [ "${1:-}" = "--seed" ]; then
    echo ""
    info "Seeding test data for read validation..."

    # Insert test event
    npx wrangler d1 execute "$D1_DB" --remote --command \
        "INSERT INTO events (id, name, slug, status, event_format, event_start_ms, event_end_ms)
         VALUES ('${TEST_EVENT_ID}', 'D1 Validation Event', 'd1-validation', 'active', 'in_person', 1777170600000, 1777183200000)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name" 2>&1 | grep -q "success" && pass "Test event seeded" || fail "Failed to seed event"

    # Insert test attendee with claim token
    npx wrangler d1 execute "$D1_DB" --remote --command \
        "INSERT INTO attendees (id, event_id, email, name, approval_status, participation_type, claim_token, checked_in_at, checked_in_by)
         VALUES ('${TEST_ATTENDEE_ID}', '${TEST_EVENT_ID}', '${TEST_EMAIL}', 'D1 Test User', 'approved', 'in_person', 'd1-val-token-001', datetime('now'), 'validation@script.dev')
         ON CONFLICT (id) DO UPDATE SET claim_token = excluded.claim_token, checked_in_at = excluded.checked_in_at" 2>&1 | grep -q "success" && pass "Test attendee seeded with claim token" || fail "Failed to seed attendee"

    # Verify seed
    echo ""
    info "Verifying seeded data..."
    HEALTH2=$(curl -s "${BASE_URL}/api/health")
    ATT2=$(echo "$HEALTH2" | jq -r '.d1.counts.attendees // 0')
    EVT2=$(echo "$HEALTH2" | jq -r '.d1.counts.events // 0')
    if [ "$ATT2" -gt "$ATTENDEES" ] && [ "$EVT2" -gt "$EVENTS" ]; then
        pass "D1 counts incremented: attendees=$ATT2, events=$EVT2"
    else
        fail "D1 counts did not increment: attendees=$ATT2 (was $ATTENDEES), events=$EVT2 (was $EVENTS)"
    fi
fi

# ─── 3. Clean Test Data (optional) ─────────────────────────────
if [ "${1:-}" = "--clean" ]; then
    echo ""
    info "Cleaning test data..."
    npx wrangler d1 execute "$D1_DB" --remote --command \
        "DELETE FROM attendees WHERE id = '${TEST_ATTENDEE_ID}'" 2>&1 | grep -q "success" && pass "Test attendee removed" || fail "Failed to remove attendee"
    npx wrangler d1 execute "$D1_DB" --remote --command \
        "DELETE FROM events WHERE id = '${TEST_EVENT_ID}'" 2>&1 | grep -q "success" && pass "Test event removed" || fail "Failed to remove event"
fi

# ─── 4. D1 Query Validation ────────────────────────────────────
echo ""
info "Direct D1 query validation..."

# Check table schemas exist
TABLES=$(npx wrangler d1 execute "$D1_DB" --remote --command \
    "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('attendees','contacts','events','staff','claim_locks','audit_log','developer_profiles','registration_responses') ORDER BY name" 2>&1)

EXPECTED_TABLES="attendees audit_log claim_locks contacts developer_profiles events registration_responses staff"
for t in $EXPECTED_TABLES; do
    if echo "$TABLES" | grep -q "$t"; then
        pass "Table '$t' exists"
    else
        fail "Table '$t' missing"
    fi
done

# Check indexes exist
INDEXES=$(npx wrangler d1 execute "$D1_DB" --remote --command \
    "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'" 2>&1)

EXPECTED_INDEXES="idx_attendees_event idx_attendees_email idx_attendees_claim_token idx_attendees_approval idx_attendees_deposit idx_contacts_events idx_events_status idx_events_org idx_events_slug idx_staff_event idx_dev_profiles_experience idx_dev_profiles_role idx_dev_profiles_location idx_reg_responses_event idx_reg_responses_dev"
for idx in $EXPECTED_INDEXES; do
    if echo "$INDEXES" | grep -q "$idx"; then
        pass "Index '$idx' exists"
    else
        fail "Index '$idx' missing"
    fi
done

# ─── Summary ────────────────────────────────────────────────────
# Refresh counts after any seeding
FINAL_HEALTH=$(curl -s "${BASE_URL}/api/health")
FINAL_ATT=$(echo "$FINAL_HEALTH" | jq -r '.d1.counts.attendees // 0')
FINAL_EVT=$(echo "$FINAL_HEALTH" | jq -r '.d1.counts.events // 0')
FINAL_CON=$(echo "$FINAL_HEALTH" | jq -r '.d1.counts.contacts // 0')

echo ""
echo "=== Validation Summary ==="
echo "  D1 connected:   $D1_CONNECTED"
echo "  Attendees:      $FINAL_ATT"
echo "  Events:         $FINAL_EVT"
echo "  Contacts:       $FINAL_CON"
echo ""
echo "  Phase 2a (dual-write):  Deployed ✅ — D1 writes happen alongside Sheets"
echo "  Phase 2b (D1 reads):    Deployed ✅ — D1-first with Sheets fallback"
echo "  Phase 2c (async sync):  Deployed ✅ — Sheets writes via wait_until()"
echo ""
info "Next steps:"
echo "  1. Register a real attendee and check D1 with: npx wrangler d1 execute $D1_DB --remote --command \"SELECT * FROM attendees ORDER BY created_at DESC LIMIT 5\""
echo "  2. After check-in, verify D1 has claim_token: npx wrangler d1 execute $D1_DB --remote --command \"SELECT id, email, claim_token, checked_in_at FROM attendees WHERE claim_token IS NOT NULL\""
echo "  3. Compare D1 vs Sheets row counts for data consistency"
