#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# BeThere Escrow — Surfpool Integration Tests
#
# Prerequisites:
#   - surfpool running with mainnet fork on SURFPOOL_RPC (default :8898)
#   - solana CLI configured
#   - python3 with requests (for JSON-RPC calls)
#
# Usage:
#   # Terminal 1: Start surfpool
#   surfpool start --network mainnet --no-tui --port 8898 --no-deploy
#
#   # Terminal 2: Run integration tests
#   bash tests/integration/run.sh
# ---------------------------------------------------------------------------
set -euo pipefail

RPC="${TEST_RPC:-http://127.0.0.1:8899}"
PASS=0
FAIL=0
TOTAL=0

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log_pass() { PASS=$((PASS + 1)); TOTAL=$((TOTAL + 1)); echo -e "  ${GREEN}✅ PASS${NC} $1"; }
log_fail() { FAIL=$((FAIL + 1)); TOTAL=$((TOTAL + 1)); echo -e "  ${RED}❌ FAIL${NC} $1 — $2"; }
log_step() { echo -e "\n${CYAN}▸${NC} $1"; }

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

rpc_call() {
    local method="$1"
    local params="${2:-[]}"
    # Use confirmed commitment to avoid finalized-slot issue in test validators
    curl -s "$RPC" \
        -X POST \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$method\",\"params\":$params}"
}

get_balance() {
    local pubkey="$1"
    rpc_call "getBalance" "[\"$pubkey\", {\"commitment\": \"confirmed\"}]" | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['value'])" 2>/dev/null
}

airdrop() {
    local pubkey="$1"
    local amount="${2:-10000000000}"  # 10 SOL default
    rpc_call "requestAirdrop" "[\"$pubkey\", $amount]" | python3 -c "import sys,json; print(json.load(sys.stdin)['result'])" 2>/dev/null
}

get_account_info() {
    local pubkey="$1"
    rpc_call "getAccountInfo" "[\"$pubkey\", {\"encoding\": \"base64\", \"commitment\": \"confirmed\"}]" | python3 -c "
import sys, json, base64
data = json.load(sys.stdin)
if data.get('result') and data['result']['value']:
    raw = base64.b64decode(data['result']['value']['data'][0])
    print(f'owner={data[\"result\"][\"value\"][\"owner\"]} len={len(raw)} lamports={data[\"result\"][\"value\"][\"lamports\"]} exec={data[\"result\"][\"value\"][\"executable\"]}')
else:
    print('NONE')
" 2>/dev/null
}

wait_for_confirmation() {
    local sig="$1"
    for _ in $(seq 1 10); do
        local status
        status=$(rpc_call "getSignatureStatuses" "[[\"$sig\"]]" | python3 -c "
import sys, json
data = json.load(sys.stdin)
s = data['result']['value'][0]
if s and s.get('confirmationStatus') in ('confirmed', 'finalized'):
    print('OK')
elif s and s.get('err'):
    print('ERR:' + str(s['err']))
else:
    print('PENDING')
" 2>/dev/null)
        if [ "$status" = "OK" ]; then return 0; fi
        if [[ "$status" == ERR:* ]]; then echo "TX failed: $status"; return 1; fi
        sleep 0.5
    done
    echo "Timeout waiting for $sig"
    return 1
}

# Program & key constants
PROGRAM_ID="C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T"
USDC_MINT="EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
TOKEN_PROGRAM="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
ATA_PROGRAM="ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
SYSTEM_PROGRAM="11111111111111111111111111111111"
RENT_SYSVAR="SysvarRent111111111111111111111111111111111"

# Solana CLI config for this test validator
export SOLANA_RPC_URL="$RPC"

echo "=============================================="
echo "  BeThere Escrow — Integration Tests"
echo "=============================================="
echo "  RPC: $RPC"
echo "  Program: $PROGRAM_ID"
echo ""

# ---------------------------------------------------------------------------
# Test 1: RPC Health Check
# ---------------------------------------------------------------------------
log_step "Test 1: Surfpool health check"
HEALTH=$(rpc_call "getHealth" | python3 -c "import sys,json; print(json.load(sys.stdin).get('result','ERR'))" 2>/dev/null || echo "ERR")
if [ "$HEALTH" = "ok" ]; then
    log_pass "Surfpool RPC healthy"
else
    log_fail "Surfpool health check" "got '$HEALTH'"
    echo -e "\n${RED}Cannot proceed without Surfpool. Start it first:${NC}"
    echo "  surfpool start --network mainnet --no-tui --port 8898 --no-deploy"
    exit 1
fi

# ---------------------------------------------------------------------------
# Test 2: Program deployed and executable
# ---------------------------------------------------------------------------
log_step "Test 2: Escrow program deployed"
PROG_INFO=$(get_account_info "$PROGRAM_ID")
if [[ "$PROG_INFO" != *"NONE"* ]] && [[ "$PROG_INFO" == *"BPFLoader"* ]]; then
    log_pass "Escrow program deployed ($PROG_INFO)"
else
    log_fail "Escrow program not found" "$PROG_INFO — deploy with: solana-test-validator --bpf-program $PROGRAM_ID bethere_escrow.so"
fi

# ---------------------------------------------------------------------------
# Test 3: Create mock USDC mint (local validator doesn't have mainnet mints)
# ---------------------------------------------------------------------------
log_step "Test 3: Create mock USDC mint (6 decimals)"
# spl-token create-mint will set up a token mint account
MINT_RESULT=$(spl-token --url "$RPC" create-token --decimals 6 2>&1) || true
USDC_MINT=$(echo "$MINT_RESULT" | grep "^Address:" | awk '{print $2}')
if [ -n "$USDC_MINT" ]; then
    log_pass "Mock USDC mint created: $USDC_MINT"
else
    log_fail "Mock USDC mint creation" "$MINT_RESULT"
fi

# ---------------------------------------------------------------------------
# Test 4: Create keypairs and fund wallets
# ---------------------------------------------------------------------------
log_step "Test 4: Fund organizer and attendee wallets"

# Generate test keypairs
ORGANIZER=$(solana-keygen new --no-bip39-passphrase -o /tmp/bethere-org.json --force 2>/dev/null | grep "^pubkey:" | awk '{print $2}')
ATTENDEE=$(solana-keygen new --no-bip39-passphrase -o /tmp/bethere-att.json --force 2>/dev/null | grep "^pubkey:" | awk '{print $2}')

# Fund wallets — transfer SOL from the genesis-funded default keypair
solana --url "$RPC" transfer --allow-unfunded-recipient "$ORGANIZER" 5 2>/dev/null
sleep 1
solana --url "$RPC" transfer --allow-unfunded-recipient "$ATTENDEE" 5 2>/dev/null
sleep 1

ORG_BAL=$(get_balance "$ORGANIZER")
ATT_BAL=$(get_balance "$ATTENDEE")

if [ "$ORG_BAL" -gt 0 ] 2>/dev/null; then
    log_pass "Organizer funded ($(( ORG_BAL / 1000000000 )) SOL)"
else
    log_fail "Organizer funding" "balance=$ORG_BAL"
fi

if [ "$ATT_BAL" -gt 0 ] 2>/dev/null; then
    log_pass "Attendee funded ($(( ATT_BAL / 1000000000 )) SOL)"
else
    log_fail "Attendee funding" "balance=$ATT_BAL"
fi

# ---------------------------------------------------------------------------
# Test 5: Verify mock USDC mint decimals
# ---------------------------------------------------------------------------
log_step "Test 5: USDC mint decimals check"
USDC_DATA=$(rpc_call "getAccountInfo" "[\"$USDC_MINT\", {\"encoding\": \"base64\", \"commitment\": \"confirmed\"}]" | python3 -c "
import sys, json, base64
data = json.load(sys.stdin)
r = data.get('result',{}).get('value')
if r:
    raw = base64.b64decode(r['data'][0])
    decimals = raw[44]
    supply = int.from_bytes(raw[36:44], 'little')
    print(f'decimals={decimals} supply={supply}')
else:
    print('NONE')
" 2>/dev/null)
if [[ "$USDC_DATA" == *"decimals=6"* ]]; then
    log_pass "USDC decimals = 6 ($USDC_DATA)"
else
    log_fail "USDC decimals check" "$USDC_DATA — expected 6"
fi

# ---------------------------------------------------------------------------
# Test 6: PDA derivation matches on-chain
# ---------------------------------------------------------------------------
log_step "Test 6: PDA derivation"

EVENT_ID=42

# Derive the event escrow PDA. Seeds match bethere-escrow/src/state.rs:
#   #[seeds(b"escrow", organizer: Address, event_id: u64)]
# The program id, organizer and event id are passed as argv, never interpolated
# into the program text (see .issues/065).
if ESCROW_PDA=$(python3 - "$PROGRAM_ID" "$ORGANIZER" "$EVENT_ID" 2>/dev/null <<'PYPDA'
import sys

from solders.pubkey import Pubkey

program = Pubkey.from_string(sys.argv[1])
organizer = Pubkey.from_string(sys.argv[2])
event_id = int(sys.argv[3])
pda, bump = Pubkey.find_program_address(
    [b"escrow", bytes(organizer), event_id.to_bytes(8, "little")], program
)
print(f"{pda} bump={bump}")
PYPDA
); then
    log_pass "Event escrow PDA (event_id=$EVENT_ID): $ESCROW_PDA"
else
    log_fail "PDA derivation" "solders not installed — pip install solders"
fi

# ---------------------------------------------------------------------------
# Test 7: program constants decode to canonical 32-byte pubkeys
# ---------------------------------------------------------------------------
log_step "Test 7: program constant addresses"

# Guards the constants above against transcription slips. Two of them were
# silently malformed until this check existed (.issues/072 item 5):
# TOKEN_PROGRAM was missing a "g" and SYSTEM_PROGRAM carried 41 ones, so both
# decoded to something other than a 32-byte key. Pure base58 — no dependency,
# so this runs even where solders is absent.
CONST_CHECK=$(python3 - \
    "PROGRAM_ID=$PROGRAM_ID" \
    "USDC_MINT=$USDC_MINT" \
    "TOKEN_PROGRAM=$TOKEN_PROGRAM" \
    "ATA_PROGRAM=$ATA_PROGRAM" \
    "SYSTEM_PROGRAM=$SYSTEM_PROGRAM" \
    "RENT_SYSVAR=$RENT_SYSVAR" <<'PYCONST'
import sys

ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def decode(text):
    """Decode base58 text to bytes. Raises ValueError on a foreign character."""
    acc = 0
    for char in text:
        acc = acc * 58 + ALPHABET.index(char)
    body = acc.to_bytes((acc.bit_length() + 7) // 8, "big") if acc else b""
    return b"\x00" * (len(text) - len(text.lstrip("1"))) + body


bad = []
for pair in sys.argv[1:]:
    name, _, value = pair.partition("=")
    try:
        width = len(decode(value))
    except ValueError as exc:
        bad.append(f"{name} is not base58 ({exc})")
        continue
    if width != 32:
        bad.append(f"{name} decodes to {width} bytes, not 32")

print("; ".join(bad) if bad else "OK")
PYCONST
)
if [[ "$CONST_CHECK" == "OK" ]]; then
    log_pass "All 6 program constants decode to 32 bytes"
else
    log_fail "Program constant addresses" "$CONST_CHECK"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "=============================================="
echo -e "  Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC}, $TOTAL total"
echo "=============================================="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi

echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "  1. Run full lifecycle test (create → deposit → check-in → refund → close)"
echo "  2. Run time-warp edge case tests (claim_forfeited after deadline)"
echo "  3. Set up Kora for gasless refund flow"
echo ""
echo -e "${CYAN}Wallet files:${NC}"
echo "  Organizer: /tmp/bethere-org.json ($ORGANIZER)"
echo "  Attendee:  /tmp/bethere-att.json ($ATTENDEE)"
echo ""
echo -e "${CYAN}Mock USDC Mint:${NC} $USDC_MINT"
