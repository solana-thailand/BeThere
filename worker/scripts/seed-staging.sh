#!/usr/bin/env bash
# seed-staging.sh — Idempotently seed the STAGING D1 with a known test event,
# attendee, and deposit row so plan 005's flow-harness has deterministic state
# to drive the deposit/refund/claim flows.
#
# Usage:
#   bash worker/scripts/seed-staging.sh           # Seed default fixture (INSERT OR REPLACE)
#   bash worker/scripts/seed-staging.sh --event-id flow-deposit-20260912
#                                                   # Seed a fresh named fixture
#   bash worker/scripts/seed-staging.sh --event-id <id> --advance-past-end
#                                                   # Phase 2: move a seeded
#                                                   # fixture's event_end into
#                                                   # the past so the refund
#                                                   # flows are exercisable
#   bash worker/scripts/seed-staging.sh --clean   # Wipe the test event rows first
#   bash worker/scripts/seed-staging.sh --local   # Target local D1 (--local) instead of --remote
#
# Preconditions:
#   1. Staging D1 exists:  npx wrangler d1 create bethere-db-staging
#   2. Migrations applied: npx wrangler d1 migrations apply bethere-db-staging --remote
#   3. The database_name below matches the [env.staging] D1 binding in wrangler.toml.
#
# This script ONLY touches staging data for the selected fixture event. It never
# reads or writes production. Verify isolation with the count check at the end.

set -euo pipefail
cd "$(dirname "$0")/.."

# Staging D1 — must match the [env.staging] [[d1_databases]] database_name.
DB_NAME="bethere-db-staging"
REMOTE_FLAG="--remote"
CLEAN=0
ADVANCE=0
EVENT_ID="flow-test-event"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --clean) CLEAN=1; shift ;;
        --advance-past-end) ADVANCE=1; shift ;;
        --local) REMOTE_FLAG="--local"; shift ;;
        --event-id)
            [[ $# -ge 2 ]] || { echo "--event-id requires a value" >&2; exit 2; }
            EVENT_ID="$2"
            shift 2
            ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

# ── Phase 2: advance an existing fixture past its own event_end ──────────────
# Runs instead of a seed, not alongside one. The escrow must already exist —
# `initialize_event_escrow` rejects a past event (`EventEndInPast`, 13), so the
# fixture is born with `event_end` in the future and is moved afterwards.
#
# The KV resync is the load-bearing half. `resolve_event_or_fallback` reads KV
# first and only falls back to D1 when the KV read *errors*, so a D1-only
# update is silently masked by the cached `EventConfig` and the worker keeps
# serving the old horizon — which reads exactly like the update not working.
advance_past_end() {
    local now_ms end_ms start_ms
    now_ms=$(( $(date +%s) * 1000 ))
    end_ms=$(( now_ms - 2 * 3600 * 1000 ))
    start_ms=$(( now_ms - 5 * 3600 * 1000 ))

    local existing
    existing=$(read_sql_json "SELECT id FROM events WHERE id = '${EVENT_ID}' LIMIT 1;")
    if ! printf '%s' "$existing" | grep -q '"id"'; then
        echo "❌ ${EVENT_ID} does not exist — seed it first, then advance it." >&2
        exit 2
    fi

    echo "⏩ Advancing ${EVENT_ID} past event_end (end=${end_ms}, now-2h)..."
    run_sql "UPDATE events SET event_start_ms = ${start_ms}, event_end_ms = ${end_ms}, updated_at = datetime('now') WHERE id = '${EVENT_ID}';"

    local base="${STAGING_BASE_URL:-https://bethere-staging.solana-thailand.workers.dev}"
    echo "🔄 Resyncing KV from D1 at ${base} ..."
    if curl -fsS -X POST "${base}/api/events/reseed-kv" \
        -H "Authorization: Bearer ${STAGING_DEV_TOKEN:-dev-token}" > /dev/null; then
        echo "   ✅ KV resynced — the worker now serves the advanced horizon."
    else
        echo "   ⚠️  KV resync FAILED. The D1 row moved but the worker will keep" >&2
        echo "      serving the cached horizon. Run this before trusting a harness run:" >&2
        echo "      curl -X POST ${base}/api/events/reseed-kv -H 'Authorization: Bearer dev-token'" >&2
        exit 1
    fi

    echo ""
    echo "✅ ${EVENT_ID} advanced. The wall clock is now inside [event_end, refund_deadline)."
    exit 0
}

# Event IDs are interpolated into D1 statements below. Restrict them to the
# same slug-safe alphabet used by the event form so the operator cannot turn a
# fixture helper into a SQL injection tool.
if [[ ! "$EVENT_ID" =~ ^[a-z0-9][a-z0-9-]{2,80}$ ]]; then
    echo "Invalid --event-id '${EVENT_ID}': use 3-81 lowercase letters, digits, or hyphens." >&2
    exit 2
fi

# ── Deterministic test data ──────────────────────────────────────────────────
# The harness needs an event whose refund window is exercisable without waiting
# hours. We anchor on the seed run time:
#   event_start = now - 1h        (registration/deposit is already open)
#   event_end   = now + 4h        (deposit flow can assert the pre-end state)
#   refund_deadline_hours = 6     → refund_deadline = event_end + 6h = now + 10h
#
# `event_end` MUST be in the future at seed time and this is not negotiable:
# the on-chain program rejects `initialize_event_escrow` for a past event with
# `EscrowError::EventEndInPast` (13). An escrow cannot be created for an event
# that has already ended, so the fixture cannot be born past its own end.
#
# The refund flows need the opposite — a wall clock at or past `event_end`
# (`flows/refund_no_show_deadline.rs` documents the horizon as
# `event_end = now - 2h`). That horizon is reached by *advancing* the fixture
# after its escrow and deposit exist, not by seeding it there:
#
#   1. bash worker/scripts/seed-staging.sh --event-id <id>   (end = now + 4h)
#   2. initialize the escrow on-chain, then make the deposit
#   3. bash worker/scripts/seed-staging.sh --event-id <id> --advance-past-end
#
# Step 3 is what makes the three refund flows exercisable. Skipping it is why
# the §3.5 production gate had never gone green (.issues/084).
NOW_MS=$(( $(date +%s) * 1000 ))
EVENT_START_MS=$(( NOW_MS - 1 * 3600 * 1000 ))
EVENT_END_MS=$(( NOW_MS + 4 * 3600 * 1000 ))
REFUND_DEADLINE_HOURS=6
# Domain and escrow values use USDC's six-decimal smallest unit. A human-facing
# 10 USDC deposit must therefore be stored and passed on-chain as 10_000_000.
DEPOSIT_AMOUNT_USDC=10000000

EVENT_SLUG="$EVENT_ID"
EVENT_NAME="Flow Harness Fixture (${EVENT_ID})"

# Test attendee — checked in for the post-event refund scenario, while its
# deposit starts pending so the first harness flow can create and verify the
# on-chain deposit. The harness can use a second attendee for the no-show path.
ATTENDEE_ID="${EVENT_ID}-attendee-1"
ATTENDEE_EMAIL="${EVENT_ID}-attendee-1@staging.local"
ATTENDEE_NAME="Flow Test Attendee (checked-in)"
# Deterministic per fixture so re-seeding does not invalidate a token an
# operator already pasted into a browser. Any string works as a lookup key.
CLAIM_TOKEN="${CLAIM_TOKEN:-${EVENT_ID}-claim-token-1}"
# The Worker parses `checked_in_at` with `chrono::DateTime::parse_from_rfc3339`
# (worker/src/claim/ttl.rs) and the real check-in handlers write
# `chrono::Utc::now().to_rfc3339()`, e.g. `2026-05-24T08:08:11.774+00:00`.
#
# SQLite's `datetime('now')` renders `2026-05-24 08:08:11` — a SPACE separator
# and no UTC offset. That is not RFC 3339, so the parser rejects it and the
# Issue 071 claim-token replay window FAILS OPEN for every seeded row. The
# seeded data then silently disagrees with production, and a staging run of
# scripts/verify/claim_token_window_staging.sh cannot tell a working window
# from a disabled one. Emit the production format instead.
CHECKED_IN_AT_SQL="strftime('%Y-%m-%dT%H:%M:%f+00:00','now')"

run_sql () {
    # $1 = SQL string. Executes against staging D1.
    npx wrangler d1 execute "$DB_NAME" --env staging $REMOTE_FLAG --command "$1" >/dev/null
}

read_sql_json () {
    # $1 = SELECT statement. Never suppress output: the caller uses the JSON
    # to refuse destructive reseeding of an initialized escrow fixture.
    npx wrangler d1 execute "$DB_NAME" --env staging $REMOTE_FLAG --json --command "$1"
}

# Phase 2 runs instead of a seed — dispatch once the SQL helpers above exist.
# It deliberately runs *before* the escrow guard below: advancing an event whose
# escrow is already initialized is exactly the supported case, whereas reseeding
# one is what the guard refuses.
if [ "$ADVANCE" -eq 1 ]; then
    advance_past_end
fi

echo "🌱 Seeding staging D1 ($DB_NAME, $REMOTE_FLAG)..."

# An EventEscrow records immutable on-chain event timing and PDA data. Replacing
# its D1 event row would silently detach the Worker from that account and make
# future deposits/refunds unsafe to diagnose. This check runs before --clean so
# an accidental fixture refresh cannot delete a live staging escrow lifecycle.
EXISTING_EVENT=$(read_sql_json "SELECT escrow_status FROM events WHERE id = '${EVENT_ID}' LIMIT 1;")
if printf '%s' "$EXISTING_EVENT" | grep -Eq '"escrow_status"[[:space:]]*:[[:space:]]*"(initialized|deactivated|closed)"'; then
    echo "❌ Refusing to reseed ${EVENT_ID}: its escrow is already initialized/deactivated/closed." >&2
    echo "   Create a new fixture event instead; never replace an on-chain escrow row." >&2
    exit 1
fi

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
    1, ${DEPOSIT_AMOUNT_USDC}, 0,
    'none', ${REFUND_DEADLINE_HOURS}, 5,
    'public', 'Staging test event for flow harness', 'Bangkok (staging)',
    datetime('now'), datetime('now')
);"

# ── Test attendee (checked-in) ────────────────────────────────────────────────
echo "📝 Upserting attendee ${ATTENDEE_ID} (checked_in, claim_token=${CLAIM_TOKEN})..."
run_sql "INSERT OR REPLACE INTO attendees (
    id, event_id, email, name, approval_status, participation_type,
    checked_in_at, claim_token, deposit_status, deposit_amount_usdc
) VALUES (
    '${ATTENDEE_ID}', '${EVENT_ID}', '${ATTENDEE_EMAIL}', '${ATTENDEE_NAME}',
    'approved', 'in_person',
    ${CHECKED_IN_AT_SQL}, '${CLAIM_TOKEN}', 'pending', ${DEPOSIT_AMOUNT_USDC}
);"

# ── Deposit status row (mirrors deposit_statuses table) ───────────────────────
echo "📝 Upserting deposit_status for ${ATTENDEE_ID}..."
run_sql "INSERT OR REPLACE INTO deposit_statuses (
    attendee_id, event_id, method, amount, currency,
    verified, deposited_at, wallet_address, deposit_order, refundable
) VALUES (
    '${ATTENDEE_ID}', '${EVENT_ID}', 'usdc', ${DEPOSIT_AMOUNT_USDC}, 'USDC',
    0, datetime('now'), '', 1, 1
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
echo "   Attendee:     ${ATTENDEE_ID} (checked-in, usdc deposit pending)"
echo "   Refund window: checked-in → anytime after event_end; no-show → before refund_deadline."
echo ""
echo "Next: open the staging event in Manage Events → Edit → Escrow Management."
echo "      Initialize its Devnet escrow with the organizer wallet, then run the focused deposit harness."
