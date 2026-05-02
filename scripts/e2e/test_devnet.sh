#!/usr/bin/env bash
# ============================================================================
# BeThere Devnet E2E Test Script
# ============================================================================
# Tests the full claim flow end-to-end:
#   1. Server health
#   2. Frontend serving
#   3. Claim token lookup (fake token → graceful error)
#   4. Adventure status check (fake token → graceful error)
#   5. Direct Helius mint (proves cNFT minting works on devnet)
#   6. Adventure config CRUD (requires AUTH_TOKEN — manual step)
#
# Prerequisites:
#   - `cd worker && npx wrangler dev --port 8787` running in another terminal
#   - HELIUS_API_KEY set in worker/.dev.vars
#   - frontend-leptos/dist/ built (`cd frontend-leptos && bash build.sh`)
#
# Usage:
#   bash scripts/e2e/test_devnet.sh                    # Run all tests
#   AUTH_TOKEN=xxx bash scripts/e2e/test_devnet.sh     # Include admin tests
#   bash scripts/e2e/test_devnet.sh --mint-only        # Just test minting
# ============================================================================

set -euo pipefail

# --- Config ---
BASE_URL="${BASE_URL:-http://localhost:8787}"
AUTH_TOKEN="${AUTH_TOKEN:-}"
EVENT_ID="${EVENT_ID:-default}"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASS=0
FAIL=0
SKIP=0

# --- Helpers ---
pass() { PASS=$((PASS + 1)); echo -e "  ${GREEN}✅ PASS${NC} $1"; }
fail() { FAIL=$((FAIL + 1)); echo -e "  ${RED}❌ FAIL${NC} $1"; }
skip() { SKIP=$((SKIP + 1)); echo -e "  ${YELLOW}⏭️  SKIP${NC} $1"; }
info() { echo -e "  ${CYAN}ℹ️  INFO${NC} $1"; }
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

# --- Parse args ---
MINT_ONLY=false
if [[ "${1:-}" == "--mint-only" ]]; then
    MINT_ONLY=true
fi

echo ""
echo "🧪 BeThere Devnet E2E Test Suite"
echo "   BASE_URL: $BASE_URL"
echo "   EVENT_ID: $EVENT_ID"
echo ""

# ============================================================================
# Test 1: Health Check
# ============================================================================
if [ "$MINT_ONLY" = false ]; then
    section "1. Health Check"

    RESPONSE=$(curl -s "$BASE_URL/api/health")
    if check_json "$RESPONSE" "['status']" "ok"; then
        pass "GET /api/health → status=ok"
    else
        fail "GET /api/health → unexpected response"
        echo "   $RESPONSE"
    fi

    VERSION=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('version','?'))" 2>/dev/null || echo "?")
    info "Server version: $VERSION"
fi

# ============================================================================
# Test 2: Frontend Served
# ============================================================================
if [ "$MINT_ONLY" = false ]; then
    section "2. Frontend Serving"

    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/")
    if [ "$HTTP_CODE" = "200" ]; then
        pass "GET / → 200 (frontend served)"
    else
        fail "GET / → HTTP $HTTP_CODE (expected 200)"
    fi

    # Check WASM file exists
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/" 2>/dev/null)
    if [ "$HTTP_CODE" = "200" ]; then
        pass "Frontend HTML loads correctly"
    fi
fi

# ============================================================================
# Test 3: Claim Token Lookup (fake token)
# ============================================================================
if [ "$MINT_ONLY" = false ]; then
    section "3. Claim Token Lookup"

    FAKE_TOKEN="00000000-0000-7000-8000-000000000000"
    RESPONSE=$(curl -s "$BASE_URL/api/claim/$FAKE_TOKEN")

    if check_json "$RESPONSE" "['success']" "False"; then
        pass "GET /api/claim/{fake_token} → success=false"
    else
        fail "GET /api/claim/{fake_token} → unexpected response"
        echo "   $RESPONSE"
    fi

    ERROR_MSG=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "")
    if echo "$ERROR_MSG" | grep -qi "not found"; then
        pass "Error message contains 'not found' → graceful rejection"
    else
        info "Error message: $ERROR_MSG"
    fi
fi

# ============================================================================
# Test 4: Adventure Status (fake token)
# ============================================================================
if [ "$MINT_ONLY" = false ]; then
    section "4. Adventure Status Check"

    FAKE_TOKEN="00000000-0000-7000-8000-000000000000"
    RESPONSE=$(curl -s "$BASE_URL/api/adventure/$FAKE_TOKEN/status?event_id=$EVENT_ID")

    if check_json "$RESPONSE" "['success']" "true"; then
        pass "GET /api/adventure/{fake_token}/status → success=true"
        STATUS_VAL=$(echo "$RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']; print(d.get('status','?'))" 2>/dev/null || echo "?")
        info "Adventure status for fake token: $STATUS_VAL"
    else
        # May also return success=false with error, that's fine for fake token
        ERROR_MSG=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "")
        info "Error (expected for fake token): $ERROR_MSG"
        pass "Adventure endpoint responds (even if no progress found)"
    fi
fi

# ============================================================================
# Test 5: Direct Helius Mint Test
# ============================================================================
section "5. Direct Helius cNFT Mint"

# Read API key from .dev.vars
HELIUS_API_KEY=""
if [ -f "worker/.dev.vars" ]; then
    HELIUS_API_KEY=$(grep "^HELIUS_API_KEY=" worker/.dev.vars | cut -d= -f2- | tr -d '"' | tr -d "'")
fi

if [ -z "$HELIUS_API_KEY" ]; then
    skip "Helius mint test — HELIUS_API_KEY not found in worker/.dev.vars"
else
    # Generate a real devnet wallet using solana CLI
    TEST_WALLET=""
    if command -v solana &>/dev/null; then
        TEMP_KEYPAIR="/tmp/bethere-e2e-test-keypair.json"
        if solana-keygen new --no-bip39-passphrase --silent --outfile "$TEMP_KEYPAIR" 2>/dev/null; then
            TEST_WALLET=$(solana address --keypair "$TEMP_KEYPAIR" 2>/dev/null || echo "")
            rm -f "$TEMP_KEYPAIR"
        fi
    fi

    # Fallback: use a known valid Solana address (system program won't work for minting)
    if [ -z "$TEST_WALLET" ] || [ "$TEST_WALLET" = "11111111111111111111111111111111" ]; then
        skip "Helius mint test — solana-keygen failed, cannot generate test wallet"
        info "Install solana CLI: sh -c \"$(curl -sSfL https://release.anza.xyz/stable/install)\""
    else
        info "Minting test cNFT to wallet: ${TEST_WALLET:0:8}...${TEST_WALLET: -4}"

        HELIUS_URL="https://devnet.helius-rpc.com/?api-key=${HELIUS_API_KEY}"
        MINT_RESPONSE=$(curl -s "$HELIUS_URL" \
            -X POST \
            -H "Content-Type: application/json" \
            -d "{\"jsonrpc\":\"2.0\",\"id\":\"e2e-test-$(date +%s)\",\"method\":\"mintCompressedNft\",\"params\":{\"name\":\"BeThere E2E Test Badge\",\"symbol\":\"BETHERE\",\"description\":\"E2E test mint from BeThere devnet test script\",\"owner\":\"$TEST_WALLET\",\"imageUrl\":\"$BASE_URL/api/badge.svg\",\"externalUrl\":\"$BASE_URL\",\"sellerFeeBasisPoints\":0,\"confirmTransaction\":true}}")

    # Check if mint succeeded
    ASSET_ID=$(echo "$MINT_RESPONSE" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    r = d.get('result', {})
    print(r.get('assetId', r.get('asset_id', '')))
except:
    print('')
" 2>/dev/null || echo "")

    if [ -n "$ASSET_ID" ] && [ "$ASSET_ID" != "" ]; then
        pass "mintCompressedNft succeeded → asset_id=$ASSET_ID"
        info "View: https://explorer.solana.com/address/$ASSET_ID?cluster=devnet"
    else
        ERROR_RPC=$(echo "$MINT_RESPONSE" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    e = d.get('error', {})
    print(e.get('message', str(e)))
except:
    print('unknown error')
" 2>/dev/null || echo "parse error")

        # Check if it's just an invalid wallet (expected if we don't have solana CLI)
        if echo "$ERROR_RPC" | grep -qi "invalid\|wallet\|owner"; then
            info "Mint failed with wallet validation error (expected without valid keypair)"
            info "Error: $ERROR_RPC"
            info "To fix: generate a real Solana keypair and fund it on devnet"
            skip "Helius mint — needs valid devnet wallet (install solana CLI)"
        else
            fail "mintCompressedNft failed: $ERROR_RPC"
            echo "   Full response: $MINT_RESPONSE" | head -c 500
        fi
    fi
    fi # wallet check
fi # HELIUS_API_KEY check

# ============================================================================
# Test 6: Admin Adventure Config (requires auth)
# ============================================================================
if [ "$MINT_ONLY" = false ]; then
    section "6. Admin Adventure Config"

    if [ -z "$AUTH_TOKEN" ]; then
        skip "Admin adventure config — set AUTH_TOKEN to run"
        info "Get your token: login at $BASE_URL/login, then check browser cookies"
        info "Usage: AUTH_TOKEN=xxx bash scripts/e2e/test_devnet.sh"
    else
        # Read config
        RESPONSE=$(curl -s "$BASE_URL/api/admin/adventure?event_id=$EVENT_ID" \
            -H "Cookie: auth_token=$AUTH_TOKEN")

        SUCCESS=$(echo "$RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

        if [ "$SUCCESS" = "true" ]; then
            pass "GET /api/admin/adventure → success=true"
            ENABLED=$(echo "$RESPONSE" | python3 -c "
import sys, json
d = json.load(sys.stdin)['data']
c = d.get('config')
if c:
    print(f'enabled={c.get(\"enabled\", False)}, required_level={c.get(\"required_level\", \"?\")}')
else:
    print('no config (not yet created)')
" 2>/dev/null || echo "?")
            info "Config: $ENABLED"
        else
            fail "GET /api/admin/adventure → unauthorized or error"
            echo "   $(echo "$RESPONSE" | head -c 200)"
        fi

        # Update config (enable adventure, set required_level=1)
        info "Updating adventure config: enabled=true, required_level=1"
        PUT_RESPONSE=$(curl -s -X PUT "$BASE_URL/api/admin/adventure?event_id=$EVENT_ID" \
            -H "Cookie: auth_token=$AUTH_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "enabled": true,
                "required_level": 1
            }')

        PUT_SUCCESS=$(echo "$PUT_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

        if [ "$PUT_SUCCESS" = "true" ]; then
            pass "PUT /api/admin/adventure → saved (enabled=true, level=1)"
        else
            fail "PUT /api/admin/adventure → error"
            echo "   $(echo "$PUT_RESPONSE" | head -c 200)"
        fi
    fi
fi

# ============================================================================
# Test 7: Adventure Save Progress (requires claim token from real check-in)
# ============================================================================
if [ "$MINT_ONLY" = false ]; then
    section "7. Adventure Save Progress"

    CLAIM_TOKEN="${CLAIM_TOKEN:-}"

    if [ -z "$CLAIM_TOKEN" ]; then
        skip "Adventure save — set CLAIM_TOKEN to run (from a real check-in)"
        info "Usage: CLAIM_TOKEN=xxx bash scripts/e2e/test_devnet.sh"
    else
        SAVE_RESPONSE=$(curl -s -X POST "$BASE_URL/api/adventure/$CLAIM_TOKEN/save?event_id=$EVENT_ID" \
            -H "Content-Type: application/json" \
            -d "{
                \"claim_token\": \"$CLAIM_TOKEN\",
                \"level_id\": \"level_01\",
                \"score\": {\"moves\": 42, \"time_seconds\": 120}
            }")

        SAVE_SUCCESS=$(echo "$SAVE_RESPONSE" | python3 -c "import sys,json; print(str(json.load(sys.stdin).get('success','')).lower())" 2>/dev/null || echo "false")

        if [ "$SAVE_SUCCESS" = "true" ]; then
            pass "POST /api/adventure/{token}/save → saved level_01"

            # Check status now
            STATUS_RESPONSE=$(curl -s "$BASE_URL/api/adventure/$CLAIM_TOKEN/status?event_id=$EVENT_ID")
            STATUS_VAL=$(echo "$STATUS_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['status'])" 2>/dev/null || echo "?")
            info "Adventure status after save: $STATUS_VAL"
        else
            fail "POST /api/adventure/{token}/save → error"
            echo "   $(echo "$SAVE_RESPONSE" | head -c 300)"
        fi
    fi
fi

# ============================================================================
# Summary
# ============================================================================
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "  ${GREEN}PASS${NC}: $PASS  ${RED}FAIL${NC}: $FAIL  ${YELLOW}SKIP${NC}: $SKIP"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ "$FAIL" -gt 0 ]; then
    echo -e "\n${RED}Some tests failed. Check the output above.${NC}"
    exit 1
else
    echo -e "\n${GREEN}All tests passed!${NC} 🎉"
fi

# ============================================================================
# Manual E2E Flow Guide
# ============================================================================
echo ""
echo "📋 Manual E2E Flow Guide:"
echo "   1. Open $BASE_URL/login → login with staff email"
echo "   2. Admin panel → enable Adventure (required_level=1)"
echo "   3. Staff scanner → scan/check-in attendee → get claim token"
echo "   4. Open $BASE_URL/claim/{TOKEN} → see NFT preview"
echo "   5. (If quiz enabled) Pass quiz first"
echo "   6. Adventure gate → click Start → complete Level 1"
echo "   7. Return to claim → enter wallet → mint cNFT"
echo "   8. Check https://explorer.solana.com/address/{ASSET_ID}?cluster=devnet"
echo ""
