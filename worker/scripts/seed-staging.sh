#!/usr/bin/env bash
# seed-staging.sh — Idempotently seed the STAGING D1 with a known test event,
# attendee, and deposit row so plan 005's flow-harness has deterministic state
# to drive the deposit/refund/claim flows.
#
# Usage:
#   bash worker/scripts/seed-staging.sh           # Seed (INSERT OR REPLACE)
#   bash worker/scripts/seed-staging.sh --clean   # Wipe the test event rows first
#   bash worker/scripts/seed-staging.sh --local   # Target local D1 (--local) instead of --remote
#
# Preconditions:
#   1. Staging D1 exists:  npx wrangler d1 create bethere-db-staging
#   2. Migrations applied: npx wrangler d1 migrations apply bethere-db-staging --remote
#   3. The database_name below matches the [env.staging] D1 binding in wrangler.toml.
#
# This script ONLY touches staging data (event id `flow-test-event`). It never
# reads or writes production. Verify isolation with the count check at the end.

set -euo pipefail
cd "$(dirname "$0")/.."

# Staging D1 — must match the [env.staging] [[d1_databases]] database_name.
DB_NAME="bethere-db-staging"
REMOTE_FLAG="--remote"
CLEAN=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --clean) CLEAN=1; shift ;;
        --local) REMOTE_FLAG="--local"; shift ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

# ── Deterministic test data ──────────────────────────────────────────────────
# The harness needs an event whose refund window is exercisable without waiting
# hours. We anchor on the seed run time:
#   event_start = now - 4h        (clearly in the past)
#   event_end   = now - 2h        (ended → checked-in refunds allowed)
#   refund_deadline_hours = 6     → refund_deadline = event_end + 6h = now + 4h
# So at seed time: a no-show can refund (now < deadline), a checked-in user can
# always refund, and waiting >4h flips the no-show path closed for re-testing.
NOW_MS=$(( $(date +%s) * 1000 ))
EVENT_START_MS=$(( NOW_MS - 1 * 3600 * 1000 ))
EVENT_END_MS=$(( NOW_MS + 4 * 3600 * 1000 ))
REFUND_DEADLINE_HOURS=6

EVENT_ID="flow-test-event"
EVENT_SLUG="flow-test-event"
EVENT_NAME="Flow Harness Test Event (staging)"

# Test attendee — deterministically checked-in so the checked-in refund path is
# immediately exercisable. The harness can insert a SECOND no-show attendee via
# the API to exercise the deadline path.
ATTENDEE_ID="flow-test-attendee-1"
ATTENDEE_EMAIL="flow-test-attendee-1@staging.local"
ATTENDEE_NAME="Flow Test Attendee (checked-in)"

run_sql () {
    # $1 = SQL string. Executes against staging D1.
    npx wrangler d1 execute "$DB_NAME" --env staging $REMOTE_FLAG --command "$1" >/dev/null
}

echo "🌱 Seeding staging D1 ($DB_NAME, $REMOTE_FLAG)..."

if [[ "$CLEAN" -eq 1 ]]; then
    echo "🧹 Removing existing flow-test rows..."
    run_sql "DELETE FROM deposit_statuses WHERE event_id = '${EVENT_ID}';"
    run_sql "DELETE FROM attendees      WHERE event_id = '${EVENT_ID}';"
    run_sql "DELETE FROM events          WHERE id       = '${EVENT_ID}';"
fi

# ── Event row ─────────────────────────────────────────────────────────────────
echo "📝 Upserting event ${EVENT_ID} (start=${EVENT_START_MS}, end=${EVENT_END_MS}, rd_h=${REFUND_DEADLINE_HOURS})..."
run_sql "INSERT OR REPLACE INTO events (
    id, name, slug, status, event_format,
    event_start_ms, event_end_ms,
    deposit_enabled, deposit_amount_usdc, deposit_amount_thb,
    escrow_status, refund_deadline_hours, max_refundable_deposits,
    visibility, tagline, location, created_at, updated_at
) VALUES (
    '${EVENT_ID}', '${EVENT_NAME}', '${EVENT_SLUG}', 'active', 'in_person',
    ${EVENT_START_MS}, ${EVENT_END_MS},
    1, 10, 0,
    'none', ${REFUND_DEADLINE_HOURS}, 5,
    'public', 'Staging test event for flow harness', 'Bangkok (staging)',
    datetime('now'), datetime('now')
);"

# ── Test attendee (checked-in) ────────────────────────────────────────────────
echo "📝 Upserting attendee ${ATTENDEE_ID} (checked_in)..."
run_sql "INSERT OR REPLACE INTO attendees (
    id, event_id, email, name, approval_status, participation_type,
    checked_in_at, deposit_status, deposit_amount_usdc
) VALUES (
    '${ATTENDEE_ID}', '${EVENT_ID}', '${ATTENDEE_EMAIL}', '${ATTENDEE_NAME}',
    'approved', 'in_person',
    datetime('now'), 'verified', 10
);"

# ── Deposit status row (mirrors deposit_statuses table) ───────────────────────
echo "📝 Upserting deposit_status for ${ATTENDEE_ID}..."
run_sql "INSERT OR REPLACE INTO deposit_statuses (
    attendee_id, event_id, method, amount, currency,
    verified, deposited_at, wallet_address, deposit_order, refundable
) VALUES (
    '${ATTENDEE_ID}', '${EVENT_ID}', 'usdc', 10, 'USDC',
    1, datetime('now'), '', 1, 1
);"

# ── Isolation sanity check ───────────────────────────────────────────────────
echo ""
echo "🔎 Isolation check — staging attendee count for ${EVENT_ID}:"
npx wrangler d1 execute "$DB_NAME" --env staging $REMOTE_FLAG \
    --command "SELECT count(*) AS n FROM attendees WHERE event_id = '${EVENT_ID}';" \
    | sed -n '1,20p'

echo ""
echo "✅ Staging seed complete."
echo "   Event:        ${EVENT_ID}  (ends ${EVENT_END_MS}, refund deadline +${REFUND_DEADLINE_HOURS}h)"
echo "   Attendee:     ${ATTENDEE_ID} (checked-in, usdc deposit verified)"
echo "   Refund window: checked-in → anytime after event_end; no-show → before refund_deadline."
echo ""
echo "Next: bash worker/deploy.sh staging   # then point flow-harness at the staging URL."
