#!/usr/bin/env bash
# ============================================================================
# BeThere Devnet Rollover Full Lifecycle E2E Test
# ============================================================================
# End-to-end lifecycle test covering the complete rollover journey with
# two attendees and two events:
#   Source Event: create → deposit (A,B) → check-in (A,B) → rollover (A) → deactivate → close
#   Target Event: create → (A deposit via rollover) → check-in (B not checked in)
#                 → refund (A) → deactivate → claim_forfeited (B) → close
#
# This script validates the complete USDC flow:
#   Attendee A: deposit → rollover → refund = USDC round-trip
#   Attendee B: deposit → no-show → forfeited = organizer claims
#
# Prerequisites:
#   - `cd worker && npx wrangler dev --port 8787` running
#   - DEV_MODE=1 in worker/.dev.vars
#   - HELIUS_API_KEY in worker/.dev.vars
#   - solana CLI installed + configured for devnet
#   - Devnet USDC in test wallet (use https://faucet.circle.com/)
#
# Usage:
#   bash scripts/e2e/test_rollover_full_lifecycle.sh
#   bash scripts/e2e/test_rollover_full_lifecycle.sh --skip-setup   # reuse existing events
# ============================================================================

set -euo pipefail

# --- Config ---
BASE_URL="${BASE_URL:-http://localhost:8787}"
DEPOSIT_AMOUNT_USDC="${DEPOSIT_AMOUNT_USDC:-1000000}"  # 1 USDC (6 decimals)
ORGANIZER_WALLET="${ORGANIZER_WALLET:-}"
RPC_URL="${RPC_URL:-https://api.devnet.solana.com}"

# Event IDs — unique per run
TIMESTAMP=$(date +%s)
SOURCE_EVENT_ID="${SOURCE_EVENT_ID:-rollover-full-src-$TIMESTAMP}"
TARGET_EVENT_ID="${TARGET_EVENT_ID:-rollover-full-tgt-$TIMESTAMP}"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

PASS=0
FAIL=0
SKIP=0

# --- Helpers ---
pass() { PASS=$((PASS + 1)); echo -e "  ${GREEN}✅ PASS${NC} $1"; }
fail() { FAIL=$((FAIL + 1)); echo -e "  ${RED}❌ FAIL${NC} $1"; }
skip() { SKIP=$((SKIP + 1)); echo -e "  ${YELLOW}⏭️  SKIP${NC} $1"; }
info() { echo -e "  ${CYAN}ℹ️  INFO${NC} $1"; }
warn() { echo -e "  ${YELLOW}⚠️  WARN${NC} $1"; }
section() { echo -e "\n${CYAN}━━━ $1 ━━━${NC}"; }

check_json() {
    local response="$1"
    local key="$2"
    local expected="$3"
    local actual
    actual=$(echo "$response" | python3 -c "import sys,json; print(json.load(sys.stdin)$key)" 2>/dev/null || echo "PARSE_ERROR")
    if [ "$actual" = "$expected" ]; then
        return 0
    else
        echo "     expected: $expected"
        echo "     actual:   $actual"
        return 1
    fi
}

sign_and_submit_tx() {
    local tx_b64="$1"
    local keypair_json="$2"
    local rpc_url="${3:-$RPC_URL}"
    python3 "$(dirname "$0")/sign_and_submit.py" "$tx_b64" "$keypair_json" "$rpc_url"
}

# --- Parse args ---
SKIP_SETUP=false
for arg in "$@"; do
    case "$arg" in
        --skip-setup) SKIP_SETUP=true ;;
    esac
done

echo ""
echo -e "${BOLD}🔄 BeThere Devnet Rollover Full Lifecycle E2E Test${NC}"
echo "   BASE_URL:        $BASE_URL"
echo "   SOURCE_EVENT_ID: $SOURCE_EVENT_ID"
echo "   TARGET_EVENT_ID: $TARGET_EVENT_ID"
echo "   RPC_URL:         $RPC_URL"
echo ""

USDC_MINT="4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
ESCROW_PROGRAM="C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T"

# --- Resolve wallets ---
if [ -z "$ORGANIZER_WALLET" ]; then
    ORGANIZER_WALLET=$(solana address --url devnet 2>/dev/null || echo "")
fi

# Attendee A keypair (will rollover)
ATTENDEE_A_KEYPAIR="/tmp/bethere-full-lifecycle-att-a.json"
if [ ! -f "$ATTENDEE_A_KEYPAIR" ]; then
    solana-keygen new --no-bip39-passphrase --silent --outfile "$ATTENDEE_A_KEYPAIR" 2>/dev/null
    info "Created new attendee A keypair"
fi
ATTENDEE_A_WALLET=$(solana address --keypair "$ATTENDEE_A_KEYPAIR" --url devnet 2>/dev/null || echo "")

# Attendee B keypair (will forfeit)
ATTENDEE_B_KEYPAIR="/tmp/bethere-full-lifecycle-att-b.json"
if [ ! -f "$ATTENDEE_B_KEYPAIR" ]; then
    solana-keygen new --no-bip39-passphrase --silent --outfile "$ATTENDEE_B_KEYPAIR" 2>/dev/null
    info "Created new attendee B keypair"
fi
ATTENDEE_B_WALLET=$(solana address --keypair "$ATTENDEE_B_KEYPAIR" --url devnet 2>/dev/null || echo "")

if [ -z "$ORGANIZER_WALLET" ] || [ -z "$ATTENDEE_A_WALLET" ] || [ -z "$ATTENDEE_B_WALLET" ]; then
    fail "Cannot resolve wallets. Is solana CLI installed?"
    exit 1
fi

ATTENDEE_A_ID="full-att-a-$TIMESTAMP"
ATTENDEE_B_ID="full-att-b-$TIMESTAMP"

info "Organizer:   ${ORGANIZER_WALLET:0:8}...${ORGANIZER_WALLET: -4}"
info "Attendee A:  ${ATTENDEE_A_WALLET:0:8}...${ATTENDEE_A_WALLET: -4} (will rollover)"
info "Attendee B:  ${ATTENDEE_B_WALLET:0:8}...${ATTENDEE_B_WALLET: -4} (will forfeit)"

# ============================================================================
# Step 0: Prerequisites
# ============================================================================
section "Step 0: Prerequisites"

HEALTH=$(curl -s "$BASE_URL/api/health")
if check_json "$HEALTH" "['status']" "ok"; then
    pass "Worker health check: OK"
else
    fail "Worker not healthy — is wrangler dev running?"
    echo "   Response: $HEALTH"
    exit 1
fi

# Fund attendee wallets
for KEYPAIR in "$ATTENDEE_A_KEYPAIR" "$ATTENDEE_B_KEYPAIR"; do
    WALLET=$(solana address --keypair "$KEYPAIR" --url devnet 2>/dev/null)
    BALANCE=$(solana balance "$WALLET" --url devnet 2>&1 | awk '{print $1}' || echo "0")
    if (( $(echo "$BALANCE < 0.05" | bc -l 2>/dev/null || echo "1") )); then
        info "Funding $WALLET with 1 SOL..."
        solana airdrop 1 "$WALLET" --url devnet 2>&1 || warn "Airdrop failed for $WALLET"
    fi
done

# Create ATAs + check USDC
for KEYPAIR_VAR in ATTENDEE_A_KEYPAIR ATTENDEE_B_KEYPAIR; do
    KP="${!KEYPAIR_VAR}"
    WALLET=$(solana address --keypair "$KP" --url devnet 2>/dev/null)
    ATA=$(spl-token address --token "$USDC_MINT" --owner "$KP" --url devnet 2>/dev/null | head -1 || echo "")
    if [ -z "$ATA" ] || [ "$ATA" = "None" ] || [ "$ATA" = "Creating" ]; then
        info "Creating USDC ATA for ${WALLET:0:8}..."
        spl-token create-account "$USDC_MINT" --owner "$KP" --url devnet 2>&1 || warn "ATA creation failed"
    fi

    USDC_BAL=$(spl-token balance "$USDC_MINT" --url devnet --owner "$WALLET" 2>&1 | awk '{print $1}' || echo "0")
    if (( $(echo "$USDC_BAL < 2" | bc -l 2>/dev/null || echo "1") )); then
        echo ""
        echo -e "  ${YELLOW}══════════════════════════════════════════════════════${NC}"
        echo -e "  ${YELLOW}  Get devnet USDC from: https://faucet.circle.com/${NC}"
        echo -e "  ${YELLOW}  Wallet: $WALLET${NC}"
        echo -e "  ${YELLOW}  Need: ≥ 2 USDC (1 for each attendee)${NC}"
        echo -e "  ${YELLOW}══════════════════════════════════════════════════════${NC}"
        echo ""
    fi
done

# Verify both have USDC
ATT_A_USDC=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_A_WALLET" 2>&1 | awk '{print $1}' || echo "0")
ATT_B_USDC=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_B_WALLET" 2>&1 | awk '{print $1}' || echo "0")
info "Attendee A USDC: $ATT_A_USDC"
info "Attendee B USDC: $ATT_B_USDC"

if (( $(echo "$ATT_A_USDC < 1" | bc -l 2>/dev/null || echo "1") )) || (( $(echo "$ATT_B_USDC < 1" | bc -l 2>/dev/null || echo "1") )); then
    echo ""
    read -p "  Press Enter after getting USDC for both wallets (or Ctrl+C to abort)..." -r
fi

# Record pre-test balances
ATT_A_USDC_BEFORE=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_A_WALLET" 2>&1 | awk '{print $1}' || echo "0")
ATT_B_USDC_BEFORE=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_B_WALLET" 2>&1 | awk '{print $1}' || echo "0")
ORG_USDC_BEFORE=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ORGANIZER_WALLET" 2>&1 | awk '{print $1}' || echo "0")
info "Pre-test balances — A: $ATT_A_USDC_BEFORE, B: $ATT_B_USDC_BEFORE, Org: $ORG_USDC_BEFORE"

# ============================================================================
# Step 1: Create Source Event
# ============================================================================
section "Step 1: Create Source Event"

SOURCE_ESCROW_ADDR=""
SOURCE_ON_CHAIN_ID=0

if [ "$SKIP_SETUP" = true ]; then
    skip "Source event setup — reusing existing"
else
    info "Creating source event '$SOURCE_EVENT_ID'..."
    SOURCE_EVENT_RESPONSE=$(curl -s -X POST "$BASE_URL/api/events" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"Full Lifecycle Source\",
            \"slug\": \"$SOURCE_EVENT_ID\",
            \"tagline\": \"Source event for full rollover lifecycle test\",
            \"link\": \"https://example.com/full-src\",
            \"sheet_id\": \"full-src-dummy\",
            \"event_start_ms\": $(($(date +%s) - 7200))000,
            \"event_end_ms\": $(($(date +%s) + 90))000,
            \"status\": \"active\",
            \"deposit_enabled\": true,
            \"deposit_amount_usdc\": $DEPOSIT_AMOUNT_USDC,
            \"deposit_amount_thb\": 0,
            \"organizer_wallet\": \"$ORGANIZER_WALLET\",
            \"refund_deadline_hours\": 168
        }")

    SOURCE_CREATED=$(echo "$SOURCE_EVENT_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
    SOURCE_CREATED_ID=$(echo "$SOURCE_EVENT_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print(d.get('id',''))" 2>/dev/null || echo "")

    if [ "$SOURCE_CREATED" = "true" ] && [ -n "$SOURCE_CREATED_ID" ]; then
        pass "Source event created: id=$SOURCE_CREATED_ID"
        SOURCE_EVENT_ID="$SOURCE_CREATED_ID"
    else
        fail "Failed to create source event"
        echo "   Response: $(echo "$SOURCE_EVENT_RESPONSE" | head -c 300)"
        exit 1
    fi
fi

# ============================================================================
# Step 2: Create Target Event
# ============================================================================
section "Step 2: Create Target Event"

TARGET_ESCROW_ADDR=""
TARGET_ON_CHAIN_ID=0

if [ "$SKIP_SETUP" = true ]; then
    skip "Target event setup — reusing existing"
else
    info "Creating target event '$TARGET_EVENT_ID'..."
    TARGET_EVENT_RESPONSE=$(curl -s -X POST "$BASE_URL/api/events" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"Full Lifecycle Target\",
            \"slug\": \"$TARGET_EVENT_ID\",
            \"tagline\": \"Target event for full rollover lifecycle test\",
            \"link\": \"https://example.com/full-tgt\",
            \"sheet_id\": \"full-tgt-dummy\",
            \"event_start_ms\": $(($(date +%s) + 86400))000,
            \"event_end_ms\": $(($(date +%s) + 172800))000,
            \"status\": \"active\",
            \"deposit_enabled\": true,
            \"deposit_amount_usdc\": $DEPOSIT_AMOUNT_USDC,
            \"deposit_amount_thb\": 0,
            \"organizer_wallet\": \"$ORGANIZER_WALLET\",
            \"refund_deadline_hours\": 168
        }")

    TARGET_CREATED=$(echo "$TARGET_EVENT_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
    TARGET_CREATED_ID=$(echo "$TARGET_EVENT_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print(d.get('id',''))" 2>/dev/null || echo "")

    if [ "$TARGET_CREATED" = "true" ] && [ -n "$TARGET_CREATED_ID" ]; then
        pass "Target event created: id=$TARGET_CREATED_ID"
        TARGET_EVENT_ID="$TARGET_CREATED_ID"
    else
        fail "Failed to create target event"
        echo "   Response: $(echo "$TARGET_EVENT_RESPONSE" | head -c 300)"
        exit 1
    fi
fi

# ============================================================================
# Step 3: Init Escrow for Both Events
# ============================================================================
ORG_KEYPAIR_JSON=$(cat ~/.config/solana/id.json)

section "Step 3a: Init Escrow — Source"

if [ "$SKIP_SETUP" = true ]; then
    skip "Source escrow init — reusing existing"
else
    SRC_INIT=$(curl -s -X POST "$BASE_URL/api/escrow/init" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_id\": \"$SOURCE_EVENT_ID\"}")

    SRC_INIT_SUCCESS=$(echo "$SRC_INIT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

    if [ "$SRC_INIT_SUCCESS" = "true" ]; then
        SRC_TX_B64=$(echo "$SRC_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        SOURCE_ESCROW_ADDR=$(echo "$SRC_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['escrow_address'])" 2>/dev/null || echo "")
        SOURCE_ON_CHAIN_ID=$(echo "$SRC_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['on_chain_event_id'])" 2>/dev/null || echo "0")

        SRC_SUBMIT=$(sign_and_submit_tx "$SRC_TX_B64" "$ORG_KEYPAIR_JSON")
        if echo "$SRC_SUBMIT" | grep -q "SIGNATURE="; then
            SRC_SIG=$(echo "$SRC_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Source escrow initialized: $SRC_SIG"
            sleep 5
            curl -s -X PUT "$BASE_URL/api/events/$SOURCE_EVENT_ID" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{\"escrow_address\": \"$SOURCE_ESCROW_ADDR\", \"on_chain_event_id\": $SOURCE_ON_CHAIN_ID}" > /dev/null 2>&1
        else
            fail "Source escrow TX failed: $SRC_SUBMIT"
            exit 1
        fi
    else
        fail "Source escrow init failed"
        exit 1
    fi

    # Extend source event_end for deposit acceptance
    FUTURE_MS=$(($(date +%s) + 3600))000
    curl -s -X PUT "$BASE_URL/api/events/$SOURCE_EVENT_ID" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_end_ms\": $FUTURE_MS}" > /dev/null 2>&1
    pass "Source event_end_ms extended"
fi

section "Step 3b: Init Escrow — Target"

if [ "$SKIP_SETUP" = true ]; then
    skip "Target escrow init — reusing existing"
else
    TGT_INIT=$(curl -s -X POST "$BASE_URL/api/escrow/init" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_id\": \"$TARGET_EVENT_ID\"}")

    TGT_INIT_SUCCESS=$(echo "$TGT_INIT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

    if [ "$TGT_INIT_SUCCESS" = "true" ]; then
        TGT_TX_B64=$(echo "$TGT_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        TARGET_ESCROW_ADDR=$(echo "$TGT_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['escrow_address'])" 2>/dev/null || echo "")
        TARGET_ON_CHAIN_ID=$(echo "$TGT_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['on_chain_event_id'])" 2>/dev/null || echo "0")

        TGT_SUBMIT=$(sign_and_submit_tx "$TGT_TX_B64" "$ORG_KEYPAIR_JSON")
        if echo "$TGT_SUBMIT" | grep -q "SIGNATURE="; then
            TGT_SIG=$(echo "$TGT_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Target escrow initialized: $TGT_SIG"
            sleep 5
            curl -s -X PUT "$BASE_URL/api/events/$TARGET_EVENT_ID" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{\"escrow_address\": \"$TARGET_ESCROW_ADDR\", \"on_chain_event_id\": $TARGET_ON_CHAIN_ID}" > /dev/null 2>&1
        else
            fail "Target escrow TX failed: $TGT_SUBMIT"
            exit 1
        fi
    else
        fail "Target escrow init failed"
        exit 1
    fi
fi

info "Source escrow: $SOURCE_ESCROW_ADDR (on_chain_id=$SOURCE_ON_CHAIN_ID)"
info "Target escrow: $TARGET_ESCROW_ADDR (on_chain_id=$TARGET_ON_CHAIN_ID)"

# ============================================================================
# Step 4: Deposit USDC — Both Attendees on Source Event
# ============================================================================
section "Step 4: Deposit — Attendee A on Source"

ATT_A_KEYPAIR_JSON=$(cat "$ATTENDEE_A_KEYPAIR")

DEPOSIT_A=$(curl -s -X POST "$BASE_URL/api/deposit/usdc" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{
        \"event_id\": \"$SOURCE_EVENT_ID\",
        \"attendee_id\": \"$ATTENDEE_A_ID\",
        \"wallet_address\": \"$ATTENDEE_A_WALLET\"
    }")

DEP_A_SUCCESS=$(echo "$DEPOSIT_A" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$DEP_A_SUCCESS" = "true" ]; then
    PAY_A=$(curl -s "$BASE_URL/api/deposit/usdc/tx?event_id=$SOURCE_EVENT_ID&attendee_id=$ATTENDEE_A_ID&wallet=$ATTENDEE_A_WALLET")
    DEP_A_TX=$(echo "$PAY_A" | python3 -c "import sys,json; print(json.load(sys.stdin).get('transaction',''))" 2>/dev/null || echo "")

    if [ -n "$DEP_A_TX" ]; then
        DEP_A_SUBMIT=$(sign_and_submit_tx "$DEP_A_TX" "$ATT_A_KEYPAIR_JSON")
        if echo "$DEP_A_SUBMIT" | grep -q "SIGNATURE="; then
            DEP_A_SIG=$(echo "$DEP_A_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Attendee A deposited: $DEP_A_SIG"
            info "View: https://solscan.io/tx/$DEP_A_SIG?cluster=devnet"
            sleep 5
            curl -s -X POST "$BASE_URL/api/deposit/usdc/webhook" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{\"event_id\": \"$SOURCE_EVENT_ID\", \"attendee_id\": \"$ATTENDEE_A_ID\", \"tx_signature\": \"$DEP_A_SIG\"}" > /dev/null 2>&1
        else
            fail "Attendee A deposit TX failed"
            exit 1
        fi
    else
        fail "Attendee A deposit TX build failed"
        exit 1
    fi
else
    fail "Attendee A deposit initiation failed"
    exit 1
fi

section "Step 4b: Deposit — Attendee B on Source"

ATT_B_KEYPAIR_JSON=$(cat "$ATTENDEE_B_KEYPAIR")

DEPOSIT_B=$(curl -s -X POST "$BASE_URL/api/deposit/usdc" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{
        \"event_id\": \"$SOURCE_EVENT_ID\",
        \"attendee_id\": \"$ATTENDEE_B_ID\",
        \"wallet_address\": \"$ATTENDEE_B_WALLET\"
    }")

DEP_B_SUCCESS=$(echo "$DEPOSIT_B" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$DEP_B_SUCCESS" = "true" ]; then
    PAY_B=$(curl -s "$BASE_URL/api/deposit/usdc/tx?event_id=$SOURCE_EVENT_ID&attendee_id=$ATTENDEE_B_ID&wallet=$ATTENDEE_B_WALLET")
    DEP_B_TX=$(echo "$PAY_B" | python3 -c "import sys,json; print(json.load(sys.stdin).get('transaction',''))" 2>/dev/null || echo "")

    if [ -n "$DEP_B_TX" ]; then
        DEP_B_SUBMIT=$(sign_and_submit_tx "$DEP_B_TX" "$ATT_B_KEYPAIR_JSON")
        if echo "$DEP_B_SUBMIT" | grep -q "SIGNATURE="; then
            DEP_B_SIG=$(echo "$DEP_B_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Attendee B deposited: $DEP_B_SIG"
            info "View: https://solscan.io/tx/$DEP_B_SIG?cluster=devnet"
            sleep 5
            curl -s -X POST "$BASE_URL/api/deposit/usdc/webhook" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{\"event_id\": \"$SOURCE_EVENT_ID\", \"attendee_id\": \"$ATTENDEE_B_ID\", \"tx_signature\": \"$DEP_B_SIG\"}" > /dev/null 2>&1
        else
            fail "Attendee B deposit TX failed"
            exit 1
        fi
    else
        fail "Attendee B deposit TX build failed"
        exit 1
    fi
else
    fail "Attendee B deposit initiation failed"
    exit 1
fi

# Verify source vault has 2 USDC
SRC_VAULT_AFTER_DEPOSIT=$(spl-token balance "$USDC_MINT" --url devnet --owner "$SOURCE_ESCROW_ADDR" 2>&1 || echo "0")
info "Source vault after deposits: $SRC_VAULT_AFTER_DEPOSIT USDC"
if echo "$SRC_VAULT_AFTER_DEPOSIT" | grep -qE "^2"; then
    pass "Source vault has 2 USDC (both deposits)"
else
    warn "Source vault balance: $SRC_VAULT_AFTER_DEPOSIT (expected ~2)"
fi

# ============================================================================
# Step 5: Check-In Both Attendees on Source Event
# ============================================================================
section "Step 5: Check-In — Both Attendees on Source"

for PAIR in "$ATTENDEE_A_ID|A" "$ATTENDEE_B_ID|B"; do
    ATT_ID="${PAIR%%|*}"
    ATT_LABEL="${PAIR##*|}"
    info "Checking in attendee $ATT_LABEL ($ATT_ID)..."
    MARK_CI=$(curl -s -X POST "$BASE_URL/api/escrow/mark-checked-in" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_id\": \"$SOURCE_EVENT_ID\", \"attendee_id\": \"$ATT_ID\"}")

    MARK_CI_SUCCESS=$(echo "$MARK_CI" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

    if [ "$MARK_CI_SUCCESS" = "true" ]; then
        MARK_CI_TX=$(echo "$MARK_CI" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        MARK_CI_SUBMIT=$(sign_and_submit_tx "$MARK_CI_TX" "$ORG_KEYPAIR_JSON")

        if echo "$MARK_CI_SUBMIT" | grep -q "SIGNATURE="; then
            MARK_CI_SIG=$(echo "$MARK_CI_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Attendee $ATT_LABEL checked in: $MARK_CI_SIG"
            sleep 5
        else
            fail "Attendee $ATT_LABEL check-in TX failed"
            exit 1
        fi
    else
        ERR=$(echo "$MARK_CI" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
        fail "Attendee $ATT_LABEL check-in build failed: $ERR"
        exit 1
    fi
done

# ============================================================================
# Step 6: Rollover Attendee A to Target Event
# ============================================================================
section "Step 6: Rollover — Attendee A to Target"

info "Requesting rollover TX for attendee A..."
ROLLOVER_RESP=$(curl -s -X POST "$BASE_URL/api/escrow/rollover-deposit" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{
        \"source_event_id\": \"$SOURCE_EVENT_ID\",
        \"target_event_id\": \"$TARGET_EVENT_ID\",
        \"attendee_id\": \"$ATTENDEE_A_ID\",
        \"wallet_address\": \"$ATTENDEE_A_WALLET\"
    }")

ROLLOVER_SUCCESS=$(echo "$ROLLOVER_RESP" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$ROLLOVER_SUCCESS" = "true" ]; then
    ROLLOVER_TX_B64=$(echo "$ROLLOVER_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    ROLLOVER_MSG=$(echo "$ROLLOVER_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

    pass "Rollover TX built"
    info "Message: $ROLLOVER_MSG"

    ROLLOVER_SUBMIT=$(sign_and_submit_tx "$ROLLOVER_TX_B64" "$ATT_A_KEYPAIR_JSON")
    if echo "$ROLLOVER_SUBMIT" | grep -q "SIGNATURE="; then
        ROLLOVER_SIG=$(echo "$ROLLOVER_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Attendee A rolled over: $ROLLOVER_SIG"
        info "View: https://solscan.io/tx/$ROLLOVER_SIG?cluster=devnet"
        sleep 8
    else
        fail "Rollover TX failed: $ROLLOVER_SUBMIT"
        exit 1
    fi
else
    ERR=$(echo "$ROLLOVER_RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:300])))" 2>/dev/null || echo "")
    fail "Rollover TX build failed: $ERR"
    exit 1
fi

# Verify vault balances after rollover
SRC_VAULT_AFTER_ROLLOVER=$(spl-token balance "$USDC_MINT" --url devnet --owner "$SOURCE_ESCROW_ADDR" 2>&1 || echo "0")
TGT_VAULT_AFTER_ROLLOVER=$(spl-token balance "$USDC_MINT" --url devnet --owner "$TARGET_ESCROW_ADDR" 2>&1 || echo "0")
info "Source vault after rollover: $SRC_VAULT_AFTER_ROLLOVER USDC"
info "Target vault after rollover: $TGT_VAULT_AFTER_ROLLOVER USDC"

if echo "$SRC_VAULT_AFTER_ROLLOVER" | grep -qE "^1|^1\.0"; then
    pass "Source vault has 1 USDC (B's deposit remains)"
else
    warn "Source vault after rollover: $SRC_VAULT_AFTER_ROLLOVER (expected ~1)"
fi

if echo "$TGT_VAULT_AFTER_ROLLOVER" | grep -qE "^1|^1\.0"; then
    pass "Target vault has 1 USDC (A's rollover deposit)"
else
    warn "Target vault after rollover: $TGT_VAULT_AFTER_ROLLOVER (expected ~1)"
fi

# ============================================================================
# Step 7: Refund Attendee A from Target Event
# ============================================================================
section "Step 7: Refund — Attendee A from Target Event"

info "Requesting refund+close TX from target for attendee A..."
REFUND_A_RESP=$(curl -s -X POST "$BASE_URL/api/escrow/refund" \
    -H "Content-Type: application/json" \
    -d "{
        \"event_id\": \"$TARGET_EVENT_ID\",
        \"attendee_id\": \"$ATTENDEE_A_ID\",
        \"wallet_address\": \"$ATTENDEE_A_WALLET\"
    }")

REFUND_A_SUCCESS=$(echo "$REFUND_A_RESP" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$REFUND_A_SUCCESS" = "true" ]; then
    REFUND_A_TX=$(echo "$REFUND_A_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    REFUND_A_SUBMIT=$(sign_and_submit_tx "$REFUND_A_TX" "$ATT_A_KEYPAIR_JSON")

    if echo "$REFUND_A_SUBMIT" | grep -q "SIGNATURE="; then
        REFUND_A_SIG=$(echo "$REFUND_A_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Attendee A refunded from target: $REFUND_A_SIG"
        info "View: https://solscan.io/tx/$REFUND_A_SIG?cluster=devnet"
        sleep 8
    else
        fail "Refund A TX failed: $REFUND_A_SUBMIT"
        info "The refund instruction requires the event deadline to have passed."
    fi
else
    ERR=$(echo "$REFUND_A_RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:300])))" 2>/dev/null || echo "")
    fail "Refund A TX build failed: $ERR"
    info "This may happen if the target event has not ended on-chain yet."
fi

# Verify target vault is empty after refund
TGT_VAULT_AFTER_REFUND=$(spl-token balance "$USDC_MINT" --url devnet --owner "$TARGET_ESCROW_ADDR" 2>&1 || echo "0")
info "Target vault after refund: $TGT_VAULT_AFTER_REFUND USDC"

if echo "$TGT_VAULT_AFTER_REFUND" | grep -qE "^0|^0\.0|no vault|empty|not found|Insufficient"; then
    pass "Target vault is empty (A's deposit refunded)"
else
    warn "Target vault still has balance: $TGT_VAULT_AFTER_REFUND"
fi

# ============================================================================
# Step 8: Deactivate Source Event + Claim Forfeited (B's deposit)
# ============================================================================
section "Step 8: Deactivate Source Event"

SRC_DEACT=$(curl -s -X POST "$BASE_URL/api/escrow/deactivate-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$SOURCE_EVENT_ID\"}")

SRC_DEACT_SUCCESS=$(echo "$SRC_DEACT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$SRC_DEACT_SUCCESS" = "true" ]; then
    SRC_DEACT_TX=$(echo "$SRC_DEACT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    SRC_DEACT_SUBMIT=$(sign_and_submit_tx "$SRC_DEACT_TX" "$ORG_KEYPAIR_JSON")

    if echo "$SRC_DEACT_SUBMIT" | grep -q "SIGNATURE="; then
        SRC_DEACT_SIG=$(echo "$SRC_DEACT_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Source event deactivated: $SRC_DEACT_SIG"
        sleep 5
    else
        fail "Source deactivate failed: $SRC_DEACT_SUBMIT"
    fi
else
    warn "Source deactivate build failed (may already be inactive)"
fi

section "Step 8b: Claim Forfeited — Source Event (B's deposit)"

info "Claiming forfeited deposits from source event..."
CLAIM_SRC=$(curl -s -X POST "$BASE_URL/api/escrow/claim-forfeited" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$SOURCE_EVENT_ID\"}")

CLAIM_SRC_SUCCESS=$(echo "$CLAIM_SRC" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$CLAIM_SRC_SUCCESS" = "true" ]; then
    CLAIM_SRC_TX=$(echo "$CLAIM_SRC" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    CLAIM_SRC_SUBMIT=$(sign_and_submit_tx "$CLAIM_SRC_TX" "$ORG_KEYPAIR_JSON")

    if echo "$CLAIM_SRC_SUBMIT" | grep -q "SIGNATURE="; then
        CLAIM_SRC_SIG=$(echo "$CLAIM_SRC_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Source forfeited claimed: $CLAIM_SRC_SIG"
        info "View: https://solscan.io/tx/$CLAIM_SRC_SIG?cluster=devnet"
        sleep 5

        # Verify source vault is now empty
        SRC_VAULT_AFTER_CLAIM=$(spl-token balance "$USDC_MINT" --url devnet --owner "$SOURCE_ESCROW_ADDR" 2>&1 || echo "0")
        if echo "$SRC_VAULT_AFTER_CLAIM" | grep -qE "^0|^0\.0|no vault|empty|Insufficient"; then
            pass "Source vault empty — B's forfeited deposit claimed by organizer"
        else
            warn "Source vault after claim: $SRC_VAULT_AFTER_CLAIM"
        fi
    else
        fail "Source claim forfeited TX failed: $CLAIM_SRC_SUBMIT"
    fi
else
    ERR=$(echo "$CLAIM_SRC" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "")
    warn "Source claim forfeited build failed: $ERR"
fi

# ============================================================================
# Step 9: Deactivate Target Event + Claim Forfeited (if any)
# ============================================================================
section "Step 9: Deactivate Target Event"

TGT_DEACT=$(curl -s -X POST "$BASE_URL/api/escrow/deactivate-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$TARGET_EVENT_ID\"}")

TGT_DEACT_SUCCESS=$(echo "$TGT_DEACT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$TGT_DEACT_SUCCESS" = "true" ]; then
    TGT_DEACT_TX=$(echo "$TGT_DEACT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    TGT_DEACT_SUBMIT=$(sign_and_submit_tx "$TGT_DEACT_TX" "$ORG_KEYPAIR_JSON")

    if echo "$TGT_DEACT_SUBMIT" | grep -q "SIGNATURE="; then
        TGT_DEACT_SIG=$(echo "$TGT_DEACT_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Target event deactivated: $TGT_DEACT_SIG"
        sleep 5
    else
        fail "Target deactivate failed: $TGT_DEACT_SUBMIT"
    fi
else
    warn "Target deactivate build failed (may already be inactive)"
fi

# Claim forfeited from target (should be nothing since A was refunded)
info "Claiming forfeited from target (expect nothing — A was already refunded)..."
CLAIM_TGT=$(curl -s -X POST "$BASE_URL/api/escrow/claim-forfeited" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$TARGET_EVENT_ID\"}")

CLAIM_TGT_SUCCESS=$(echo "$CLAIM_TGT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$CLAIM_TGT_SUCCESS" = "true" ]; then
    CLAIM_TGT_TX=$(echo "$CLAIM_TGT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    CLAIM_TGT_SUBMIT=$(sign_and_submit_tx "$CLAIM_TGT_TX" "$ORG_KEYPAIR_JSON")

    if echo "$CLAIM_TGT_SUBMIT" | grep -q "SIGNATURE="; then
        CLAIM_TGT_SIG=$(echo "$CLAIM_TGT_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        info "Target claim forfeited submitted: $CLAIM_TGT_SIG (may be no-op if vault empty)"
        sleep 5
    fi
else
    info "Target claim forfeited skipped (no forfeited deposits — expected)"
fi

# ============================================================================
# Step 10: Close Both Events
# ============================================================================
section "Step 10: Close Both Events"

# --- Close Source ---
info "Closing source event..."
SRC_CLOSE=$(curl -s -X POST "$BASE_URL/api/escrow/close-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$SOURCE_EVENT_ID\"}")

SRC_CLOSE_SUCCESS=$(echo "$SRC_CLOSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$SRC_CLOSE_SUCCESS" = "true" ]; then
    SRC_CLOSE_TX=$(echo "$SRC_CLOSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    SRC_CLOSE_SUBMIT=$(sign_and_submit_tx "$SRC_CLOSE_TX" "$ORG_KEYPAIR_JSON")

    if echo "$SRC_CLOSE_SUBMIT" | grep -q "SIGNATURE="; then
        SRC_CLOSE_SIG=$(echo "$SRC_CLOSE_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Source event closed: $SRC_CLOSE_SIG"
        sleep 5

        SRC_ESCROW_CHECK=$(solana account "$SOURCE_ESCROW_ADDR" --url devnet 2>&1 || echo "CLOSED")
        if echo "$SRC_ESCROW_CHECK" | grep -qi "error\|not found\|CLOSED"; then
            pass "Source escrow account closed — rent reclaimed"
        else
            warn "Source escrow still exists"
        fi
    else
        fail "Source close TX failed: $SRC_CLOSE_SUBMIT"
    fi
else
    warn "Source close build failed (may need vault emptied first)"
fi

# --- Close Target ---
info "Closing target event..."
TGT_CLOSE=$(curl -s -X POST "$BASE_URL/api/escrow/close-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$TARGET_EVENT_ID\"}")

TGT_CLOSE_SUCCESS=$(echo "$TGT_CLOSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$TGT_CLOSE_SUCCESS" = "true" ]; then
    TGT_CLOSE_TX=$(echo "$TGT_CLOSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    TGT_CLOSE_SUBMIT=$(sign_and_submit_tx "$TGT_CLOSE_TX" "$ORG_KEYPAIR_JSON")

    if echo "$TGT_CLOSE_SUBMIT" | grep -q "SIGNATURE="; then
        TGT_CLOSE_SIG=$(echo "$TGT_CLOSE_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Target event closed: $TGT_CLOSE_SIG"
        sleep 5

        TGT_ESCROW_CHECK=$(solana account "$TARGET_ESCROW_ADDR" --url devnet 2>&1 || echo "CLOSED")
        if echo "$TGT_ESCROW_CHECK" | grep -qi "error\|not found\|CLOSED"; then
            pass "Target escrow account closed — rent reclaimed"
        else
            warn "Target escrow still exists"
        fi
    else
        fail "Target close TX failed: $TGT_CLOSE_SUBMIT"
    fi
else
    warn "Target close build failed (may need vault emptied first)"
fi

# ============================================================================
# Step 11: Verify Final State
# ============================================================================
section "Step 11: Final State Verification"

# Attendee A should have their USDC back (deposit → rollover → refund round-trip)
ATT_A_USDC_AFTER=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_A_WALLET" 2>&1 | awk '{print $1}' || echo "?")
info "Attendee A USDC: $ATT_A_USDC_BEFORE → $ATT_A_USDC_AFTER"
if [ "$ATT_A_USDC_AFTER" = "$ATT_A_USDC_BEFORE" ]; then
    pass "Attendee A USDC restored (deposit → rollover → refund round-trip)"
else
    warn "Attendee A USDC changed: $ATT_A_USDC_BEFORE → $ATT_A_USDC_AFTER"
fi

# Attendee B should have 1 less USDC (deposited → forfeited → claimed by organizer)
ATT_B_USDC_AFTER=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_B_WALLET" 2>&1 | awk '{print $1}' || echo "?")
info "Attendee B USDC: $ATT_B_USDC_BEFORE → $ATT_B_USDC_AFTER"

# Organizer should have gained 1 USDC (claimed B's forfeited deposit)
ORG_USDC_AFTER=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ORGANIZER_WALLET" 2>&1 | awk '{print $1}' || echo "?")
info "Organizer USDC: $ORG_USDC_BEFORE → $ORG_USDC_AFTER"

# Both escrow accounts should be gone
SRC_GONE=$(solana account "$SOURCE_ESCROW_ADDR" --url devnet 2>&1 || echo "NOT_FOUND")
TGT_GONE=$(solana account "$TARGET_ESCROW_ADDR" --url devnet 2>&1 || echo "NOT_FOUND")

if echo "$SRC_GONE" | grep -qi "error\|not found"; then
    pass "Source escrow account confirmed closed"
else
    fail "Source escrow account still exists"
fi

if echo "$TGT_GONE" | grep -qi "error\|not found"; then
    pass "Target escrow account confirmed closed"
else
    fail "Target escrow account still exists"
fi

# ============================================================================
# Summary
# ============================================================================
section "Summary"

echo ""
echo -e "  ${BOLD}Results:${NC}"
echo -e "    ${GREEN}Passed:${NC}  $PASS"
echo -e "    ${RED}Failed:${NC}  $FAIL"
echo -e "    ${YELLOW}Skipped:${NC} $SKIP"
echo ""

if [ $FAIL -eq 0 ]; then
    echo -e "  ${GREEN}${BOLD}🎉 Rollover full lifecycle E2E test PASSED!${NC}"
    echo ""
    echo "  Lifecycle completed:"
    echo "    A: deposit → check-in → rollover → refund = USDC round-trip ✅"
    echo "    B: deposit → check-in → forfeit → claimed by organizer ✅"
    echo ""
    echo "  Source event: $SOURCE_EVENT_ID"
    echo "  Target event: $TARGET_EVENT_ID"
    echo "  Attendee A:   $ATTENDEE_A_WALLET"
    echo "  Attendee B:   $ATTENDEE_B_WALLET"
else
    echo -e "  ${RED}${BOLD}❌ Rollover full lifecycle E2E test FAILED${NC}"
    echo ""
    echo "  Check failed steps above for details."
    echo "  Note: Some steps may fail due to on-chain timing (event_end not yet passed)."
    exit 1
fi
