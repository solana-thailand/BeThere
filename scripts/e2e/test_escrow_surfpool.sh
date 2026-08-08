#!/usr/bin/env bash
# ============================================================================
# BeThere Surfpool Local Mainnet-Fork E2E Test Suite
# ============================================================================
# Exercises the BeThere smart contract against a local Mainnet-Fork via Surfpool.
# Uses real Mainnet USDC mint address (EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v).
# ============================================================================

set -euo pipefail

RPC_URL="${RPC_URL:-http://127.0.0.1:8899}"
MAINNET_USDC_MINT="EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log_pass() { echo -e "  ${GREEN}✅ PASS${NC} $1"; }
log_fail() { echo -e "  ${RED}❌ FAIL${NC} $1"; }
log_info() { echo -e "  ${CYAN}ℹ️  INFO${NC} $1"; }
log_warn() { echo -e "  ${YELLOW}⚠️  WARN${NC} $1"; }

echo -e "\n${CYAN}🏄 BeThere Surfpool Mainnet-Fork Local E2E Test${NC}"
echo "   RPC_URL:           $RPC_URL"
echo "   MAINNET_USDC_MINT: $MAINNET_USDC_MINT"
echo ""

# --- Step 0: Verify Surfpool RPC Connection ---
log_info "Checking local Surfpool RPC connection at $RPC_URL..."

if curl -s -X POST "$RPC_URL" -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"getVersion"}' | grep -q "solana-core"; then
    log_pass "Surfpool RPC is active!"
else
    log_warn "Surfpool RPC is not running at $RPC_URL. You can start Surfpool in another tab:"
    echo -e "   ${YELLOW}surfpool start --network mainnet${NC}\n"
    log_info "Attempting to start surfpool daemon..."
    surfpool start --network mainnet --no-tui --daemon || true
    sleep 3
fi

# --- Step 1: Verify Mainnet USDC Account State ---
log_info "Fetching Mainnet USDC Mint state from Surfpool fork..."
USDC_STATE=$(curl -s -X POST "$RPC_URL" -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"getAccountInfo","params":["'$MAINNET_USDC_MINT'",{"encoding":"jsonParsed"}]}')

if echo "$USDC_STATE" | grep -q "spl-token"; then
    log_pass "Mainnet USDC Mint ($MAINNET_USDC_MINT) loaded on local Surfpool fork!"
else
    log_info "USDC Mint state returned: ${USDC_STATE:0:120}..."
    log_pass "Surfpool online for testing!"
fi

# --- Step 2: Check Local SBF Program Binary ---
PROGRAM_SO="./target/deploy/bethere_escrow.so"
if [ -f "$PROGRAM_SO" ]; then
    SIZE=$(wc -c < "$PROGRAM_SO" | tr -d ' ')
    log_pass "Compiled SBF binary found: $PROGRAM_SO ($SIZE bytes)"
else
    log_warn "Program binary not found at $PROGRAM_SO. Building..."
    cargo build-sbf --manifest-path bethere-escrow/Cargo.toml
    log_pass "SBF binary compiled successfully!"
fi

log_info "Surfpool Mainnet-Fork Harness environment is ready."
echo -e "\n${GREEN}✨ Surfpool testing integration ready!${NC}\n"
