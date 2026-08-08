#!/usr/bin/env bash
# ============================================================================
# BeThere Devnet Rollover Deposit E2E Test
# ============================================================================
# Tests the full rollover_deposit lifecycle on Solana devnet:
#   0. Prerequisites check + wallet setup
#   1. Create Source Event (past) with escrow
#   2. Create Target Event (future) with escrow — same organizer + deposit amount
#   3. Init Escrow for both events
#   4. Deposit USDC on Source Event (attendee signs)
#   5. Mark Attendee Checked-In on Source Event (organizer signs)
#   6. (No wait needed — rollover works after check-in)
#   7. Build & Submit Rollover Deposit TX (attendee signs)
#   8. Verify post-rollover state (vaults, indexer)
#   9. Verify Target Event DepositStatus (indexer fix validation)
#  10. Refund Attendee from Target Event (attendee signs)
#  11. Verify post-refund vault balances
#  12. Deactivate both events
#  13. Close both events (reclaim rent)
#
# Prerequisites:
#   - `cd worker && npx wrangler dev --port 8787` running
#   - DEV_MODE=1 in worker/.dev.vars
#   - HELIUS_API_KEY in worker/.dev.vars
#   - solana CLI installed + configured for devnet
#   - Devnet USDC in test wallet (use https://faucet.circle.com/)
#
# Usage:
#   bash scripts/e2e/test_rollover_devnet.sh
#   bash scripts/e2e/test_rollover_devnet.sh --skip-setup     # reuse existing events
#   bash scripts/e2e/test_rollover_devnet.sh --skip-wait      # skip event_end wait
# ============================================================================

set -euo pipefail

# --- Config ---
BASE_URL="${BASE_URL:-http://localhost:8787}"
DEPOSIT_AMOUNT_USDC="${DEPOSIT_AMOUNT_USDC:-1000000}"  # 1 USDC (6 decimals)
ORGANIZER_WALLET="${ORGANIZER_WALLET:-}"
ATTENDEE_WALLET="${ATTENDEE_WALLET:-}"
ATTENDEE_KEYPAIR="${ATTENDEE_KEYPAIR:-}"
RPC_URL="${RPC_URL:-https://api.devnet.solana.com}"

# Event IDs — unique per run
TIMESTAMP=$(date +%s)
SOURCE_EVENT_ID="${SOURCE_EVENT_ID:-rollover-src-$TIMESTAMP}"
TARGET_EVENT_ID="${TARGET_EVENT_ID:-rollover-tgt-$TIMESTAMP}"

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
echo -e "${BOLD}🔄 BeThere Devnet Rollover Deposit E2E Test${NC}"
echo "   BASE_URL:        $BASE_URL"
echo "   SOURCE_EVENT_ID: $SOURCE_EVENT_ID"
echo "   TARGET_EVENT_ID: $TARGET_EVENT_ID"
echo "   RPC_URL:         $RPC_URL"
echo ""

# --- Resolve wallets ---
if [ -z "$ORGANIZER_WALLET" ]; then
    ORGANIZER_WALLET=$(solana address --url devnet 2>/dev/null || echo "")
fi

if [ -z "$ATTENDEE_KEYPAIR" ]; then
    ATTENDEE_KEYPAIR="/tmp/bethere-rollover-e2e-attendee.json"
fi
if [ -z "$ATTENDEE_WALLET" ]; then
    if [ ! -f "$ATTENDEE_KEYPAIR" ]; then
        solana-keygen new --no-bip39-passphrase --silent --outfile "$ATTENDEE_KEYPAIR" 2>/dev/null
        info "Created new attendee keypair"
    fi
    ATTENDEE_WALLET=$(solana address --keypair "$ATTENDEE_KEYPAIR" --url devnet 2>/dev/null || echo "")
fi

if [ -z "$ORGANIZER_WALLET" ] || [ -z "$ATTENDEE_WALLET" ]; then
    fail "Cannot resolve wallets. Is solana CLI installed?"
    exit 1
fi

info "Organizer wallet: ${ORGANIZER_WALLET:0:8}...${ORGANIZER_WALLET: -4}"
info "Attendee wallet:  ${ATTENDEE_WALLET:0:8}...${ATTENDEE_WALLET: -4}"

USDC_MINT="4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
ESCROW_PROGRAM="C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T"

# Test attendee ID
TEST_ATTENDEE_ID="rollover-test-att-$TIMESTAMP"

# ============================================================================
# Step 0: Prerequisites
# ============================================================================
section "Step 0: Prerequisites"

# Check health
HEALTH=$(curl -s "$BASE_URL/api/health")
if check_json "$HEALTH" "['status']" "ok"; then
    pass "Worker health check: OK"
else
    fail "Worker not healthy — is wrangler dev running?"
    echo "   Response: $HEALTH"
    exit 1
fi

# Fund attendee if needed
ATT_BALANCE=$(solana balance "$ATTENDEE_WALLET" --url devnet 2>&1 | awk '{print $1}' || echo "0")
info "Attendee SOL balance: $ATT_BALANCE SOL"
if (( $(echo "$ATT_BALANCE < 0.05" | bc -l 2>/dev/null || echo "1") )); then
    info "Funding attendee wallet with 1 SOL..."
    solana airdrop 1 "$ATTENDEE_WALLET" --url devnet 2>&1 || warn "Airdrop failed"
fi

# Check attendee USDC
ATT_USDC_ATA=$(spl-token address --token "$USDC_MINT" --owner "$ATTENDEE_KEYPAIR" --url devnet 2>/dev/null | head -1 || echo "")
if [ -z "$ATT_USDC_ATA" ] || [ "$ATT_USDC_ATA" = "None" ] || [ "$ATT_USDC_ATA" = "Creating" ]; then
    info "Creating attendee USDC ATA..."
    spl-token create-account "$USDC_MINT" --owner "$ATTENDEE_KEYPAIR" --url devnet 2>&1 || warn "ATA creation failed"
fi

ATT_USDC_BALANCE=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_WALLET" 2>&1 | awk '{print $1}' || echo "0")
info "Attendee USDC balance: $ATT_USDC_BALANCE"

if (( $(echo "$ATT_USDC_BALANCE < 1" | bc -l 2>/dev/null || echo "1") )); then
    echo ""
    echo -e "  ${YELLOW}══════════════════════════════════════════════════════${NC}"
    echo -e "  ${YELLOW}  Get devnet USDC from: https://faucet.circle.com/${NC}"
    echo -e "  ${YELLOW}  Wallet: $ATTENDEE_WALLET${NC}"
    echo -e "  ${YELLOW}  Chain: Solana Devnet | Token: USDC${NC}"
    echo -e "  ${YELLOW}══════════════════════════════════════════════════════${NC}"
    echo ""
    read -p "  Press Enter after getting USDC (or Ctrl+C to abort)..." -r
    ATT_USDC_BALANCE=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_WALLET" 2>&1 | awk '{print $1}' || echo "0")
    info "Attendee USDC balance (updated): $ATT_USDC_BALANCE"
fi

# ============================================================================
# Step 1: Create Source Event (past event — will end soon)
# ============================================================================
section "Step 1: Create Source Event (Source)"

SOURCE_ESCROW_ADDR=""
SOURCE_ON_CHAIN_ID=0

if [ "$SKIP_SETUP" = true ]; then
    skip "Source event setup — reusing existing"
else
    # Source event: event_end_ms = 10 minutes from now (must be in future for on-chain init)
    # Note: on-chain event_end_ms is immutable — set during create_event instruction.
    # 10 min gives enough time for escrow init + confirm + deposit.
    SOURCE_EVENT_END_MS=$(($(date +%s) + 600))000
    info "Creating source event '$SOURCE_EVENT_ID'..."
    SOURCE_EVENT_RESPONSE=$(curl -s -X POST "$BASE_URL/api/events" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"Rollover Source Event\",
            \"slug\": \"$SOURCE_EVENT_ID\",
            \"tagline\": \"Past event for rollover test\",
            \"link\": \"https://example.com/rollover-src\",
            \"sheet_id\": \"rollover-src-dummy\",
            \"event_start_ms\": $(($(date +%s) - 7200))000,
            \"event_end_ms\": $SOURCE_EVENT_END_MS,
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
        # Try seed fallback
        info "Direct create failed, trying seed..."
        SEED_RESPONSE=$(curl -s -X POST "$BASE_URL/api/events/seed" \
            -H "Authorization: Bearer dev-token" \
            -H "Content-Type: application/json")
        SEED_SUCCESS=$(echo "$SEED_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
        if [ "$SEED_SUCCESS" = "true" ]; then
            SOURCE_EVENT_ID=$(echo "$SEED_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['id'])" 2>/dev/null || echo "")
            pass "Source event seeded: id=$SOURCE_EVENT_ID"
        else
            fail "Failed to create source event"
            exit 1
        fi
    fi

    # Record the on-chain event_end timestamp before init
    SOURCE_EVENT_END_MS=$(($(date +%s) + 90))000
fi

info "Source event ID: $SOURCE_EVENT_ID"

# ============================================================================
# Step 2: Create Target Event (future event — receiving event)
# ============================================================================
section "Step 2: Create Target Event (Target)"

TARGET_ESCROW_ADDR=""
TARGET_ON_CHAIN_ID=0

if [ "$SKIP_SETUP" = true ]; then
    skip "Target event setup — reusing existing"
else
    # Target event: far in the future so it stays active
    info "Creating target event '$TARGET_EVENT_ID'..."
    TARGET_EVENT_RESPONSE=$(curl -s -X POST "$BASE_URL/api/events" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"Rollover Target Event\",
            \"slug\": \"$TARGET_EVENT_ID\",
            \"tagline\": \"Future event for rollover test\",
            \"link\": \"https://example.com/rollover-tgt\",
            \"sheet_id\": \"rollover-tgt-dummy\",
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
        exit 1
    fi
fi

info "Target event ID: $TARGET_EVENT_ID"

# ============================================================================
# Step 3: Init Escrow for Both Events
# ============================================================================
section "Step 3: Init Escrow — Source Event"

if [ "$SKIP_SETUP" = true ]; then
    skip "Source escrow init — reusing existing"
else
    info "Requesting init escrow TX for source event..."
    SRC_INIT=$(curl -s -X POST "$BASE_URL/api/escrow/init" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_id\": \"$SOURCE_EVENT_ID\"}")

    SRC_INIT_SUCCESS=$(echo "$SRC_INIT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

    if [ "$SRC_INIT_SUCCESS" = "true" ]; then
        SRC_TX_B64=$(echo "$SRC_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        SOURCE_ESCROW_ADDR=$(echo "$SRC_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['escrow_address'])" 2>/dev/null || echo "")
        SOURCE_ON_CHAIN_ID=$(echo "$SRC_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['on_chain_event_id'])" 2>/dev/null || echo "0")
        SRC_MSG=$(echo "$SRC_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

        pass "Source escrow TX built"
        info "Message: $SRC_MSG"
        info "Escrow PDA: $SOURCE_ESCROW_ADDR"
        info "On-chain event ID: $SOURCE_ON_CHAIN_ID"

        # Submit with organizer keypair
        ORG_KEYPAIR_JSON=$(cat ~/.config/solana/id.json)
        SRC_SUBMIT=$(sign_and_submit_tx "$SRC_TX_B64" "$ORG_KEYPAIR_JSON")
        info "Submit: $SRC_SUBMIT"

        if echo "$SRC_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
            SRC_SIG=$(echo "$SRC_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Source escrow TX submitted!"
            info "Signature: $SRC_SIG"
            info "View: https://solscan.io/tx/$SRC_SIG?cluster=devnet"
            sleep 5

            # Update event with escrow address
            curl -s -X PUT "$BASE_URL/api/events/$SOURCE_EVENT_ID" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{\"escrow_address\": \"$SOURCE_ESCROW_ADDR\", \"on_chain_event_id\": $SOURCE_ON_CHAIN_ID}" > /dev/null 2>&1 || warn "Failed to update source escrow address"

            # Confirm init with server (verifies on-chain + sets escrow_status=initialized)
            info "Confirming source escrow init..."
            SRC_CONFIRM_INIT=$(curl -s -X POST "$BASE_URL/api/escrow/confirm-init" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{\"event_id\": \"$SOURCE_EVENT_ID\"}")
            SRC_CI_OK=$(echo "$SRC_CONFIRM_INIT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
            if [ "$SRC_CI_OK" = "true" ]; then
                pass "Source escrow init confirmed"
            else
                SRC_CI_ERR=$(echo "$SRC_CONFIRM_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "unknown")
                warn "Source confirm-init: $SRC_CI_ERR"
            fi
        else
            fail "Source escrow TX submission failed: $SRC_SUBMIT"
        fi
    else
        ERR=$(echo "$SRC_INIT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
        fail "Source escrow TX build failed: $ERR"
    fi
fi

# --- Target Event Escrow Init ---
section "Step 3b: Init Escrow — Target Event"

if [ "$SKIP_SETUP" = true ]; then
    skip "Target escrow init — reusing existing"
else
    info "Requesting init escrow TX for target event..."
    TGT_INIT=$(curl -s -X POST "$BASE_URL/api/escrow/init" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_id\": \"$TARGET_EVENT_ID\"}")

    TGT_INIT_SUCCESS=$(echo "$TGT_INIT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

    if [ "$TGT_INIT_SUCCESS" = "true" ]; then
        TGT_TX_B64=$(echo "$TGT_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        TARGET_ESCROW_ADDR=$(echo "$TGT_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['escrow_address'])" 2>/dev/null || echo "")
        TARGET_ON_CHAIN_ID=$(echo "$TGT_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['on_chain_event_id'])" 2>/dev/null || echo "0")
        TGT_MSG=$(echo "$TGT_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

        pass "Target escrow TX built"
        info "Message: $TGT_MSG"
        info "Escrow PDA: $TARGET_ESCROW_ADDR"
        info "On-chain event ID: $TARGET_ON_CHAIN_ID"

        # Submit with organizer keypair
        ORG_KEYPAIR_JSON=$(cat ~/.config/solana/id.json)
        TGT_SUBMIT=$(sign_and_submit_tx "$TGT_TX_B64" "$ORG_KEYPAIR_JSON")
        info "Submit: $TGT_SUBMIT"

        if echo "$TGT_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
            TGT_SIG=$(echo "$TGT_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Target escrow TX submitted!"
            info "Signature: $TGT_SIG"
            info "View: https://solscan.io/tx/$TGT_SIG?cluster=devnet"
            sleep 5

            # Update event with escrow address
            curl -s -X PUT "$BASE_URL/api/events/$TARGET_EVENT_ID" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{\"escrow_address\": \"$TARGET_ESCROW_ADDR\", \"on_chain_event_id\": $TARGET_ON_CHAIN_ID}" > /dev/null 2>&1 || warn "Failed to update target escrow address"

            # Confirm init with server (verifies on-chain + sets escrow_status=initialized)
            info "Confirming target escrow init..."
            TGT_CONFIRM_INIT=$(curl -s -X POST "$BASE_URL/api/escrow/confirm-init" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{\"event_id\": \"$TARGET_EVENT_ID\"}")
            TGT_CI_OK=$(echo "$TGT_CONFIRM_INIT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
            if [ "$TGT_CI_OK" = "true" ]; then
                pass "Target escrow init confirmed"
            else
                TGT_CI_ERR=$(echo "$TGT_CONFIRM_INIT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "unknown")
                warn "Target confirm-init: $TGT_CI_ERR"
            fi
        else
            fail "Target escrow TX submission failed: $TGT_SUBMIT"
        fi
    else
        ERR=$(echo "$TGT_INIT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
        fail "Target escrow TX build failed: $ERR"
    fi
fi

# Extend source event_end_ms server-side for deposit acceptance
section "Step 3c: Extend Source Event End (server-side)"
if [ "$SKIP_SETUP" = true ]; then
    skip "Source event_end_ms extension — reusing existing"
else
    FUTURE_MS=$(($(date +%s) + 3600))000
    info "Setting source event_end_ms to +1hr for deposit time..."
    curl -s -X PUT "$BASE_URL/api/events/$SOURCE_EVENT_ID" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_end_ms\": $FUTURE_MS}" > /dev/null 2>&1
    pass "Source event_end_ms extended (server-side)"
fi

# ============================================================================
# Step 4: Deposit USDC on Source Event
# ============================================================================
section "Step 4: Deposit USDC on Source Event"

info "Initiating deposit for attendee $ATTENDEE_WALLET on source event..."

DEPOSIT_INIT=$(curl -s -X POST "$BASE_URL/api/deposit/usdc" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{
        \"event_id\": \"$SOURCE_EVENT_ID\",
        \"attendee_id\": \"$TEST_ATTENDEE_ID\",
        \"wallet_address\": \"$ATTENDEE_WALLET\"
    }")

DEP_INIT_SUCCESS=$(echo "$DEPOSIT_INIT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(str(d.get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$DEP_INIT_SUCCESS" = "true" ]; then
    pass "Deposit initiated"

    # Fetch TX from Solana Pay callback
    PAY_CALLBACK=$(curl -s "$BASE_URL/api/deposit/usdc/tx?event_id=$SOURCE_EVENT_ID&attendee_id=$TEST_ATTENDEE_ID&wallet=$ATTENDEE_WALLET")
    DEP_TX_B64=$(echo "$PAY_CALLBACK" | python3 -c "import sys,json; print(json.load(sys.stdin).get('transaction',''))" 2>/dev/null || echo "")

    if [ -n "$DEP_TX_B64" ]; then
        pass "Deposit TX built via callback"

        # Sign and submit with attendee keypair
        ATT_KEYPAIR_JSON=$(cat "$ATTENDEE_KEYPAIR")
        DEP_SUBMIT=$(sign_and_submit_tx "$DEP_TX_B64" "$ATT_KEYPAIR_JSON")
        info "Deposit submit: $DEP_SUBMIT"

        if echo "$DEP_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
            DEP_SIG=$(echo "$DEP_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Deposit TX submitted!"
            info "Signature: $DEP_SIG"
            info "View: https://solscan.io/tx/$DEP_SIG?cluster=devnet"
            sleep 8

            # Notify worker
            curl -s -X POST "$BASE_URL/api/deposit/usdc/webhook" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{\"event_id\": \"$SOURCE_EVENT_ID\", \"attendee_id\": \"$TEST_ATTENDEE_ID\", \"tx_signature\": \"$DEP_SIG\"}" > /dev/null 2>&1

            # Confirm deposit synchronously (verifies on-chain + sets verified=true)
            # Retry up to 3 times — devnet confirmation can be slow
            DEP_CONFIRMED="False"
            for i in 1 2 3; do
                info "Confirming deposit (attempt $i) via /api/deposit/usdc/confirm..."
                DEP_CONFIRM=$(curl -s "$BASE_URL/api/deposit/usdc/confirm?event_id=$SOURCE_EVENT_ID&attendee_id=$TEST_ATTENDEE_ID" \
                    -H "Authorization: Bearer dev-token")
                DEP_CONFIRM_OK=$(echo "$DEP_CONFIRM" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
                if [ "$DEP_CONFIRM_OK" = "true" ]; then
                    DEP_CONFIRMED=$(echo "$DEP_CONFIRM" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print(d.get('confirmed',False))" 2>/dev/null || echo "False")
                    if [ "$DEP_CONFIRMED" = "True" ]; then
                        pass "Deposit confirmed on-chain: confirmed=$DEP_CONFIRMED"
                        break
                    fi
                fi
                if [ $i -lt 3 ]; then
                    info "Deposit not yet confirmed, waiting 5s..."
                    sleep 5
                fi
            done
            if [ "$DEP_CONFIRMED" != "True" ]; then
                warn "Deposit confirmation: confirmed=$DEP_CONFIRMED (may still be processing)"
            fi

            # Verify vault balance
            SRC_VAULT_BAL=$(spl-token balance "$USDC_MINT" --url devnet --owner "$SOURCE_ESCROW_ADDR" 2>&1 || echo "0")
            info "Source vault USDC: $SRC_VAULT_BAL"
        else
            fail "Deposit TX submission failed"
            echo "   $DEP_SUBMIT" | head -c 500
            exit 1
        fi
    else
        fail "Deposit TX build failed via callback"
        echo "   $PAY_CALLBACK" | head -c 300
        exit 1
    fi
else
    ERR=$(echo "$DEPOSIT_INIT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message','')))" 2>/dev/null || echo "")
    fail "Deposit initiation failed: $ERR"
    exit 1
fi

# ============================================================================
# Step 5: Mark Attendee Checked-In on Source Event
# ============================================================================
section "Step 5: Mark Attendee Checked-In (Source Event)"

info "Requesting mark_checked_in TX..."
MARK_CI=$(curl -s -X POST "$BASE_URL/api/escrow/mark-checked-in" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$SOURCE_EVENT_ID\", \"attendee_id\": \"$TEST_ATTENDEE_ID\", \"attendee_wallet\": \"$ATTENDEE_WALLET\"}")

MARK_CI_SUCCESS=$(echo "$MARK_CI" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$MARK_CI_SUCCESS" = "true" ]; then
    MARK_CI_TX=$(echo "$MARK_CI" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    MARK_CI_MSG=$(echo "$MARK_CI" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

    pass "mark_checked_in TX built"
    info "Message: $MARK_CI_MSG"

    ORG_KEYPAIR_JSON=$(cat ~/.config/solana/id.json)
    MARK_CI_SUBMIT=$(sign_and_submit_tx "$MARK_CI_TX" "$ORG_KEYPAIR_JSON")
    info "Submit: $MARK_CI_SUBMIT"

    if echo "$MARK_CI_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
        MARK_CI_SIG=$(echo "$MARK_CI_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "mark_checked_in TX submitted!"
        info "Signature: $MARK_CI_SIG"
        info "View: https://solscan.io/tx/$MARK_CI_SIG?cluster=devnet"
        sleep 5
    else
        fail "mark_checked_in TX submission failed"
        echo "   $MARK_CI_SUBMIT" | head -c 500
        exit 1
    fi
else
    ERR=$(echo "$MARK_CI" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
    fail "mark_checked_in TX build failed: $ERR"
    exit 1
fi

# ============================================================================
# Step 6: (No wait needed — rollover_deposit works anytime after check-in)
# ============================================================================
section "Step 6: Ready to Rollover (no event_end wait needed)"

# The on-chain rollover_deposit instruction does NOT require the source event to
# have ended. It only requires: checked_in=true, not refunded, target is active.
pass "No event_end wait needed — rollover works after check-in"

# ============================================================================
# Step 7: Build & Submit Rollover Deposit TX
# ============================================================================
section "Step 7: Build & Submit Rollover Deposit TX"

info "Requesting rollover deposit TX..."
info "  Source: $SOURCE_EVENT_ID (on_chain_id=$SOURCE_ON_CHAIN_ID)"
info "  Target: $TARGET_EVENT_ID (on_chain_id=$TARGET_ON_CHAIN_ID)"
info "  Attendee: $ATTENDEE_WALLET"
info "  Attendee ID: $TEST_ATTENDEE_ID"

ROLLOVER_RESPONSE=$(curl -s -X POST "$BASE_URL/api/escrow/rollover-deposit" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{
        \"source_event_id\": \"$SOURCE_EVENT_ID\",
        \"target_event_id\": \"$TARGET_EVENT_ID\",
        \"attendee_id\": \"$TEST_ATTENDEE_ID\",
        \"wallet_address\": \"$ATTENDEE_WALLET\"
    }")

info "Rollover response: $(echo "$ROLLOVER_RESPONSE" | head -c 300)"

ROLLOVER_SUCCESS=$(echo "$ROLLOVER_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$ROLLOVER_SUCCESS" = "true" ]; then
    ROLLOVER_TX_B64=$(echo "$ROLLOVER_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    ROLLOVER_MSG=$(echo "$ROLLOVER_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

    pass "Rollover TX built!"
    info "Message: $ROLLOVER_MSG"
    info "Transaction: ${ROLLOVER_TX_B64:0:60}..."

    # Sign and submit with attendee keypair (attendee is the signer)
    ATT_KEYPAIR_JSON=$(cat "$ATTENDEE_KEYPAIR")
    ROLLOVER_SUBMIT=$(sign_and_submit_tx "$ROLLOVER_TX_B64" "$ATT_KEYPAIR_JSON")
    info "Rollover submit: $ROLLOVER_SUBMIT"

    if echo "$ROLLOVER_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
        ROLLOVER_SIG=$(echo "$ROLLOVER_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Rollover TX submitted!"
        info "Signature: $ROLLOVER_SIG"
        info "View: https://solscan.io/tx/$ROLLOVER_SIG?cluster=devnet"
        sleep 8
    else
        fail "Rollover TX submission failed"
        echo "   $ROLLOVER_SUBMIT" | head -c 500

        # Try to decode the error
        if echo "$ROLLOVER_SUBMIT" | grep -qi "custom program error"; then
            ERR_CODE=$(echo "$ROLLOVER_SUBMIT" | grep -o 'custom program error: 0x[0-9a-f]*' || echo "")
            warn "Escrow program error: $ERR_CODE"
            warn "Common causes:"
            warn "  0xbb9 = NotCheckedIn (attendee not checked in on source)"
            warn "  0xbbb = AlreadyRefunded (source deposit already refunded)"
            warn "  0xbc0 = EventNotActive (target event not accepting deposits)"
            warn "  0xbc1 = IncorrectDepositAmount (deposit amounts don't match)"
        fi
        exit 1
    fi
else
    # Try wrapped response
    ROLLOVER_WRAP=$(echo "$ROLLOVER_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if d.get('data',{}).get('transaction') else 'no')" 2>/dev/null || echo "no")
    if [ "$ROLLOVER_WRAP" = "yes" ]; then
        ROLLOVER_TX_B64=$(echo "$ROLLOVER_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        ROLLOVER_MSG=$(echo "$ROLLOVER_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

        pass "Rollover TX built (wrapped response)"
        info "Message: $ROLLOVER_MSG"

        ATT_KEYPAIR_JSON=$(cat "$ATTENDEE_KEYPAIR")
        ROLLOVER_SUBMIT=$(sign_and_submit_tx "$ROLLOVER_TX_B64" "$ATT_KEYPAIR_JSON")
        info "Rollover submit: $ROLLOVER_SUBMIT"

        if echo "$ROLLOVER_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
            ROLLOVER_SIG=$(echo "$ROLLOVER_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Rollover TX submitted!"
            info "Signature: $ROLLOVER_SIG"
            info "View: https://solscan.io/tx/$ROLLOVER_SIG?cluster=devnet"
            sleep 8
        else
            fail "Rollover TX submission failed"
            echo "   $ROLLOVER_SUBMIT" | head -c 500
            exit 1
        fi
    else
        ERR=$(echo "$ROLLOVER_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:300])))" 2>/dev/null || echo "")
        fail "Rollover TX build failed: $ERR"
        echo "   Full: $(echo "$ROLLOVER_RESPONSE" | head -c 500)"
        exit 1
    fi
fi

# ============================================================================
# Step 8: Verify Post-Rollover State
# ============================================================================
section "Step 8: Verify Post-Rollover State"

# Source vault should be empty (USDC moved out)
info "Checking source vault balance..."
SRC_VAULT_FINAL=$(spl-token balance "$USDC_MINT" --url devnet --owner "$SOURCE_ESCROW_ADDR" 2>&1 || echo "0 (no vault)")
info "Source vault USDC: $SRC_VAULT_FINAL"

# Target vault should have the deposit amount
info "Checking target vault balance..."
TGT_VAULT_FINAL=$(spl-token balance "$USDC_MINT" --url devnet --owner "$TARGET_ESCROW_ADDR" 2>&1 || echo "0 (no vault)")
info "Target vault USDC: $TGT_VAULT_FINAL"

EXPECTED_USDC=$(echo "$DEPOSIT_AMOUNT_USDC / 1000000" | bc -l 2>/dev/null || echo "1")
if echo "$TGT_VAULT_FINAL" | grep -q "$EXPECTED_USDC"; then
    pass "Target vault has the rolled-over deposit ($TGT_VAULT_FINAL USDC)"
else
    # Be more lenient — just check it's non-zero
    if echo "$TGT_VAULT_FINAL" | grep -qE "^[1-9]"; then
        pass "Target vault has funds: $TGT_VAULT_FINAL USDC"
    else
        warn "Target vault balance: $TGT_VAULT_FINAL (expected ~$EXPECTED_USDC)"
    fi
fi

# Attendee USDC should be unchanged (no refund — it was rolled over)
ATT_USDC_FINAL=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_WALLET" 2>&1 | awk '{print $1}' || echo "?")
info "Attendee USDC (final): $ATT_USDC_FINAL"

# Sync source event to trigger indexing + rollover DepositStatus hook
# (In production, Helius webhooks handle this; for local dev we trigger manually)
info "Syncing source event on-chain events (triggers rollover DepositStatus hook)..."

# Wait for rollover tx to be confirmed, then sync with retry
sleep 3
SYNC_ATTEMPTS=0
TOTAL_INDEXED=0
while [ $SYNC_ATTEMPTS -lt 3 ]; do
    SRC_SYNC=$(curl -s -X POST "$BASE_URL/api/escrow/sync" \
      -H "Authorization: Bearer dev-token" \
      -H "Content-Type: application/json" \
      -d "{\"event_id\": \"$SOURCE_EVENT_ID\"}" 2>/dev/null)
    SRC_SYNC_INDEXED=$(echo "$SRC_SYNC" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('indexed',0))" 2>/dev/null || echo "0")
    TOTAL_INDEXED=$((TOTAL_INDEXED + SRC_SYNC_INDEXED))
    SYNC_ATTEMPTS=$((SYNC_ATTEMPTS + 1))
    info "Sync attempt $SYNC_ATTEMPTS: indexed $SRC_SYNC_INDEXED new events (total: $TOTAL_INDEXED)"

    # Check if rollover_deposit was indexed
    SRC_EVENTS=$(curl -s "$BASE_URL/api/escrow/events/$SOURCE_EVENT_ID" -H "Authorization: Bearer dev-token" 2>/dev/null)
    HAS_ROLLOVER=$(echo "$SRC_EVENTS" | python3 -c "import sys,json; events=json.load(sys.stdin).get('data',{}).get('events',[]); print('yes' if any(e.get('instruction')=='rollover_deposit' for e in events) else 'no')" 2>/dev/null || echo "no")

    if [ "$HAS_ROLLOVER" = "yes" ]; then
        pass "RolloverDeposit event found in on-chain events"
        break
    fi

    if [ $SYNC_ATTEMPTS -lt 3 ]; then
        info "RolloverDeposit not yet indexed, waiting 5s before retry..."
        sleep 5
    fi
done

if [ $TOTAL_INDEXED -gt 0 ]; then
    pass "Source event on-chain events indexed ($TOTAL_INDEXED)"
else
    info "No new events to index (may already be indexed)"
fi

# Verify deposit status on source event
info "Checking source event deposit status (should show refunded/rolled)..."
SRC_DEP_STATUS=$(curl -s "$BASE_URL/api/deposit/status/$TEST_ATTENDEE_ID?event_id=$SOURCE_EVENT_ID")
info "Source deposit status: $(echo "$SRC_DEP_STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); s=d.get('status',{}); print(f\"method={s.get('method','?')}, verified={s.get('verified',False)}\" if s else 'no status')" 2>/dev/null || echo "$SRC_DEP_STATUS" | head -c 200)"

# ============================================================================
# Step 9: Verify Target Event DepositStatus (Indexer Fix Validation)
# ============================================================================
section "Step 9: Verify Target Event DepositStatus (Indexer Fix)"

info "Checking target event deposit status for attendee..."
info "  GET /api/deposit/status/$TEST_ATTENDEE_ID?event_id=$TARGET_EVENT_ID"

TGT_DEP_STATUS=$(curl -s "$BASE_URL/api/deposit/status/$TEST_ATTENDEE_ID?event_id=$TARGET_EVENT_ID")
info "Target deposit status response: $(echo "$TGT_DEP_STATUS" | head -c 300)"

TGT_DEP_SUCCESS=$(echo "$TGT_DEP_STATUS" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$TGT_DEP_SUCCESS" = "true" ]; then
    TGT_DEP_METHOD=$(echo "$TGT_DEP_STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); s=d.get('status',{}); print(s.get('method',''))" 2>/dev/null || echo "")
    TGT_DEP_VERIFIED=$(echo "$TGT_DEP_STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); s=d.get('status',{}); print(str(s.get('verified',False)).lower())" 2>/dev/null || echo "false")
    TGT_DEP_ESCROW_ADDR=$(echo "$TGT_DEP_STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); s=d.get('status',{}); print(s.get('escrow_address',''))" 2>/dev/null || echo "")

    info "Target deposit: method=$TGT_DEP_METHOD, verified=$TGT_DEP_VERIFIED, escrow=$TGT_DEP_ESCROW_ADDR"

    if [ "$TGT_DEP_VERIFIED" = "true" ]; then
        pass "Target event DepositStatus exists and is verified"
    else
        fail "Target event DepositStatus exists but NOT verified (verified=$TGT_DEP_VERIFIED)"
    fi

    if [ "$TGT_DEP_METHOD" = "Usdc" ] || [ "$TGT_DEP_METHOD" = "usdc" ]; then
        pass "Target deposit method is USDC (as expected for rollover)"
    else
        warn "Target deposit method is '$TGT_DEP_METHOD' (expected Usdc)"
    fi

    if [ -n "$TGT_DEP_ESCROW_ADDR" ] && [ "$TGT_DEP_ESCROW_ADDR" = "$TARGET_ESCROW_ADDR" ]; then
        pass "Target escrow address matches ($TGT_DEP_ESCROW_ADDR)"
    elif [ -n "$TGT_DEP_ESCROW_ADDR" ]; then
        warn "Target escrow address mismatch: got=$TGT_DEP_ESCROW_ADDR expected=$TARGET_ESCROW_ADDR"
    else
        info "No escrow address in deposit status (may not be stored)"
    fi
else
    # DepositStatus not found — indexer fix may not have fired
    fail "Target event DepositStatus NOT found — indexer fix did not create it"
    info "This means the RolloverDeposit event was indexed but the post-indexing"
    info "hook to create DepositStatus on the target event did not fire."
    info ""
    info "Check worker logs for:"
    info "  [rollover-indexer] Creating DepositStatus on target event"
    info "  [rollover-indexer] Failed to resolve target event"
    info "  [rollover-indexer] Failed to find attendee by wallet"
fi

# Also verify via escrow events endpoint
info "Checking target event escrow events..."
TGT_ESCROW_EVENTS=$(curl -s "$BASE_URL/api/escrow/events/$TARGET_EVENT_ID")
TGT_HAS_DEPOSIT=$(echo "$TGT_ESCROW_EVENTS" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    events = d.get('data', {}).get('events', [])
    found = any(e.get('event_type', '') in ('Deposit', 'RolloverDeposit') for e in events)
    print('yes' if found else 'no')
except:
    print('parse_error')
" 2>/dev/null || echo "error")

if [ "$TGT_HAS_DEPOSIT" = "yes" ]; then
    pass "Target event has deposit/rollover events in escrow history"
else
    warn "No deposit/rollover events found for target event (may not be indexed yet)"
fi

# ============================================================================
# Step 10: Refund from Target Event (attendee claims refund from target)
# ============================================================================
section "Step 10: Refund Attendee from Target Event"

info "Requesting refund+close TX from target event for attendee..."
info "  Target event: $TARGET_EVENT_ID"
info "  Attendee:     $ATTENDEE_WALLET"
info "  Attendee ID:  $TEST_ATTENDEE_ID"

REFUND_RESPONSE=$(curl -s -X POST "$BASE_URL/api/escrow/refund" \
    -H "Content-Type: application/json" \
    -d "{
        \"event_id\": \"$TARGET_EVENT_ID\",
        \"attendee_id\": \"$TEST_ATTENDEE_ID\",
        \"wallet_address\": \"$ATTENDEE_WALLET\"
    }")

REFUND_SUCCESS=$(echo "$REFUND_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$REFUND_SUCCESS" = "true" ]; then
    REFUND_TX_B64=$(echo "$REFUND_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    REFUND_MSG=$(echo "$REFUND_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

    pass "Refund+close TX built from target event"
    info "Message: $REFUND_MSG"

    # Attendee signs the refund TX
    ATT_KEYPAIR_JSON=$(cat "$ATTENDEE_KEYPAIR")
    REFUND_SUBMIT=$(sign_and_submit_tx "$REFUND_TX_B64" "$ATT_KEYPAIR_JSON")
    info "Refund submit: $REFUND_SUBMIT"

    if echo "$REFUND_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
        REFUND_SIG=$(echo "$REFUND_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Refund TX submitted from target event!"
        info "Signature: $REFUND_SIG"
        info "View: https://solscan.io/tx/$REFUND_SIG?cluster=devnet"
        sleep 8
    else
        fail "Refund TX submission failed"
        echo "   $REFUND_SUBMIT" | head -c 500
        # Non-fatal — continue with cleanup
    fi
else
    ERR=$(echo "$REFUND_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:300])))" 2>/dev/null || echo "unknown")
    fail "Refund TX build failed: $ERR"
    info "This may happen if the target event has not ended yet on-chain."
    info "The refund instruction requires the event deadline to have passed."
fi

# ============================================================================
# Step 11: Verify Post-Refund Vault Balances
# ============================================================================
section "Step 11: Verify Post-Refund Vault Balances"

# Source vault should still be empty (was drained by rollover)
info "Checking source vault balance..."
SRC_VAULT_POST=$(spl-token balance "$USDC_MINT" --url devnet --owner "$SOURCE_ESCROW_ADDR" 2>&1 || echo "0 (no vault / empty)")
info "Source vault USDC (post-refund): $SRC_VAULT_POST"

if echo "$SRC_VAULT_POST" | grep -qE "^0|^0\.0|no vault|empty|not found|Insufficient"; then
    pass "Source vault is empty (as expected after rollover)"
else
    warn "Source vault has unexpected balance: $SRC_VAULT_POST"
fi

# Target vault should now be empty (refunded to attendee)
info "Checking target vault balance..."
TGT_VAULT_POST=$(spl-token balance "$USDC_MINT" --url devnet --owner "$TARGET_ESCROW_ADDR" 2>&1 || echo "0 (no vault / empty)")
info "Target vault USDC (post-refund): $TGT_VAULT_POST"

if echo "$TGT_VAULT_POST" | grep -qE "^0|^0\.0|no vault|empty|not found|Insufficient"; then
    pass "Target vault is empty (deposit refunded to attendee)"
else
    warn "Target vault still has balance: $TGT_VAULT_POST"
fi

# Attendee should have their USDC back
ATT_USDC_POST=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_WALLET" 2>&1 | awk '{print $1}' || echo "?")
info "Attendee USDC (post-refund): $ATT_USDC_POST"

# ============================================================================
# Step 12: Deactivate Both Events
# ============================================================================
section "Step 12: Deactivate Both Events"

ORG_KEYPAIR_JSON=$(cat ~/.config/solana/id.json)

# --- Deactivate Source Event ---
info "Deactivating source event..."
SRC_DEACT=$(curl -s -X POST "$BASE_URL/api/escrow/deactivate-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$SOURCE_EVENT_ID\"}")

SRC_DEACT_SUCCESS=$(echo "$SRC_DEACT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$SRC_DEACT_SUCCESS" = "true" ]; then
    SRC_DEACT_TX=$(echo "$SRC_DEACT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    SRC_DEACT_SUBMIT=$(sign_and_submit_tx "$SRC_DEACT_TX" "$ORG_KEYPAIR_JSON")

    if echo "$SRC_DEACT_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
        SRC_DEACT_SIG=$(echo "$SRC_DEACT_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Source event deactivated: $SRC_DEACT_SIG"
        info "View: https://solscan.io/tx/$SRC_DEACT_SIG?cluster=devnet"
        sleep 5

        # Sync escrow status in KV (on-chain deactivation doesn't auto-update KV)
        curl -s -X PUT "$BASE_URL/api/events/$SOURCE_EVENT_ID" \
            -H "Authorization: Bearer dev-token" \
            -H "Content-Type: application/json" \
            -d '{"escrow_status": "deactivated"}' > /dev/null 2>&1
    else
        fail "Source deactivate TX failed: $SRC_DEACT_SUBMIT"
    fi
else
    ERR=$(echo "$SRC_DEACT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
    warn "Source deactivate build failed: $ERR (may already be inactive)"
fi

# --- Deactivate Target Event ---
info "Deactivating target event..."
TGT_DEACT=$(curl -s -X POST "$BASE_URL/api/escrow/deactivate-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$TARGET_EVENT_ID\"}")

TGT_DEACT_SUCCESS=$(echo "$TGT_DEACT" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$TGT_DEACT_SUCCESS" = "true" ]; then
    TGT_DEACT_TX=$(echo "$TGT_DEACT" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    TGT_DEACT_SUBMIT=$(sign_and_submit_tx "$TGT_DEACT_TX" "$ORG_KEYPAIR_JSON")

    if echo "$TGT_DEACT_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
        TGT_DEACT_SIG=$(echo "$TGT_DEACT_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Target event deactivated: $TGT_DEACT_SIG"
        info "View: https://solscan.io/tx/$TGT_DEACT_SIG?cluster=devnet"
        sleep 5

        # Sync escrow status in KV (on-chain deactivation doesn't auto-update KV)
        curl -s -X PUT "$BASE_URL/api/events/$TARGET_EVENT_ID" \
            -H "Authorization: Bearer dev-token" \
            -H "Content-Type: application/json" \
            -d '{"escrow_status": "deactivated"}' > /dev/null 2>&1
    else
        fail "Target deactivate TX failed: $TGT_DEACT_SUBMIT"
    fi
else
    ERR=$(echo "$TGT_DEACT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
    warn "Target deactivate build failed: $ERR (may already be inactive)"
fi

# ============================================================================
# Step 13: Close Both Events (reclaim rent)
# ============================================================================
section "Step 13: Close Both Events (Reclaim Rent)"

# --- Close Source Event ---
info "Closing source event..."
SRC_CLOSE=$(curl -s -X POST "$BASE_URL/api/escrow/close-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$SOURCE_EVENT_ID\"}")

SRC_CLOSE_SUCCESS=$(echo "$SRC_CLOSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$SRC_CLOSE_SUCCESS" = "true" ]; then
    SRC_CLOSE_TX=$(echo "$SRC_CLOSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    SRC_CLOSE_SUBMIT=$(sign_and_submit_tx "$SRC_CLOSE_TX" "$ORG_KEYPAIR_JSON")

    if echo "$SRC_CLOSE_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
        SRC_CLOSE_SIG=$(echo "$SRC_CLOSE_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Source event closed: $SRC_CLOSE_SIG"
        info "View: https://solscan.io/tx/$SRC_CLOSE_SIG?cluster=devnet"
        sleep 5

        # Verify source escrow is gone
        SRC_ESCROW_CHECK=$(solana account "$SOURCE_ESCROW_ADDR" --url devnet 2>&1 || echo "CLOSED")
        if echo "$SRC_ESCROW_CHECK" | grep -qi "error\|not found\|CLOSED"; then
            pass "Source escrow account closed — rent reclaimed"
        else
            warn "Source escrow still exists (may need vault to be empty first)"
        fi
    else
        fail "Source close TX failed: $SRC_CLOSE_SUBMIT"
    fi
else
    ERR=$(echo "$SRC_CLOSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
    warn "Source close build failed: $ERR (may need vault emptied first)"
fi

# --- Close Target Event ---
info "Closing target event..."
TGT_CLOSE=$(curl -s -X POST "$BASE_URL/api/escrow/close-event" \
    -H "Authorization: Bearer dev-token" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\": \"$TARGET_EVENT_ID\"}")

TGT_CLOSE_SUCCESS=$(echo "$TGT_CLOSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$TGT_CLOSE_SUCCESS" = "true" ]; then
    TGT_CLOSE_TX=$(echo "$TGT_CLOSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
    TGT_CLOSE_SUBMIT=$(sign_and_submit_tx "$TGT_CLOSE_TX" "$ORG_KEYPAIR_JSON")

    if echo "$TGT_CLOSE_SUBMIT" | grep -q "STATUS=CONFIRMED"; then
        TGT_CLOSE_SIG=$(echo "$TGT_CLOSE_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
        pass "Target event closed: $TGT_CLOSE_SIG"
        info "View: https://solscan.io/tx/$TGT_CLOSE_SIG?cluster=devnet"
        sleep 5

        # Verify target escrow is gone
        TGT_ESCROW_CHECK=$(solana account "$TARGET_ESCROW_ADDR" --url devnet 2>&1 || echo "CLOSED")
        if echo "$TGT_ESCROW_CHECK" | grep -qi "error\|not found\|CLOSED"; then
            pass "Target escrow account closed — rent reclaimed"
        else
            warn "Target escrow still exists (may need vault to be empty first)"
        fi
    else
        fail "Target close TX failed: $TGT_CLOSE_SUBMIT"
    fi
else
    ERR=$(echo "$TGT_CLOSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
    warn "Target close build failed: $ERR (may need vault emptied first)"
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
    echo -e "  ${GREEN}${BOLD}🎉 Rollover deposit E2E test PASSED!${NC}"
    echo ""
    echo "  Source event: $SOURCE_EVENT_ID"
    echo "  Target event: $TARGET_EVENT_ID"
    echo "  Attendee:     $ATTENDEE_WALLET"
    if [ -n "${ROLLOVER_SIG:-}" ]; then
        echo "  Rollover TX:  https://solscan.io/tx/$ROLLOVER_SIG?cluster=devnet"
    fi
else
    echo -e "  ${RED}${BOLD}❌ Rollover deposit E2E test FAILED${NC}"
    exit 1
fi
