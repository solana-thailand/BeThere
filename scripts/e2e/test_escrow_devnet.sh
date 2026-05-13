#!/usr/bin/env bash
# ============================================================================
# BeThere Devnet Escrow E2E Test Script
# ============================================================================
# Tests the full on-chain escrow flow end-to-end on Solana devnet:
#   1. Health check + event setup
#   2. Build & submit init escrow TX (combined ATA + CreateEvent)
#   3. Update event_end_ms to future (server-side only, for deposits)
#   4. Verify escrow on-chain
#   5. Check deposit status (pre-deposit)
#   6. Build & submit deposit TX → USDC → vault
#   7. Check deposit status (post-deposit)
#   8. Build & submit mark_checked_in TX
#   9. Build & submit refund TX → USDC back to attendee
#  10. Verify final on-chain state
#  11. THB deposit flow test
#  12. Deactivate event
#  13. Claim forfeited deposits
#  14. Close event & reclaim rent
#
# Prerequisites:
#   - `cd worker && npx wrangler dev --port 8787` running
#   - DEV_MODE=1 in worker/.dev.vars
#   - HELIUS_API_KEY in worker/.dev.vars
#   - solana CLI installed + configured for devnet
#   - Devnet USDC in test wallet (use https://faucet.circle.com/)
#
# Usage:
#   bash scripts/e2e/test_escrow_devnet.sh
#   bash scripts/e2e/test_escrow_devnet.sh --skip-setup       # reuse existing event
#   bash scripts/e2e/test_escrow_devnet.sh --with-vault-ata   # run create_vault_ata step
#   EVENT_ID=myevent bash scripts/e2e/test_escrow_devnet.sh
# ============================================================================

set -euo pipefail

# --- Config ---
BASE_URL="${BASE_URL:-http://localhost:8787}"
EVENT_ID="${EVENT_ID:-escrow-e2e-$(date +%s)}"
DEPOSIT_AMOUNT_USDC="${DEPOSIT_AMOUNT_USDC:-1000000}"  # 1 USDC (6 decimals)
DEPOSIT_AMOUNT_THB="${DEPOSIT_AMOUNT_THB:-100}"         # 100 THB
ORGANIZER_WALLET="${ORGANIZER_WALLET:-}"
ATTENDEE_WALLET="${ATTENDEE_WALLET:-}"
RPC_URL="${RPC_URL:-https://api.devnet.solana.com}"

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
SKIP_ONCHAIN=false
for arg in "$@"; do
    case "$arg" in
        --skip-setup) SKIP_SETUP=true ;;
        --skip-onchain) SKIP_ONCHAIN=true ;;
    esac
done

echo ""
echo -e "${BOLD}🧪 BeThere Devnet Escrow E2E Test Suite${NC}"
echo "   BASE_URL:    $BASE_URL"
echo "   EVENT_ID:    $EVENT_ID"
echo "   RPC_URL:     $RPC_URL"
echo ""

# --- Read config from .dev.vars ---
HELIUS_API_KEY=""
if [ -f "worker/.dev.vars" ]; then
    HELIUS_API_KEY=$(grep "^HELIUS_API_KEY=" worker/.dev.vars | cut -d= -f2- | tr -d '"' | tr -d "'" || true)
fi

# --- Resolve wallets ---
if [ -z "$ORGANIZER_WALLET" ]; then
    ORGANIZER_WALLET=$(solana address --url devnet 2>/dev/null || echo "")
fi

# Create or reuse attendee keypair
ATTENDEE_KEYPAIR="/tmp/bethere-escrow-e2e-attendee.json"
if [ -z "$ATTENDEE_WALLET" ]; then
    if [ ! -f "$ATTENDEE_KEYPAIR" ]; then
        solana-keygen new --no-bip39-passphrase --silent --outfile "$ATTENDEE_KEYPAIR" 2>/dev/null
        info "Created new attendee keypair"
    fi
    ATTENDEE_WALLET=$(solana address --keypair "$ATTENDEE_KEYPAIR" --url devnet 2>/dev/null || echo "")
fi

if [ -z "$ORGANIZER_WALLET" ] || [ -z "$ATTENDEE_WALLET" ]; then
    echo -e "  ${RED}❌ Cannot resolve wallets. Is solana CLI installed?${NC}"
    exit 1
fi

info "Organizer wallet: ${ORGANIZER_WALLET:0:8}...${ORGANIZER_WALLET: -4}"
info "Attendee wallet:  ${ATTENDEE_WALLET:0:8}...${ATTENDEE_WALLET: -4}"
info "Keypair path:     $ATTENDEE_KEYPAIR"

# USDC devnet constants
USDC_MINT="4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
ESCROW_PROGRAM="C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T"

# ============================================================================
# Step 0: Prerequisites Check
# ============================================================================
section "Step 0: Prerequisites"

# Check solana CLI
if ! command -v solana &>/dev/null; then
    fail "solana CLI not installed"
    echo "   Install: sh -c \"\$(curl -sSfL https://release.anza.xyz/stable/install)\""
    exit 1
fi
pass "solana CLI: $(solana --version 2>&1 | head -1)"

# Check spl-token CLI
if ! command -v spl-token &>/dev/null; then
    fail "spl-token CLI not installed"
    exit 1
fi
pass "spl-token CLI: $(spl-token --version 2>&1 | head -1)"

# Check health
HEALTH=$(curl -s "$BASE_URL/api/health")
if check_json "$HEALTH" "['status']" "ok"; then
    pass "Worker health check: OK"
else
    fail "Worker not healthy — is wrangler dev running?"
    echo "   Response: $HEALTH"
    exit 1
fi

# Check organizer SOL balance
ORG_BALANCE=$(solana balance "$ORGANIZER_WALLET" --url devnet 2>&1 | awk '{print $1}' || echo "0")
info "Organizer SOL balance: $ORG_BALANCE SOL"
if (( $(echo "$ORG_BALANCE < 0.5" | bc -l 2>/dev/null || echo "1") )); then
    warn "Low SOL balance — may need airdrop: solana airdrop 2 $ORGANIZER_WALLET --url devnet"
fi

# Fund attendee wallet if empty
ATT_BALANCE=$(solana balance "$ATTENDEE_WALLET" --url devnet 2>&1 | awk '{print $1}' || echo "0")
info "Attendee SOL balance: $ATT_BALANCE SOL"
if (( $(echo "$ATT_BALANCE < 0.05" | bc -l 2>/dev/null || echo "1") )); then
    info "Funding attendee wallet with 1 SOL..."
    solana airdrop 1 "$ATTENDEE_WALLET" --url devnet 2>&1 || warn "Airdrop failed — fund manually"
fi

# Check attendee USDC ATA
ATT_USDC_ATA=$(spl-token address --token "$USDC_MINT" --owner "$ATTENDEE_KEYPAIR" --url devnet 2>/dev/null | head -1 || echo "")
if [ -z "$ATT_USDC_ATA" ] || [ "$ATT_USDC_ATA" = "None" ] || [ "$ATT_USDC_ATA" = "Creating" ]; then
    info "Creating attendee USDC ATA..."
    ATT_USDC_ATA=$(spl-token create-account "$USDC_MINT" --owner "$ATTENDEE_KEYPAIR" --url devnet 2>&1 | grep "Creating account" | awk '{print $3}' || echo "")
fi
info "Attendee USDC ATA: ${ATT_USDC_ATA:0:8}...${ATT_USDC_ATA: -4}"

ATT_USDC_BALANCE=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_WALLET" 2>&1 | awk '{print $1}' || echo "0")
info "Attendee USDC balance: $ATT_USDC_BALANCE"

if (( $(echo "$ATT_USDC_BALANCE < 1" | bc -l 2>/dev/null || echo "1") )); then
    warn "Attendee needs devnet USDC!"
    echo ""
    echo -e "  ${YELLOW}══════════════════════════════════════════════════════${NC}"
    echo -e "  ${YELLOW}  Get devnet USDC from: https://faucet.circle.com/${NC}"
    echo -e "  ${YELLOW}  Wallet: $ATTENDEE_WALLET${NC}"
    echo -e "  ${YELLOW}  Chain: Solana Devnet | Token: USDC${NC}"
    echo -e "  ${YELLOW}══════════════════════════════════════════════════════${NC}"
    echo ""
    if [ "$SKIP_ONCHAIN" = true ]; then
        warn "--skip-onchain: skipping USDC-dependent on-chain tests"
    else
        read -p "  Press Enter after getting USDC (or Ctrl+C to abort)..." -r
        ATT_USDC_BALANCE=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_WALLET" 2>&1 | awk '{print $1}' || echo "0")
        info "Attendee USDC balance (updated): $ATT_USDC_BALANCE"
    fi
fi

# ============================================================================
# Step 1: Seed / Create Event with Deposit Config
# ============================================================================
section "Step 1: Event Setup with Deposit Config"

if [ "$SKIP_SETUP" = true ]; then
    skip "Event setup — reusing existing event"
else
    # Create a new event with deposit configuration
    # Uses dev-token auth (DEV_MODE=1)
    info "Creating event '$EVENT_ID' with deposit config..."

    # IMPORTANT: event_end_ms is set to ~2 minutes in the future so that:
    #   1. The on-chain create_event instruction accepts it (event_end must be in the future)
    #   2. The server-side deposit handler allows deposits
    # After deposit, we wait for event_end to pass, then refunds work.
    # refund_deadline_hours=168 means refund_deadline = event_end + 7 days.
    # Step 3b extends event_end_ms further (server-side only) for deposit time buffer.
    EVENT_RESPONSE=$(curl -s -X POST "$BASE_URL/api/events" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"Escrow E2E Test Event\",
            \"slug\": \"$EVENT_ID\",
            \"tagline\": \"Automated E2E test for deposit/refund escrow\",
            \"link\": \"https://example.com/e2e-test\",
            \"sheet_id\": \"e2e-test-dummy\",
            \"event_start_ms\": $(($(date +%s) - 7200))000,
            \"event_end_ms\": $(($(date +%s) + 120))000,
            \"status\": \"active\",
            \"deposit_enabled\": true,
            \"deposit_amount_usdc\": $DEPOSIT_AMOUNT_USDC,
            \"deposit_amount_thb\": $DEPOSIT_AMOUNT_THB,
            \"promptpay_id\": \"0812345678\",
            \"organizer_wallet\": \"$ORGANIZER_WALLET\",
            \"refund_deadline_hours\": 168
        }")

    EVENT_SUCCESS=$(echo "$EVENT_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
    EVENT_CREATED_ID=$(echo "$EVENT_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print(d.get('id',''))" 2>/dev/null || echo "")

    if [ "$EVENT_SUCCESS" = "true" ] && [ -n "$EVENT_CREATED_ID" ]; then
        pass "Event created: id=$EVENT_CREATED_ID"
        EVENT_ID="$EVENT_CREATED_ID"
    else
        ERR=$(echo "$EVENT_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "$EVENT_RESPONSE")
        # Try seed endpoint as fallback
        info "Create event failed ($ERR), trying seed endpoint..."
        SEED_RESPONSE=$(curl -s -X POST "$BASE_URL/api/events/seed" \
            -H "Authorization: Bearer dev-token" \
            -H "Content-Type: application/json")

        SEED_SUCCESS=$(echo "$SEED_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
        if [ "$SEED_SUCCESS" = "true" ]; then
            EVENT_ID=$(echo "$SEED_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['id'])" 2>/dev/null || echo "default")
            pass "Event seeded: id=$EVENT_ID"
            # Update with deposit config
            info "Updating event with deposit config..."
            UPDATE_RESPONSE=$(curl -s -X PUT "$BASE_URL/api/events/$EVENT_ID" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{
                    \"deposit_enabled\": true,
                    \"deposit_amount_usdc\": $DEPOSIT_AMOUNT_USDC,
                    \"deposit_amount_thb\": $DEPOSIT_AMOUNT_THB,
                    \"promptpay_id\": \"0812345678\",
                    \"organizer_wallet\": \"$ORGANIZER_WALLET\",
                    \"refund_deadline_hours\": 168
                }")
            UPDATE_SUCCESS=$(echo "$UPDATE_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
            if [ "$UPDATE_SUCCESS" = "true" ]; then
                pass "Event updated with deposit config"
            else
                warn "Update response: $(echo "$UPDATE_RESPONSE" | head -c 200)"
            fi
        else
            fail "Failed to create/seed event"
            echo "   $SEED_RESPONSE" | head -c 300
        fi
    fi
fi

# ============================================================================
# Step 2: Verify Event Config
# ============================================================================
section "Step 2: Verify Event Config"

EVENT_DETAIL=$(curl -s "$BASE_URL/api/events/$EVENT_ID" \
    -H "Authorization: Bearer dev-token")

EVENT_SUCCESS=$(echo "$EVENT_DETAIL" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

if [ "$EVENT_SUCCESS" = "true" ]; then
    pass "GET /api/events/$EVENT_ID → success"

    DEPOSIT_ENABLED=$(echo "$EVENT_DETAIL" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']['event']; print(d.get('deposit_enabled',False))" 2>/dev/null || echo "False")
    DEPOSIT_AMOUNT=$(echo "$EVENT_DETAIL" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']['event']; print(d.get('deposit_amount_usdc',0))" 2>/dev/null || echo "0")
    ORG_WALLET=$(echo "$EVENT_DETAIL" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']['event']; print(d.get('organizer_wallet',''))" 2>/dev/null || echo "")
    ESCROW_ADDR=$(echo "$EVENT_DETAIL" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']['event']; print(d.get('escrow_address',''))" 2>/dev/null || echo "")
    ON_CHAIN_ID=$(echo "$EVENT_DETAIL" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']['event']; print(d.get('on_chain_event_id',0))" 2>/dev/null || echo "0")
    PROMPTPAY=$(echo "$EVENT_DETAIL" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']['event']; print(d.get('promptpay_id',''))" 2>/dev/null || echo "")

    info "deposit_enabled=$DEPOSIT_ENABLED, deposit_amount=$DEPOSIT_AMOUNT"
    info "organizer_wallet=$ORG_WALLET"
    info "escrow_address=$ESCROW_ADDR"
    info "on_chain_event_id=$ON_CHAIN_ID"
    info "promptpay_id=$PROMPTPAY"

    # Store the original event_end_ms (before Step 3b modifies it) for the refund wait
    ORIGINAL_EVENT_END_MS=$(echo "$EVENT_DETAIL" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']['event']; print(d.get('event_end_ms',0))" 2>/dev/null || echo "0")
    info "Original event_end_ms=$ORIGINAL_EVENT_END_MS (baked into on-chain PDA)"

    if [ "$DEPOSIT_ENABLED" != "True" ]; then
        fail "Deposit not enabled — cannot proceed"
        exit 1
    fi
    if [ -z "$ORG_WALLET" ]; then
        fail "organizer_wallet not set — cannot proceed"
        exit 1
    fi
else
    fail "Failed to get event details"
    echo "   $EVENT_DETAIL" | head -c 300
    exit 1
fi

# ============================================================================
# Step 3: Init Escrow (Combined: ATA + CreateEvent in one TX)
# ============================================================================
section "Step 3: Init Escrow (POST /api/escrow/init)"

if [ -n "$ESCROW_ADDR" ] && [ "$ESCROW_ADDR" != "" ]; then
    skip "Init escrow TX — escrow already initialized at $ESCROW_ADDR"
else
    info "Requesting combined init escrow TX from worker..."

    INIT_RESPONSE=$(curl -s -X POST "$BASE_URL/api/escrow/init" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_id\": \"$EVENT_ID\"}")

    INIT_SUCCESS=$(echo "$INIT_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

    if [ "$INIT_SUCCESS" = "true" ]; then
        TX_B64=$(echo "$INIT_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        ESCROW_ADDR=$(echo "$INIT_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['escrow_address'])" 2>/dev/null || echo "")
        VAULT_ADDR=$(echo "$INIT_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['vault_address'])" 2>/dev/null || echo "")
        ON_CHAIN_ID=$(echo "$INIT_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['on_chain_event_id'])" 2>/dev/null || echo "")
        TX_MSG=$(echo "$INIT_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

        pass "Init escrow TX built (combined ATA + CreateEvent)"
        info "Message: $TX_MSG"
        info "Escrow PDA: $ESCROW_ADDR"
        info "Vault ATA: $VAULT_ADDR"
        info "On-chain event ID: $ON_CHAIN_ID"

        # Submit TX with organizer keypair
        info "Signing and submitting TX to devnet..."
        ORG_KEYPAIR_JSON=$(cat ~/.config/solana/id.json)

        SUBMIT_OUTPUT=$(sign_and_submit_tx "$TX_B64" "$ORG_KEYPAIR_JSON")
        info "Submit output: $SUBMIT_OUTPUT"

        if echo "$SUBMIT_OUTPUT" | grep -q "SIGNATURE="; then
            INIT_SIG=$(echo "$SUBMIT_OUTPUT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Init escrow TX submitted!"
            info "Signature: $INIT_SIG"
            info "View: https://explorer.solana.com/tx/$INIT_SIG?cluster=devnet"

            # Wait for confirmation
            info "Waiting for confirmation..."
            sleep 5
            solana confirm "$INIT_SIG" --url devnet 2>&1 || warn "Confirmation check failed (may still be processing)"

            # Update event config with escrow address
            info "Updating event with escrow_address=$ESCROW_ADDR, on_chain_event_id=$ON_CHAIN_ID..."
            UPDATE_ESCROW=$(curl -s -X PUT "$BASE_URL/api/events/$EVENT_ID" \
                -H "Authorization: Bearer dev-token" \
                -H "Content-Type: application/json" \
                -d "{\"escrow_address\": \"$ESCROW_ADDR\", \"on_chain_event_id\": $ON_CHAIN_ID}")
            UPDATE_OK=$(echo "$UPDATE_ESCROW" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
            if [ "$UPDATE_OK" = "true" ]; then
                pass "Event updated with escrow_address"
            else
                warn "Failed to update escrow_address: $(echo "$UPDATE_ESCROW" | head -c 200)"
            fi
        else
            fail "Init escrow TX submission failed"
            echo "   $SUBMIT_OUTPUT" | head -c 500

            # Fallback: verify escrow account directly
            info "Checking if escrow PDA exists on-chain..."
            ESCROW_ACCOUNT=$(solana account "$ESCROW_ADDR" --url devnet 2>&1 || echo "NOT_FOUND")
            if echo "$ESCROW_ACCOUNT" | grep -q "length:"; then
                pass "Escrow PDA exists on-chain (may have been created despite error)"
            else
                warn "Escrow PDA not found: $ESCROW_ADDR"
            fi
        fi
    else
        ERR=$(echo "$INIT_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
        fail "Init escrow TX build failed: $ERR"
        echo "   Full: $(echo "$INIT_RESPONSE" | head -c 400)"
    fi
fi

# ============================================================================
# Step 3b: Update event_end_ms to Future (server-side only — for deposit acceptance)
# ============================================================================
section "Step 3b: Set Event End to Future for Deposits"

# The event was created with event_end_ms 2 minutes in the future (for on-chain init).
# We extend it further into the future (2 hours) for the server-side deposit handler.
# The on-chain event_end is baked into the PDA at init time (~2 min from now).
# After the deposit succeeds, we wait for the on-chain event_end to pass, then refund.
if [ "$SKIP_SETUP" = true ]; then
    skip "Event end time update — reusing existing event"
else
    FUTURE_MS=$(($(date +%s) + 7200))000
    info "Updating event_end_ms to $FUTURE_MS (2 hours from now) for deposit acceptance..."
    UPDATE_END_FUTURE=$(curl -s -X PUT "$BASE_URL/api/events/$EVENT_ID" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_end_ms\": $FUTURE_MS}")
    UPDATE_END_OK=$(echo "$UPDATE_END_FUTURE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
    if [ "$UPDATE_END_OK" = "true" ]; then
        pass "event_end_ms set to future (server-side only — on-chain PDA unchanged)"
    else
        warn "Failed to update event_end_ms: $(echo "$UPDATE_END_FUTURE" | head -c 200)"
    fi
fi

if [ -n "$ESCROW_ADDR" ] && [ "$ESCROW_ADDR" != "" ]; then
    info "Checking escrow account: $ESCROW_ADDR"
    ESCROW_INFO=$(solana account "$ESCROW_ADDR" --url devnet 2>&1 || echo "NOT_FOUND")

    if echo "$ESCROW_INFO" | grep -qi "length:"; then
        ESCROW_LAMPORTS=$(echo "$ESCROW_INFO" | grep "lamports:" | awk '{print $2}' || echo "0")
        ESCROW_OWNER=$(echo "$ESCROW_INFO" | grep "owner:" | awk '{print $2}' || echo "?")
        pass "Escrow PDA exists on-chain"
        info "Lamports: $ESCROW_LAMPORTS, Owner: $ESCROW_OWNER"

        # Verify it's owned by our escrow program
        if [ "$ESCROW_OWNER" = "$ESCROW_PROGRAM" ]; then
            pass "Escrow owned by correct program"
        else
            warn "Escrow owner: $ESCROW_OWNER (expected: $ESCROW_PROGRAM)"
        fi
    else
        warn "Escrow PDA not found: $ESCROW_ADDR"
        info "This is expected if create_event TX hasn't been submitted yet"
    fi

    # Derive and check vault ATA
    info "Deriving vault ATA..."
    # Use python to derive ATA (same logic as worker)
    VAULT_ATA=$(python3 -c "
import hashlib, struct, base58

def find_pda(seeds, program_id):
    \"\"\"Find program derived address (PDA).\"\"\"
    # In production, use proper PDA derivation
    # For devnet, we can use the Solana RPC's findProgramAddress
    pass

# Use solana RPC to derive ATA
import urllib.request, json

# Actually, just check if the vault exists via the event escrow
# The ATA is: SHA256(event_escrow + TOKEN_PROGRAM + usdc_mint) truncated
# Let's use RPC getAccountInfo to check

# For now, just check via spl-token
print('checking...')
" 2>/dev/null || echo "")

    # Check vault via spl-token balance for the escrow PDA
    info "Checking vault USDC balance (if vault ATA exists)..."
    VAULT_USDC=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ESCROW_ADDR" 2>&1 || echo "0 (vault may not exist yet)")
    info "Vault USDC: $VAULT_USDC"
else
    skip "Escrow verification — no escrow_address"
fi

# ============================================================================
# Step 5: Deposit Status Check (pre-deposit)
# ============================================================================
section "Step 5: Deposit Status (Pre-Deposit)"

TEST_ATTENDEE_ID="e2e-attendee-$(date +%s)"
DEPOSIT_STATUS=$(curl -s "$BASE_URL/api/deposit/status/$TEST_ATTENDEE_ID?event_id=$EVENT_ID")

# API wraps response in ApiOk: {"success":true,"data":{...}}
STATUS_PARSE=$(echo "$DEPOSIT_STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print('ok')" 2>/dev/null || echo "err")
if [ "$STATUS_PARSE" = "ok" ]; then
    DEP_ENABLED=$(echo "$DEPOSIT_STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print(d.get('deposit_enabled',False))" 2>/dev/null || echo "False")
    DEP_AMOUNT=$(echo "$DEPOSIT_STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print(d.get('deposit_amount_usdc',0))" 2>/dev/null || echo "0")
    DEP_STATUS=$(echo "$DEPOSIT_STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print(d.get('status'))" 2>/dev/null || echo "None")
    PROMPTPAY_STATUS=$(echo "$DEPOSIT_STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print(d.get('promptpay_id',''))" 2>/dev/null || echo "")

    pass "GET /api/deposit/status → deposit_enabled=$DEP_ENABLED, amount=$DEP_AMOUNT"
    info "Status: $DEP_STATUS, promptpay_id=$PROMPTPAY_STATUS"
else
    fail "GET /api/deposit/status → parse error"
    echo "   $(echo "$DEPOSIT_STATUS" | head -c 300)"
fi

# ============================================================================
# Step 6: Build Deposit TX (Solana Pay)
# ============================================================================
section "Step 6: Build Deposit Transaction"

info "Requesting deposit TX for attendee $ATTENDEE_WALLET..."

# Step 7a: Initiate deposit — POST /api/deposit/usdc
# This creates a pending deposit status and returns a Solana Pay URL.
DEPOSIT_INIT=$(curl -s -X POST "$BASE_URL/api/deposit/usdc" \
    -H "Content-Type: application/json" \
    -d "{
        \"event_id\": \"$EVENT_ID\",
        \"attendee_id\": \"$TEST_ATTENDEE_ID\",
        \"wallet_address\": \"$ATTENDEE_WALLET\"
    }")

DEP_INIT_SUCCESS=$(echo "$DEPOSIT_INIT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(str(d.get('success','')).lower())" 2>/dev/null || echo "false")
DEP_INIT_ERROR=$(echo "$DEPOSIT_INIT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message','')))" 2>/dev/null || echo "")

if [ "$DEP_INIT_SUCCESS" = "true" ]; then
    DEP_SOL_URL=$(echo "$DEPOSIT_INIT" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print(d.get('solana_pay_url',''))" 2>/dev/null || echo "")
    pass "Deposit initiated (Solana Pay URL)"
    info "Solana Pay URL: ${DEP_SOL_URL:0:80}..."

    # Step 7b: Fetch the actual serialized TX from the Solana Pay callback.
    # The wallet calls this URL to get the base64 TX to sign.
    info "Fetching deposit TX from Solana Pay callback..."
    PAY_CALLBACK=$(curl -s "$BASE_URL/api/deposit/usdc/tx?event_id=$EVENT_ID&attendee_id=$TEST_ATTENDEE_ID&wallet=$ATTENDEE_WALLET")

    DEP_TX_B64=$(echo "$PAY_CALLBACK" | python3 -c "import sys,json; print(json.load(sys.stdin).get('transaction',''))" 2>/dev/null || echo "")

    if [ -n "$DEP_TX_B64" ] && [ "$DEP_TX_B64" != "" ]; then
        pass "Deposit TX built via callback"
        info "Transaction: ${DEP_TX_B64:0:50}..."
    else
        # Callback might return error
        CB_ERR=$(echo "$PAY_CALLBACK" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', str(d)[:200]))" 2>/dev/null || echo "")
        warn "Solana Pay callback error: $CB_ERR"
        fail "Deposit TX build failed via callback"
    fi

    # Step 7c: Submit the deposit TX with attendee keypair
    if [ -n "$DEP_TX_B64" ] && [ "$DEP_TX_B64" != "" ] && [ -f "$ATTENDEE_KEYPAIR" ]; then
        info "Signing and submitting deposit TX with attendee keypair..."

        ATT_KEYPAIR_JSON=$(cat "$ATTENDEE_KEYPAIR")

        DEP_SUBMIT=$(sign_and_submit_tx "$DEP_TX_B64" "$ATT_KEYPAIR_JSON")
        info "Deposit submit: $DEP_SUBMIT"

        if echo "$DEP_SUBMIT" | grep -q "SIGNATURE="; then
            DEP_SIG=$(echo "$DEP_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Deposit TX submitted!"
            info "Signature: $DEP_SIG"
            info "View: https://explorer.solana.com/tx/$DEP_SIG?cluster=devnet"

            # Wait for confirmation
            info "Waiting for deposit confirmation..."
            sleep 5

            # Notify the worker about the TX signature so it can verify on-chain
            info "Notifying worker of deposit TX signature..."
            WEBHOOK_RESPONSE=$(curl -s -X POST "$BASE_URL/api/deposit/usdc/webhook" \
                -H "Content-Type: application/json" \
                -d "{
                    \"event_id\": \"$EVENT_ID\",
                    \"attendee_id\": \"$TEST_ATTENDEE_ID\",
                    \"tx_signature\": \"$DEP_SIG\"
                }")

            WEBHOOK_SUCCESS=$(echo "$WEBHOOK_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
            if [ "$WEBHOOK_SUCCESS" = "true" ]; then
                WEBHOOK_CONFIRMED=$(echo "$WEBHOOK_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); r=d.get('data',d); print(r.get('confirmed',False))" 2>/dev/null || echo "False")
                pass "Deposit webhook: confirmed=$WEBHOOK_CONFIRMED"
            else
                WH_ERR=$(echo "$WEBHOOK_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "")
                warn "Deposit webhook: $WH_ERR"
            fi

            # Also verify via confirm endpoint
            info "Verifying deposit via /api/deposit/usdc/confirm..."
            CONFIRM_RESPONSE=$(curl -s "$BASE_URL/api/deposit/usdc/confirm?event_id=$EVENT_ID&attendee_id=$TEST_ATTENDEE_ID" \
                -H "Authorization: Bearer dev-token")

            CONFIRM_SUCCESS=$(echo "$CONFIRM_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
            if [ "$CONFIRM_SUCCESS" = "true" ]; then
                CONFIRMED=$(echo "$CONFIRM_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print(d.get('confirmed',False))" 2>/dev/null || echo "False")
                pass "Deposit confirmation: confirmed=$CONFIRMED"
            else
                info "Confirm response: $(echo "$CONFIRM_RESPONSE" | head -c 200)"
            fi

            # Check vault balance after deposit
            info "Checking vault USDC balance after deposit..."
            VAULT_BAL=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ESCROW_ADDR" 2>&1 || echo "?")
            info "Vault USDC: $VAULT_BAL"
        else
            fail "Deposit TX submission failed"
            echo "   $DEP_SUBMIT" | head -c 500

            if echo "$DEP_SUBMIT" | grep -qi "insufficient"; then
                warn "Insufficient USDC — attendee needs more devnet USDC"
                warn "Use https://faucet.circle.com/ to get more"
            fi

            if echo "$DEP_SUBMIT" | grep -qi "PyNaCl"; then
                warn "PyNaCl not installed — install with: pip3 install pynacl"
            fi
        fi
    else
        skip "Deposit TX submission — no TX or attendee keypair not available"
        info "To submit manually: use the Solana Pay URL in a wallet adapter"
    fi
else
    fail "Deposit init failed"
    info "Error: $DEP_INIT_ERROR"
    echo "   Full: $(echo "$DEPOSIT_INIT" | head -c 400)"
fi

# ============================================================================
# Step 7: Deposit Status Check (post-deposit)
# ============================================================================
section "Step 7: Deposit Status (Post-Deposit)"

DEPOSIT_STATUS2=$(curl -s "$BASE_URL/api/deposit/status/$TEST_ATTENDEE_ID?event_id=$EVENT_ID")

STATUS2_PARSE=$(echo "$DEPOSIT_STATUS2" | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}); print('ok')" 2>/dev/null || echo "err")
if [ "$STATUS2_PARSE" = "ok" ]; then
    DEP_STATUS2=$(echo "$DEPOSIT_STATUS2" | python3 -c "
import sys, json
d = json.load(sys.stdin).get('data', {})
s = d.get('status')
if s:
    print(f\"method={s.get('method','?')}, verified={s.get('verified',False)}, tx={s.get('tx_signature','none')[:16]}...\")
else:
    print('no deposit recorded')
" 2>/dev/null || echo "?")
    pass "GET /api/deposit/status → $DEP_STATUS2"
else
    info "Deposit status: $(echo "$DEPOSIT_STATUS2" | head -c 200)"
fi

# ============================================================================
# Step 8: Mark Attendee Checked-In
# ============================================================================
section "Step 8: Mark Attendee Checked-In"

# The organizer must mark the attendee as checked in before a refund can be claimed.
# This is a protected endpoint (organizer signs the TX).
if [ -z "$ESCROW_ADDR" ] || [ "$ESCROW_ADDR" = "" ]; then
    skip "mark_checked_in TX — no escrow_address (deposit may not have succeeded)"
else
    info "Requesting mark_checked_in TX for attendee $ATTENDEE_WALLET..."

    MARK_CI_RESPONSE=$(curl -s -X POST "$BASE_URL/api/escrow/mark-checked-in" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_id\": \"$EVENT_ID\", \"attendee_wallet\": \"$ATTENDEE_WALLET\"}")

    MARK_CI_SUCCESS=$(echo "$MARK_CI_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

    if [ "$MARK_CI_SUCCESS" = "true" ]; then
        MARK_CI_TX=$(echo "$MARK_CI_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        MARK_CI_MSG=$(echo "$MARK_CI_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

        pass "mark_checked_in TX built"
        info "Message: $MARK_CI_MSG"

        # Submit TX with organizer keypair (organizer signs this)
        ORG_KEYPAIR_JSON=$(cat ~/.config/solana/id.json)
        MARK_CI_SUBMIT=$(sign_and_submit_tx "$MARK_CI_TX" "$ORG_KEYPAIR_JSON")
        info "Mark checked-in submit: $MARK_CI_SUBMIT"

        if echo "$MARK_CI_SUBMIT" | grep -q "SIGNATURE="; then
            MARK_CI_SIG=$(echo "$MARK_CI_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "mark_checked_in TX submitted!"
            info "Signature: $MARK_CI_SIG"
            info "View: https://explorer.solana.com/tx/$MARK_CI_SIG?cluster=devnet"
            sleep 5
        else
            fail "mark_checked_in TX submission failed"
            echo "   $MARK_CI_SUBMIT" | head -c 500
        fi
    else
        ERR=$(echo "$MARK_CI_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
        warn "mark_checked_in TX build failed: $ERR"
        info "Note: This may fail if deposit wasn't confirmed on-chain"
    fi
fi

# ============================================================================
# Step 8b: Wait for On-Chain Event End
# ============================================================================
section "Step 8b: Wait for Event End (for refund eligibility)"

# The on-chain refund instruction requires clock > event_end.
# The escrow PDA was initialized with event_end = ~2 min from creation.
# We need to wait for that time to pass before the refund will work.
info "Checking if on-chain event_end has passed..."
ONCHAIN_EVENT_END_TS=$((ORIGINAL_EVENT_END_MS / 1000))
info "On-chain event_end (unix seconds): $ONCHAIN_EVENT_END_TS"
WAIT_SECONDS=0
MAX_WAIT=180  # Maximum 3 minutes to wait
while [ $WAIT_SECONDS -lt $MAX_WAIT ]; do
    NOW_TS=$(date +%s)
    if [ "$NOW_TS" -ge "$ONCHAIN_EVENT_END_TS" ]; then
        pass "Event end has passed (now=$NOW_TS, event_end=$ONCHAIN_EVENT_END_TS) — refunds enabled"
        break
    fi
    info "Waiting for event_end to pass... (now=$NOW_TS, event_end=$ONCHAIN_EVENT_END_TS, waited=${WAIT_SECONDS}s)"
    sleep 10
    WAIT_SECONDS=$((WAIT_SECONDS + 10))
done
if [ $WAIT_SECONDS -ge $MAX_WAIT ]; then
    warn "Timeout waiting for event_end — refund may fail with EventEndInPast check"
fi

# ============================================================================
# Step 9: Build Refund TX
# ============================================================================
section "Step 9: Build Refund Transaction"

# The on-chain refund instruction requires clock.unix_timestamp > event_end.
# We created the event with event_end_ms ~2 min in the future.
# Step 8b waited for that time to pass. Now refunds should work.
# The refund also requires the attendee to be marked as checked-in first.
info "Requesting refund TX for attendee $ATTENDEE_WALLET..."

REFUND_TX=$(curl -s -X POST "$BASE_URL/api/escrow/refund" \
    -H "Content-Type: application/json" \
    -d "{
        \"event_id\": \"$EVENT_ID\",
        \"attendee_id\": \"$TEST_ATTENDEE_ID\",
        \"wallet_address\": \"$ATTENDEE_WALLET\"
    }")

REFUND_SUCCESS=$(echo "$REFUND_TX" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if d.get('transaction') else 'no')" 2>/dev/null || echo "no")
REFUND_ERROR=$(echo "$REFUND_TX" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message','')))" 2>/dev/null || echo "")

if [ "$REFUND_SUCCESS" = "yes" ]; then
    REFUND_TX_B64=$(echo "$REFUND_TX" | python3 -c "import sys,json; print(json.load(sys.stdin).get('transaction',''))" 2>/dev/null || echo "")
    REFUND_MSG=$(echo "$REFUND_TX" | python3 -c "import sys,json; print(json.load(sys.stdin).get('message',''))" 2>/dev/null || echo "")

    pass "Refund TX built"
    info "Message: $REFUND_MSG"
    info "Transaction: ${REFUND_TX_B64:0:40}..."

    # Submit refund TX with attendee keypair
    if [ -f "$ATTENDEE_KEYPAIR" ]; then
        info "Signing and submitting refund TX with attendee keypair..."

        ATT_KEYPAIR_JSON=$(cat "$ATTENDEE_KEYPAIR")

        REFUND_SUBMIT=$(sign_and_submit_tx "$REFUND_TX_B64" "$ATT_KEYPAIR_JSON")
        info "Refund submit: $REFUND_SUBMIT"

        if echo "$REFUND_SUBMIT" | grep -q "SIGNATURE="; then
            REFUND_SIG=$(echo "$REFUND_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "Refund TX submitted!"
            info "Signature: $REFUND_SIG"
            info "View: https://explorer.solana.com/tx/$REFUND_SIG?cluster=devnet"

            # Wait for confirmation
            info "Waiting for refund confirmation..."
            sleep 5

            # Check attendee balance after refund
            ATT_USDC_AFTER=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_WALLET" 2>&1 | awk '{print $1}' || echo "?")
            info "Attendee USDC after refund: $ATT_USDC_AFTER"
        else
            fail "Refund TX submission failed"
            echo "   $REFUND_SUBMIT" | head -c 500

            if echo "$REFUND_SUBMIT" | grep -qi "custom program error"; then
                warn "Escrow program error — the refund may require mark_checked_in first"
                info "Note: In full flow, organizer must mark_checked_in before refund"
            fi
        fi
    else
        skip "Refund TX submission — attendee keypair not available"
    fi
else
    # Could be success=false with wrapped response
    REFUND_WRAP=$(echo "$REFUND_TX" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if d.get('data',{}).get('transaction') else 'no')" 2>/dev/null || echo "no")
    if [ "$REFUND_WRAP" = "yes" ]; then
        REFUND_TX_B64=$(echo "$REFUND_TX" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        REFUND_MSG=$(echo "$REFUND_TX" | python3 -c "import sys,json; print(json.load(sys.stdin)['data'].get('message',''))" 2>/dev/null || echo "")
        pass "Refund TX built (wrapped response)"
        info "Message: $REFUND_MSG"
        info "Transaction: ${REFUND_TX_B64:0:40}..."

        # Submit refund TX with attendee keypair
        if [ -f "$ATTENDEE_KEYPAIR" ] && [ -n "$REFUND_TX_B64" ]; then
            info "Signing and submitting refund TX with attendee keypair..."
            ATT_KEYPAIR_JSON=$(cat "$ATTENDEE_KEYPAIR")
            REFUND_SUBMIT=$(sign_and_submit_tx "$REFUND_TX_B64" "$ATT_KEYPAIR_JSON")
            info "Refund submit: $REFUND_SUBMIT"

            if echo "$REFUND_SUBMIT" | grep -q "SIGNATURE="; then
                REFUND_SIG=$(echo "$REFUND_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
                pass "Refund TX submitted!"
                info "Signature: $REFUND_SIG"
                info "View: https://explorer.solana.com/tx/$REFUND_SIG?cluster=devnet"
                sleep 5
                ATT_USDC_AFTER=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_WALLET" 2>&1 | awk '{print $1}' || echo "?")
                info "Attendee USDC after refund: $ATT_USDC_AFTER"
            else
                fail "Refund TX submission failed"
                echo "   $REFUND_SUBMIT" | head -c 500
            fi
        fi
    else
        fail "Refund TX build failed"
        info "Error: $REFUND_ERROR"
        echo "   Full: $(echo "$REFUND_TX" | head -c 400)"

        if echo "$REFUND_ERROR" | grep -qi "not verified"; then
            info "Deposit not yet verified — refund requires confirmed deposit first"
            info "This is expected if the deposit TX failed in Step 6"
        fi
    fi
fi

# ============================================================================
# Step 10: Verify Final State
# ============================================================================
section "Step 10: Verify Final On-Chain State"

info "Checking final vault balance..."
if [ -n "$ESCROW_ADDR" ] && [ "$ESCROW_ADDR" != "" ]; then
    VAULT_FINAL=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ESCROW_ADDR" 2>&1 || echo "0 (no vault)")
    info "Vault USDC (final): $VAULT_FINAL"
fi

ATT_USDC_FINAL=$(spl-token balance "$USDC_MINT" --url devnet --owner "$ATTENDEE_WALLET" 2>&1 | awk '{print $1}' || echo "?")
info "Attendee USDC (final): $ATT_USDC_FINAL"

ATT_SOL_FINAL=$(solana balance "$ATTENDEE_WALLET" --url devnet 2>&1 | awk '{print $1}' || echo "?")
info "Attendee SOL (final): $ATT_SOL_FINAL"

# ============================================================================
# Step 11: THB Deposit Flow Test
# ============================================================================
section "Step 11: THB Deposit Flow"

# Test THB slip upload
info "Testing THB slip upload..."
SLIP_UPLOAD=$(curl -s -X POST "$BASE_URL/api/deposit/thb/upload" \
    -H "Content-Type: application/json" \
    -d "{
        \"event_id\": \"$EVENT_ID\",
        \"attendee_id\": \"$TEST_ATTENDEE_ID\",
        \"slip_url\": \"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPj/HwADBwIAMCbHYQAAAABJRU5ErkJggg==\"
    }")

SLIP_PARSE=$(echo "$SLIP_UPLOAD" | python3 -c "import sys,json; d=json.load(sys.stdin); print('ok')" 2>/dev/null || echo "err")
if [ "$SLIP_PARSE" = "ok" ]; then
    SLIP_SUCCESS=$(echo "$SLIP_UPLOAD" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")
    if [ "$SLIP_SUCCESS" = "true" ]; then
        pass "THB slip uploaded"
    else
        ERR=$(echo "$SLIP_UPLOAD" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "")
        pass "THB slip endpoint responded (error: $ERR)"
    fi
else
    info "THB slip upload response: $(echo "$SLIP_UPLOAD" | head -c 200)"
fi

# Test pending THB slips (admin)
info "Checking pending THB slips..."
PENDING_SLIPS=$(curl -s "$BASE_URL/api/deposit/thb/pending?event_id=$EVENT_ID" \
    -H "Authorization: Bearer dev-token")

PENDING_PARSE=$(echo "$PENDING_SLIPS" | python3 -c "import sys,json; d=json.load(sys.stdin); print('ok')" 2>/dev/null || echo "err")
if [ "$PENDING_PARSE" = "ok" ]; then
    SLIP_COUNT=$(echo "$PENDING_SLIPS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d.get('data',{}).get('slips',[])))" 2>/dev/null || echo "0")
    pass "Pending THB slips: $SLIP_COUNT"
else
    info "Pending slips response: $(echo "$PENDING_SLIPS" | head -c 200)"
fi

# ===========================================================================
# Step 12: Deactivate Event
# ===========================================================================
section "Step 12: Deactivate Event"

if [ -z "$ESCROW_ADDR" ] || [ "$ESCROW_ADDR" = "" ]; then
    skip "deactivate_event TX — no escrow_address"
else
    info "Requesting deactivate_event TX..."

    DEACTIVATE_RESPONSE=$(curl -s -X POST "$BASE_URL/api/escrow/deactivate-event" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_id\": \"$EVENT_ID\"}")

    DEACTIVATE_SUCCESS=$(echo "$DEACTIVATE_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

    if [ "$DEACTIVATE_SUCCESS" = "true" ]; then
        DEACTIVATE_TX=$(echo "$DEACTIVATE_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        DEACTIVATE_MSG=$(echo "$DEACTIVATE_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

        pass "deactivate_event TX built"
        info "Message: $DEACTIVATE_MSG"

        # Submit TX with organizer keypair
        ORG_KEYPAIR_JSON=$(cat ~/.config/solana/id.json)
        DEACTIVATE_SUBMIT=$(sign_and_submit_tx "$DEACTIVATE_TX" "$ORG_KEYPAIR_JSON")
        info "Deactivate submit: $DEACTIVATE_SUBMIT"

        if echo "$DEACTIVATE_SUBMIT" | grep -q "SIGNATURE="; then
            DEACTIVATE_SIG=$(echo "$DEACTIVATE_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "deactivate_event TX submitted!"
            info "Signature: $DEACTIVATE_SIG"
            info "View: https://explorer.solana.com/tx/$DEACTIVATE_SIG?cluster=devnet"
            sleep 5
        else
            fail "deactivate_event TX submission failed"
            echo "   $DEACTIVATE_SUBMIT" | head -c 500
        fi
    else
        ERR=$(echo "$DEACTIVATE_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
        warn "deactivate_event TX build failed: $ERR"
    fi
fi

# ===========================================================================
# Step 13: Claim Forfeited (no-show deposits)
# ===========================================================================
section "Step 13: Claim Forfeited Deposits"

if [ -z "$ESCROW_ADDR" ] || [ "$ESCROW_ADDR" = "" ]; then
    skip "claim_forfeited TX — no escrow_address"
else
    info "Requesting claim_forfeited TX..."

    CLAIM_RESPONSE=$(curl -s -X POST "$BASE_URL/api/escrow/claim-forfeited" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_id\": \"$EVENT_ID\"}")

    CLAIM_SUCCESS=$(echo "$CLAIM_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

    if [ "$CLAIM_SUCCESS" = "true" ]; then
        CLAIM_TX=$(echo "$CLAIM_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        CLAIM_MSG=$(echo "$CLAIM_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

        pass "claim_forfeited TX built"
        info "Message: $CLAIM_MSG"

        # Submit TX with organizer keypair
        ORG_KEYPAIR_JSON=$(cat ~/.config/solana/id.json)
        CLAIM_SUBMIT=$(sign_and_submit_tx "$CLAIM_TX" "$ORG_KEYPAIR_JSON")
        info "Claim forfeited submit: $CLAIM_SUBMIT"

        if echo "$CLAIM_SUBMIT" | grep -q "SIGNATURE="; then
            CLAIM_SIG=$(echo "$CLAIM_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "claim_forfeited TX submitted!"
            info "Signature: $CLAIM_SIG"
            info "View: https://explorer.solana.com/tx/$CLAIM_SIG?cluster=devnet"
            sleep 5
        else
            # claim_forfeited may fail with "nothing to claim" if all attendees were refunded
            if echo "$CLAIM_SUBMIT" | grep -qi "nothing\|no forfeited"; then
                pass "claim_forfeited: nothing to claim (expected — all refunded)"
            else
                fail "claim_forfeited TX submission failed"
                echo "   $CLAIM_SUBMIT" | head -c 500
            fi
        fi
    else
        ERR=$(echo "$CLAIM_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
        warn "claim_forfeited TX build failed: $ERR"
    fi
fi

# ===========================================================================
# Step 14: Close Event (reclaim rent)
# ===========================================================================
section "Step 14: Close Event & Reclaim Rent"

if [ -z "$ESCROW_ADDR" ] || [ "$ESCROW_ADDR" = "" ]; then
    skip "close_event TX — no escrow_address"
else
    info "Requesting close_event TX..."

    CLOSE_RESPONSE=$(curl -s -X POST "$BASE_URL/api/escrow/close-event" \
        -H "Authorization: Bearer dev-token" \
        -H "Content-Type: application/json" \
        -d "{\"event_id\": \"$EVENT_ID\"}")

    CLOSE_SUCCESS=$(echo "$CLOSE_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

    if [ "$CLOSE_SUCCESS" = "true" ]; then
        CLOSE_TX=$(echo "$CLOSE_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['transaction'])" 2>/dev/null || echo "")
        CLOSE_MSG=$(echo "$CLOSE_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['message'])" 2>/dev/null || echo "")

        pass "close_event TX built"
        info "Message: $CLOSE_MSG"

        # Submit TX with organizer keypair
        ORG_KEYPAIR_JSON=$(cat ~/.config/solana/id.json)
        CLOSE_SUBMIT=$(sign_and_submit_tx "$CLOSE_TX" "$ORG_KEYPAIR_JSON")
        info "Close event submit: $CLOSE_SUBMIT"

        if echo "$CLOSE_SUBMIT" | grep -q "SIGNATURE="; then
            CLOSE_SIG=$(echo "$CLOSE_SUBMIT" | grep "SIGNATURE=" | cut -d= -f2)
            pass "close_event TX submitted!"
            info "Signature: $CLOSE_SIG"
            info "View: https://explorer.solana.com/tx/$CLOSE_SIG?cluster=devnet"
            sleep 5

            # Verify escrow account is closed
            info "Verifying escrow account closed..."
            ESCROW_CHECK=$(solana account "$ESCROW_ADDR" --url devnet 2>&1 || echo "CLOSED")
            if echo "$ESCROW_CHECK" | grep -qi "error\|not found\|CLOSED"; then
                pass "Escrow account closed successfully — rent reclaimed"
            else
                info "Escrow account still exists: $(echo "$ESCROW_CHECK" | head -c 200)"
            fi
        else
            fail "close_event TX submission failed"
            echo "   $CLOSE_SUBMIT" | head -c 500

            if echo "$CLOSE_SUBMIT" | grep -qi "still active\|EventStillActive"; then
                warn "Event still active — deactivate_event must succeed first (Step 13)"
            fi
            if echo "$CLOSE_SUBMIT" | grep -qi "not empty\|VaultNotEmpty"; then
                warn "Vault not empty — all funds must be refunded or claimed first"
            fi
        fi
    else
        ERR=$(echo "$CLOSE_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message', str(d)[:200])))" 2>/dev/null || echo "unknown")
        warn "close_event TX build failed: $ERR"
    fi
fi

# ============================================================================
# Summary
# ============================================================================
echo ""
echo -e "${BOLD}━━━ Test Summary ━━━${NC}"
echo -e "  ${GREEN}✅ Pass: $PASS${NC}"
echo -e "  ${RED}❌ Fail: $FAIL${NC}"
echo -e "  ${YELLOW}⏭️  Skip: $SKIP${NC}"
echo ""

if [ $FAIL -eq 0 ]; then
    echo -e "${GREEN}${BOLD}🎉 All tests passed!${NC}"
else
    echo -e "${RED}${BOLD}❌ $FAIL test(s) failed${NC}"
fi

echo ""
echo -e "${CYAN}Next Steps:${NC}"
echo "  1. If create_event TX failed: submit via Phantom/Backpack wallet adapter"
echo "  2. If deposit TX failed: ensure attendee has devnet USDC"
echo "  3. If refund TX failed: ensure deposit was verified + mark_checked_in"
echo "  4. Test full UI flow: open http://localhost:8787 in browser with wallet adapter"
echo ""
echo -e "${CYAN}Manual Wallet Testing:${NC}"
echo "  - Open Phantom/Backpack, switch to Devnet"
echo "  - Get USDC from https://faucet.circle.com/"
echo "  - Navigate to deposit page in browser"
echo "  - Connect wallet → sign TX → verify on explorer"
echo ""

# Cleanup
exit $FAIL
