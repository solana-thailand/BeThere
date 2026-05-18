#!/usr/bin/env bash
# ============================================================================
# BeThere Escrow Lifecycle Test — Deactivate → Claim → Close on Devnet
# ============================================================================
# Tests the full lifecycle: create_vault_ata → create_event → deactivate → close
# (No deposit/refund — just the admin lifecycle)
#
# Prerequisites:
#   - `cd worker && npx wrangler dev --port 8787` running
#   - DEV_MODE=1 in worker/.dev.vars
#   - solana CLI configured for devnet with funded keypair
#
# Usage:
#   bash scripts/e2e/test_lifecycle.sh
#   EVENT_ID=myevent bash scripts/e2e/test_lifecycle.sh --reuse
# ============================================================================

set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:8787}"
ORGANIZER_WALLET=$(solana address --url devnet 2>/dev/null)
EVENT_ID="${EVENT_ID:-lifecycle-$(date +%s)}"
RPC_URL="https://api.devnet.solana.com"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

pass() { echo -e "  ${GREEN}✅ PASS${NC} $1"; }
fail() { echo -e "  ${RED}❌ FAIL${NC} $1"; }
info() { echo -e "  ${CYAN}ℹ️  INFO${NC} $1"; }
section() { echo -e "\n${CYAN}━━━ $1 ━━━${NC}"; }

sign_and_submit() {
    local tx_b64="$1"
    python3 "$(dirname "$0")/sign_and_submit.py" "$tx_b64" "$(cat ~/.config/solana/id.json)" "$RPC_URL"
}

REUSE=false
for arg in "$@"; do
    case "$arg" in
        --reuse) REUSE=true ;;
    esac
done

echo ""
echo -e "${BOLD}🧪 BeThere Escrow Lifecycle Test${NC}"
echo "   BASE_URL:    $BASE_URL"
echo "   EVENT_ID:    $EVENT_ID"
echo "   ORGANIZER:   $ORGANIZER_WALLET"
echo ""

# --- Health check ---
HEALTH=$(curl -sf "$BASE_URL/api/health" 2>&1 || echo "")
if echo "$HEALTH" | grep -q '"ok"'; then
    pass "Worker health check"
else
    fail "Worker not healthy — is wrangler dev running?"
    exit 1
fi

# --- Step 1: Create Event ---
section "Step 1: Create Event"

NOW_S=$(date +%s)
EVENT_END_MS=$(( (NOW_S + 86400) * 1000 ))
EVENT_START_MS=$(( NOW_S * 1000 ))

EVENT_RESP=$(curl -s -X POST "$BASE_URL/api/events" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{
        \"name\": \"Lifecycle Test\",
        \"slug\": \"$EVENT_ID\",
        \"tagline\": \"Automated lifecycle test\",
        \"link\": \"https://example.com\",
        \"sheet_id\": \"test\",
        \"event_start_ms\": $EVENT_START_MS,
        \"event_end_ms\": $EVENT_END_MS,
        \"status\": \"active\",
        \"deposit_enabled\": true,
        \"deposit_amount_usdc\": 1000000,
        \"deposit_amount_thb\": 100,
        \"promptpay_id\": \"0812345678\",
        \"organizer_wallet\": \"$ORGANIZER_WALLET\",
        \"refund_deadline_hours\": 168
    }")

EVENT_SUCCESS=$(echo "$EVENT_RESP" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
if [ "$EVENT_SUCCESS" = "true" ]; then
    EVENT_ID=$(echo "$EVENT_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['id'])" 2>/dev/null)
    pass "Event created: $EVENT_ID"
else
    ERR=$(echo "$EVENT_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "$EVENT_RESP")
    fail "Event creation: $ERR"
    exit 1
fi

# --- Step 2: Create Vault ATA ---
section "Step 2: Create Vault ATA"

VAULT_RESP=$(curl -s -X POST "$BASE_URL/api/escrow/create-vault-ata" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$EVENT_ID\"}")

VAULT_SUCCESS=$(echo "$VAULT_RESP" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
if [ "$VAULT_SUCCESS" = "true" ]; then
    VAULT_TX=$(echo "$VAULT_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null)
    VAULT_ADDR=$(echo "$VAULT_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['vault_address'])" 2>/dev/null)
    pass "Vault ATA TX built: ${VAULT_ADDR:0:16}..."

    RESULT=$(sign_and_submit "$VAULT_TX")
    if echo "$RESULT" | grep -q "SIGNATURE="; then
        SIG=$(echo "$RESULT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Vault ATA created: $SIG"
        sleep 5
    else
        fail "Vault ATA TX: $RESULT"
    fi
else
    ERR=$(echo "$VAULT_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "unknown")
    fail "Vault ATA build: $ERR"
    exit 1
fi

# --- Step 3: Create Event Escrow ---
section "Step 3: Create Event Escrow"

CREATE_RESP=$(curl -s -X POST "$BASE_URL/api/escrow/create-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$EVENT_ID\"}")

CREATE_SUCCESS=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
if [ "$CREATE_SUCCESS" = "true" ]; then
    CREATE_TX=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null)
    ESCROW_ADDR=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['escrow_address'])" 2>/dev/null)
    ON_CHAIN_ID=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['on_chain_event_id'])" 2>/dev/null)
    pass "Create event TX built: escrow=${ESCROW_ADDR:0:16}... on_chain_id=$ON_CHAIN_ID"

    RESULT=$(sign_and_submit "$CREATE_TX")
    if echo "$RESULT" | grep -q "SIGNATURE="; then
        SIG=$(echo "$RESULT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Event escrow created: $SIG"
        info "View: https://solscan.io/tx/$SIG?cluster=devnet"
        sleep 5

        # Verify escrow on-chain
        ESCROW_INFO=$(solana account "$ESCROW_ADDR" --url devnet 2>&1 || echo "NOT_FOUND")
        if echo "$ESCROW_INFO" | grep -qi "length:"; then
            pass "Escrow PDA confirmed on-chain"
        else
            fail "Escrow not found: $ESCROW_ADDR"
            echo "   $ESCROW_INFO"
        fi
    else
        fail "Create event TX: $RESULT"
        exit 1
    fi
else
    ERR=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "unknown")
    fail "Create event build: $ERR"
    exit 1
fi

# --- Step 4: Deactivate Event ---
section "Step 4: Deactivate Event"

DEACT_RESP=$(curl -s -X POST "$BASE_URL/api/escrow/deactivate-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$EVENT_ID\"}")

DEACT_SUCCESS=$(echo "$DEACT_RESP" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
if [ "$DEACT_SUCCESS" = "true" ]; then
    DEACT_TX=$(echo "$DEACT_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null)
    pass "Deactivate event TX built"

    RESULT=$(sign_and_submit "$DEACT_TX")
    if echo "$RESULT" | grep -q "SIGNATURE="; then
        SIG=$(echo "$RESULT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Deactivate event submitted: $SIG"
        info "View: https://solscan.io/tx/$SIG?cluster=devnet"

        # Check if it actually succeeded
        sleep 5
        CONFIRM=$(solana confirm "$SIG" --url devnet 2>&1 || echo "confirm failed")
        if echo "$CONFIRM" | grep -qi "confirmed\|finalized"; then
            pass "Deactivate event confirmed on-chain"
        else
            fail "Deactivate event on-chain result: $CONFIRM"
        fi
    else
        fail "Deactivate event TX: $RESULT"
    fi
else
    ERR=$(echo "$DEACT_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "unknown")
    fail "Deactivate event build: $ERR"
fi

# --- Step 5: Claim Forfeited ---
section "Step 5: Claim Forfeited Deposits"

CLAIM_RESP=$(curl -s -X POST "$BASE_URL/api/escrow/claim-forfeited" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$EVENT_ID\"}")

CLAIM_SUCCESS=$(echo "$CLAIM_RESP" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
if [ "$CLAIM_SUCCESS" = "true" ]; then
    CLAIM_TX=$(echo "$CLAIM_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null)
    pass "Claim forfeited TX built"

    RESULT=$(sign_and_submit "$CLAIM_TX")
    if echo "$RESULT" | grep -q "SIGNATURE="; then
        SIG=$(echo "$RESULT" | grep "SIGNATURE=" | cut -d= -f2)
        info "Claim forfeited submitted: $SIG"
        info "View: https://solscan.io/tx/$SIG?cluster=devnet"

        sleep 5
        CONFIRM=$(solana confirm "$SIG" --url devnet 2>&1 || echo "confirm failed")
        if echo "$CONFIRM" | grep -qi "confirmed\|finalized"; then
            pass "Claim forfeited confirmed"
        else
            # Expected: "no forfeited funds" since no deposits were made
            info "Claim forfeited result: $CONFIRM"
            if echo "$CONFIRM" | grep -qi "failed"; then
                info "Expected — no deposits to claim in this test"
            fi
        fi
    else
        info "Claim forfeited TX: $RESULT"
    fi
else
    ERR=$(echo "$CLAIM_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "unknown")
    info "Claim forfeited build: $ERR (may be expected)"
fi

# --- Step 6: Close Event ---
section "Step 6: Close Event & Reclaim Rent"

CLOSE_RESP=$(curl -s -X POST "$BASE_URL/api/escrow/close-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$EVENT_ID\"}")

CLOSE_SUCCESS=$(echo "$CLOSE_RESP" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
if [ "$CLOSE_SUCCESS" = "true" ]; then
    CLOSE_TX=$(echo "$CLOSE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null)
    pass "Close event TX built"

    RESULT=$(sign_and_submit "$CLOSE_TX")
    if echo "$RESULT" | grep -q "SIGNATURE="; then
        SIG=$(echo "$RESULT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Close event submitted: $SIG"
        info "View: https://solscan.io/tx/$SIG?cluster=devnet"

        sleep 5
        CONFIRM=$(solana confirm "$SIG" --url devnet 2>&1 || echo "confirm failed")
        if echo "$CONFIRM" | grep -qi "confirmed\|finalized"; then
            pass "Close event confirmed on-chain"
        else
            fail "Close event on-chain result: $CONFIRM"
        fi

        # Verify escrow account is closed
        ESCROW_CHECK=$(solana account "$ESCROW_ADDR" --url devnet 2>&1 || echo "CLOSED")
        if echo "$ESCROW_CHECK" | grep -qi "error\|not found\|CLOSED"; then
            pass "Escrow account closed — rent reclaimed"
        else
            info "Escrow account state: $(echo "$ESCROW_CHECK" | head -c 100)"
        fi
    else
        fail "Close event TX: $RESULT"
    fi
else
    ERR=$(echo "$CLOSE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "unknown")
    fail "Close event build: $ERR"
fi

echo ""
echo -e "${BOLD}Done. Check explorer links above for TX details.${NC}"
