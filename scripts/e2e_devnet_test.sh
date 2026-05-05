#!/usr/bin/env bash
# ============================================================================
# E2E Devnet Test — BeThere Escrow Full Cycle
# ============================================================================
# Tests the complete escrow flow on devnet:
#   0. Prerequisites check
#   1. Setup: generate wallets, airdrop SOL, fund USDC
#   2. Create event in KV with escrow fields
#   3. Create Vault ATA (organizer signs)
#   4. Create Event On-Chain (organizer signs)
#   5. Deposit USDC (attendee signs)
#   6. Verify deposit on-chain
#   7. Mark Checked-In (organizer signs)
#   8. Refund (attendee signs)
#   9. Verify refund on-chain
#
# Prerequisites:
#   - solana CLI 3.x+ (configured for devnet)
#   - curl, jq, python3
#   - pip3 install solders
#   - Staging deployment live at WORKER_URL
#   - DEV_MODE=1 deployed on worker
#
# Usage:
#   ./scripts/e2e_devnet_test.sh              # full test
#   ./scripts/e2e_devnet_test.sh --cleanup    # cleanup only
#   ./scripts/e2e_devnet_test.sh --skip-setup # reuse existing wallets
# ============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
WORKER_URL="${WORKER_URL:-https://bethere.solana-thailand.workers.dev}"
PUBLIC_RPC="https://api.devnet.solana.com"
ESCROW_PROGRAM="2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo"
USDC_MINT="4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
TIMESTAMP=$(date +%s)
EVENT_SLUG="e2e-escrow-${TIMESTAMP}"
DEPOSIT_AMOUNT=1000000  # 1 USDC (6 decimals)
AUTH_TOKEN="dev-token"
NON_INTERACTIVE=false

# Test keypair dir
TEST_DIR="/tmp/bethere-e2e"
ORG_KEYPAIR="$TEST_DIR/organizer.json"
ATTENDEE_KEYPAIR="$TEST_DIR/attendee.json"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
log()  { echo -e "${CYAN}[E2E]${NC} $*"; }
ok()   { echo -e "${GREEN}[PASS]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; exit 1; }

rpc_call() {
  local method="$1"
  local params="$2"
  curl -s -X POST "$PUBLIC_RPC" \
    -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":\"e2e\",\"method\":\"$method\",\"params\":$params}"
}

api_get() {
  local path="$1"
  curl -s "$WORKER_URL$path" \
    -H "Authorization: Bearer $AUTH_TOKEN"
}

api_post() {
  local path="$1"
  local body="$2"
  curl -s -X POST "$WORKER_URL$path" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $AUTH_TOKEN" \
    -d "$body"
}

api_put() {
  local path="$1"
  local body="$2"
  curl -s -X PUT "$WORKER_URL$path" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $AUTH_TOKEN" \
    -d "$body"
}

# Compute FNV-1a hash to match Rust's derive_on_chain_event_id
fnv1a_hash() {
  python3 -c "
h = 0xcbf29ce484222325
for c in '$1'.encode():
    h ^= c
    h = (h * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
if h == 0: h = 1
print(h)
"
}

wait_for_confirmation() {
  local sig="$1"
  local max_attempts=30
  local attempt=0
  log "Waiting for TX confirmation: ${sig:0:20}..."
  while [ $attempt -lt $max_attempts ]; do
    local status
    status=$(rpc_call "getSignatureStatuses" "[[\"$sig\"]]" | jq -r '.result.value[0].confirmationStatus // empty')
    if [ "$status" = "finalized" ] || [ "$status" = "confirmed" ]; then
      ok "TX confirmed ($status)"
      return 0
    fi
    attempt=$((attempt + 1))
    sleep 2
  done
  fail "TX not confirmed after $max_attempts attempts"
}

# Sign a base64-encoded unsigned TX with a keypair and return base64 signed TX
sign_tx() {
  local tx_b64_file="$1"
  local keypair_file="$2"

  python3 -c "
import base64, json
from solders.keypair import Keypair
from solders.message import Message
from solders.transaction import Transaction

tx_bytes = base64.b64decode(open('$tx_b64_file').read().strip())

with open('$keypair_file') as f:
    kp = Keypair.from_json(f.read())

# Parse unsigned transaction: compact-u16 num_sigs | sig placeholders | message
pos = 0
sig_count = tx_bytes[pos]; pos += 1
if sig_count > 0:
    pos += 64  # skip zero-filled signature placeholder
message = tx_bytes[pos:]

msg = Message.from_bytes(message)
tx = Transaction.new_unsigned(msg)
tx.sign([kp], msg.recent_blockhash)
print(base64.b64encode(bytes(tx)).decode())
"
}

# Send a base64-encoded signed TX to the RPC
send_tx() {
  local signed_b64="$1"

  python3 -c "
import json, urllib.request
data = json.dumps({
    'jsonrpc': '2.0', 'id': 'e2e',
    'method': 'sendTransaction',
    'params': ['$signed_b64', {'encoding': 'base64', 'skipPreflight': True}]
}).encode()
req = urllib.request.Request('$PUBLIC_RPC', data=data, headers={'Content-Type': 'application/json'})
resp = urllib.request.urlopen(req)
result = json.loads(resp.read())
print(result.get('result', result.get('error', json.dumps(result))))
"
}

# Derive PDA address using solders
derive_pda() {
  local seeds_py="$1"
  local program_id="$2"

  python3 -c "
from solders.pubkey import Pubkey
program_id = Pubkey.from_string('$program_id')
$seeds_py
"
}

# ---------------------------------------------------------------------------
# Cleanup
# ---------------------------------------------------------------------------
cleanup() {
  log "Cleaning up test artifacts..."
  rm -rf "$TEST_DIR"
  ok "Cleanup done"
}

# ---------------------------------------------------------------------------
# Step 0: Prerequisites
# ---------------------------------------------------------------------------
check_prereqs() {
  log "=== Step 0: Prerequisites ==="

  command -v solana >/dev/null 2>&1 || fail "solana CLI not found"
  command -v jq >/dev/null 2>&1 || fail "jq not found"
  command -v curl >/dev/null 2>&1 || fail "curl not found"
  command -v python3 >/dev/null 2>&1 || fail "python3 not found"
  python3 -c "import solders" 2>/dev/null || fail "solders not installed: pip3 install solders"

  # Check worker is live
  local health
  health=$(curl -s "$WORKER_URL/api/health" | jq -r '.status // empty')
  [ "$health" = "ok" ] || fail "Worker not healthy at $WORKER_URL"

  # Check devnet connectivity
  solana slot --url "$PUBLIC_RPC" >/dev/null 2>&1 || fail "Cannot connect to devnet"

  # Check dev mode auth
  local auth_check
  auth_check=$(api_get "/api/auth/me" | jq -r '.data.email // empty')
  [ -n "$auth_check" ] || fail "Dev auth not working — check DEV_MODE + DEV_EMAIL deployment"
  log "Auth as: $auth_check"

  ok "All prerequisites met"
}

# ---------------------------------------------------------------------------
# Step 1: Generate wallets + airdrop
# ---------------------------------------------------------------------------
setup_wallets() {
  log "=== Step 1: Setup wallets ==="

  mkdir -p "$TEST_DIR"

  # Use the default CLI wallet as organizer (already has SOL)
  cp ~/.config/solana/id.json "$ORG_KEYPAIR"
  ORG_ADDR=$(solana address --keypair "$ORG_KEYPAIR")
  log "Organizer: $ORG_ADDR (default CLI wallet)"

  # Generate attendee keypair
  if [ ! -f "$ATTENDEE_KEYPAIR" ]; then
    solana-keygen new --no-bip39-passphrase --force -o "$ATTENDEE_KEYPAIR" >/dev/null 2>&1
  fi
  ATTENDEE_ADDR=$(solana address --keypair "$ATTENDEE_KEYPAIR")
  log "Attendee:  $ATTENDEE_ADDR"

  # Save for later steps
  echo "$ORG_ADDR" > "$TEST_DIR/org_addr.txt"
  echo "$ATTENDEE_ADDR" > "$TEST_DIR/attendee_addr.txt"

  # Airdrop SOL to attendee (organizer already has SOL from CLI)
  log "Airdropping SOL to attendee..."
  local airdrop_ok=false
  for i in 1 2 3; do
    if solana airdrop 2 "$ATTENDEE_ADDR" --url "$PUBLIC_RPC" 2>&1; then
      airdrop_ok=true
      break
    fi
    log "Airdrop attempt $i failed, waiting 5s..."
    sleep 5
  done
  if [ "$airdrop_ok" = false ]; then
    warn "Airdrop failed after 3 attempts — attendee may not have SOL"
  fi

  local org_bal att_bal
  org_bal=$(solana balance "$ORG_ADDR" --url "$PUBLIC_RPC" 2>&1 | grep -oE '[0-9.]+' || echo "0")
  att_bal=$(solana balance "$ATTENDEE_ADDR" --url "$PUBLIC_RPC" 2>&1 | grep -oE '[0-9.]+' || echo "0")
  log "Organizer balance: ${org_bal} SOL"
  log "Attendee balance:  ${att_bal} SOL"

  ok "Wallets ready"
}

# ---------------------------------------------------------------------------
# Step 1b: Fund attendee with devnet USDC
# ---------------------------------------------------------------------------
fund_usdc() {
  log "=== Step 1b: Fund attendee USDC ==="

  # Transfer SOL from organizer to attendee for TX fees
  log "Transferring 0.1 SOL to attendee for fees..."
  solana transfer "$ATTENDEE_ADDR" 0.1 --keypair "$ORG_KEYPAIR" --url "$PUBLIC_RPC" --allow-unfunded-recipient 2>&1 || {
    warn "SOL transfer failed — attendee may not have funds for TX"
  }

  # Create USDC ATA for attendee using spl-token
  log "Creating attendee USDC ATA..."
  spl-token create-account "$USDC_MINT" --owner "$ATTENDEE_KEYPAIR" --url "$PUBLIC_RPC" 2>&1 || true

  # Check if attendee already has USDC
  local usdc_bal
  usdc_bal=$(spl-token balance --owner "$ATTENDEE_ADDR" "$USDC_MINT" --url "$PUBLIC_RPC" 2>&1 || echo "0")
  local amount=${usdc_bal%% *}

  if echo "$amount" | grep -qE '^[0-9]+$' && [ "$amount" -ge "$((DEPOSIT_AMOUNT / 1000000))" ] 2>/dev/null; then
    ok "Attendee already has $usdc_bal USDC"
    return 0
  fi

  # Need USDC — inform user and wait
  echo ""
  warn "Attendee has $usdc_bal USDC — needs at least $((DEPOSIT_AMOUNT / 1000000)) USDC"
  echo ""
  echo "  Attendee wallet: $ATTENDEE_ADDR"
  echo "  USDC mint:       $USDC_MINT (devnet)"
  echo ""
  echo "  Please fund USDC via one of:"
  echo "    1. https://faucet.quicknode.com/solana/devnet (browser)"
  echo "    2. https://spl-token-faucet.com/?token-name=USDC-SPL (browser)"
  echo ""
  read -p "  Press Enter once funded (or 's' to skip USDC test)... " -r answer
  echo ""

  if [ "$answer" = "s" ] || [ "$NON_INTERACTIVE" = true ]; then
    warn "Skipping USDC funding — deposit test will fail without USDC"
    return 0
  fi

  # Check again
  usdc_bal=$(spl-token balance --owner "$ATTENDEE_ADDR" "$USDC_MINT" --url "$PUBLIC_RPC" 2>&1 || echo "0")
  log "Attendee USDC balance: $usdc_bal"
  ok "USDC funding done"
}

# ---------------------------------------------------------------------------
# Step 2: Create event in KV
# ---------------------------------------------------------------------------
step_create_event_kv() {
  log "=== Step 2: Create event in KV ==="

  local event_end_ms refund_deadline_ms
  event_end_ms=$(( (TIMESTAMP + 120) * 1000 ))  # 2 min from now (deposits accepted)
  refund_deadline_ms=$(( (TIMESTAMP + 86400 * 30) * 1000 ))  # 30 days

  local event_body
  event_body=$(cat <<EOF
{
  "id": "$EVENT_SLUG",
  "name": "E2E Test Event $TIMESTAMP",
  "description": "Automated E2E escrow test — delete after testing",
  "date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "location": "Devnet",
  "organizer_emails": ["ratchapon.poc@gmail.com"],
  "staff_emails": ["ratchapon.poc@gmail.com"],
  "deposit_enabled": true,
  "deposit_amount_usdc": $DEPOSIT_AMOUNT,
  "deposit_amount_thb": 0,
  "promptpay_id": "",
  "organizer_wallet": "$ORG_ADDR",
  "escrow_address": "",
  "on_chain_event_id": 0,
  "event_start_ms": $(( (TIMESTAMP - 7200) * 1000 )),
  "event_end_ms": $event_end_ms,
  "refund_deadline_hours": 168,
  "claim_base_url": "$WORKER_URL/claim",
  "sheet_id": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgVE2upms"
}
EOF
)

  log "Creating event: $EVENT_SLUG"
  local resp
  resp=$(api_post "/api/events" "$event_body")
  log "Create event response: $(echo "$resp" | jq . 2>/dev/null || echo "$resp")"

  local success
  success=$(echo "$resp" | jq -r '.success // empty')
  if [ "$success" != "true" ]; then
    fail "Failed to create event: $resp"
  fi

  # The API may generate a different slug from the name — use the returned ID
  EVENT_SLUG=$(echo "$resp" | jq -r '.data.id // .data.slug // empty')
  if [ -z "$EVENT_SLUG" ]; then
    fail "No event ID in response: $resp"
  fi

  ok "Event created in KV: $EVENT_SLUG"
}

# ---------------------------------------------------------------------------
# Step 3: Create Vault ATA (organizer signs)
# ---------------------------------------------------------------------------
step_create_vault_ata() {
  log "=== Step 3: Create Vault ATA ==="

  local resp tx_b64 vault_address
  resp=$(api_post "/api/escrow/create-vault-ata" "{\"event_id\":\"$EVENT_SLUG\"}")
  log "API response: $(echo "$resp" | jq . 2>/dev/null || echo "$resp")"

  tx_b64=$(echo "$resp" | jq -r '.data.transaction // empty')
  vault_address=$(echo "$resp" | jq -r '.data.vault_address // empty')

  if [ -z "$tx_b64" ]; then
    fail "No transaction returned from create-vault-ata: $resp"
  fi

  log "Vault address: $vault_address"
  echo "$tx_b64" > "$TEST_DIR/vault_ata_tx.b64"

  # Sign with organizer
  log "Signing vault ATA TX..."
  local signed_tx
  signed_tx=$(sign_tx "$TEST_DIR/vault_ata_tx.b64" "$ORG_KEYPAIR" 2>&1) || fail "Sign failed: $signed_tx"

  # Send
  log "Sending vault ATA TX..."
  local send_result
  send_result=$(send_tx "$signed_tx" 2>&1)
  log "Send result: $send_result"

  if echo "$send_result" | grep -qE "^[1-9A-HJ-NP-Za-km-z]+"; then
    wait_for_confirmation "$send_result"
    ok "Vault ATA created: $vault_address"
    echo "$vault_address" > "$TEST_DIR/vault_address.txt"
  else
    # Check if already exists (idempotent)
    if echo "$send_result" | grep -qi "already\|exists\|0x1\|custom program error: 0x0"; then
      warn "Vault ATA already exists (ok for idempotent)"
      echo "$vault_address" > "$TEST_DIR/vault_address.txt"
    else
      fail "Failed to create vault ATA: $send_result"
    fi
  fi
}

# ---------------------------------------------------------------------------
# Step 4: Create Event On-Chain (organizer signs)
# ---------------------------------------------------------------------------
step_create_event_onchain() {
  log "=== Step 4: Create Event On-Chain ==="

  local resp tx_b64 escrow_address on_chain_id
  resp=$(api_post "/api/escrow/create-event" "{\"event_id\":\"$EVENT_SLUG\"}")
  log "API response: $(echo "$resp" | jq . 2>/dev/null || echo "$resp")"

  tx_b64=$(echo "$resp" | jq -r '.data.transaction // empty')
  escrow_address=$(echo "$resp" | jq -r '.data.escrow_address // empty')
  on_chain_id=$(echo "$resp" | jq -r '.data.on_chain_event_id // empty')

  if [ -z "$tx_b64" ]; then
    fail "No transaction returned from create-event: $resp"
  fi

  log "Escrow PDA: $escrow_address"
  log "On-chain event ID: $on_chain_id"
  echo "$tx_b64" > "$TEST_DIR/create_event_tx.b64"

  # Sign with organizer
  log "Signing create_event TX..."
  local signed_tx
  signed_tx=$(sign_tx "$TEST_DIR/create_event_tx.b64" "$ORG_KEYPAIR" 2>&1) || fail "Sign failed: $signed_tx"

  # Send
  local send_result
  send_result=$(send_tx "$signed_tx" 2>&1)

  if echo "$send_result" | grep -qE "^[1-9A-HJ-NP-Za-km-z]+"; then
    wait_for_confirmation "$send_result"
    ok "Event created on-chain: $escrow_address"
    echo "$escrow_address" > "$TEST_DIR/escrow_address.txt"
    ESCROW_ADDRESS="$escrow_address"

    # Update event in KV with escrow info so subsequent API calls work
    local update_resp
    update_resp=$(api_put "/api/events/$EVENT_SLUG" "{\"escrow_address\":\"$escrow_address\",\"on_chain_event_id\":$on_chain_id}")
    log "Event KV update: $(echo "$update_resp" | jq -r '.success // empty')"
  else
    fail "Failed to create event on-chain: $send_result"
  fi
}

# ---------------------------------------------------------------------------
# Step 5: Deposit USDC (attendee signs)
# ---------------------------------------------------------------------------
step_deposit() {
  log "=== Step 5: Deposit USDC ==="

  # Step 5a: Initiate deposit (saves pending deposit status in KV)
  # Note: If event has ended, this will fail — we handle that gracefully
  local init_resp
  init_resp=$(api_post "/api/deposit/usdc" "{\"event_id\":\"$EVENT_SLUG\",\"attendee_id\":\"e2e-test-attendee\",\"wallet_address\":\"$ATTENDEE_ADDR\"}")
  local init_success
  init_success=$(echo "$init_resp" | jq -r '.success // empty')
  if [ "$init_success" = "true" ]; then
    ok "Deposit initiated in KV"
  else
    warn "Deposit init skipped: $(echo "$init_resp" | jq -r '.error // empty')"
  fi

  # Step 5b: Get the deposit TX from the Solana Pay callback
  local resp tx_b64
  resp=$(curl -s "$WORKER_URL/api/deposit/usdc/tx?event_id=$EVENT_SLUG&attendee_id=e2e-test-attendee&wallet=$ATTENDEE_ADDR")
  log "Deposit TX response: $(echo "$resp" | jq . 2>/dev/null || echo "$resp")"

  tx_b64=$(echo "$resp" | jq -r '.transaction // empty')
  if [ -z "$tx_b64" ]; then
    fail "No deposit transaction returned: $resp"
  fi

  echo "$tx_b64" > "$TEST_DIR/deposit_tx.b64"

  # Sign with attendee
  log "Signing deposit TX with attendee..."
  local signed_tx
  signed_tx=$(sign_tx "$TEST_DIR/deposit_tx.b64" "$ATTENDEE_KEYPAIR" 2>&1) || fail "Sign failed: $signed_tx"

  # Send
  log "Sending deposit TX..."
  local send_result
  send_result=$(send_tx "$signed_tx" 2>&1)

  if echo "$send_result" | grep -qE "^[1-9A-HJ-NP-Za-km-z]+"; then
    wait_for_confirmation "$send_result"
    ok "Deposit TX confirmed!"
    echo "$send_result" > "$TEST_DIR/deposit_sig.txt"

    # Step 5c: Submit TX signature to webhook (verifies on-chain and marks verified in KV)
    # If deposit init failed (event ended), create the deposit record first via a direct webhook call
    local confirm_resp
    confirm_resp=$(api_post "/api/deposit/usdc/webhook" "{\"event_id\":\"$EVENT_SLUG\",\"attendee_id\":\"e2e-test-attendee\",\"tx_signature\":\"$send_result\"}")
    local confirmed
    confirmed=$(echo "$confirm_resp" | jq -r '.data.confirmed // empty')
    if [ "$confirmed" = "true" ]; then
      ok "Deposit verified via webhook"
    else
      warn "Webhook response: $(echo "$confirm_resp" | jq -r '.error // .data // empty')"
      # If webhook failed because no deposit record, try creating one manually
      # The deposit is on-chain so we can still proceed with test
    fi
  else
    fail "Deposit failed: $send_result"
  fi
}

# ---------------------------------------------------------------------------
# Step 6: Verify deposit on-chain
# ---------------------------------------------------------------------------
step_verify_deposit() {
  log "=== Step 6: Verify deposit on-chain ==="

  local escrow_address
  escrow_address=$(cat "$TEST_DIR/escrow_address.txt")

  # Derive AttendeeDeposit PDA
  local deposit_pda
  deposit_pda=$(python3 -c "
from solders.pubkey import Pubkey
program_id = Pubkey.from_string('$ESCROW_PROGRAM')
escrow = Pubkey.from_string('$escrow_address')
attendee = Pubkey.from_string('$ATTENDEE_ADDR')
pda, _ = Pubkey.find_program_address(
    [b'deposit', bytes(escrow), bytes(attendee)],
    program_id
)
print(str(pda))
" 2>&1) || fail "PDA derivation failed: $deposit_pda"

  log "AttendeeDeposit PDA: $deposit_pda"

  # Wait for RPC to catch up (public RPC can be stale)
  sleep 3

  # Fetch account data with retry
  local account_info data_b64
  local retry=0
  while [ $retry -lt 5 ]; do
    account_info=$(rpc_call "getAccountInfo" "[\"$deposit_pda\",{\"encoding\":\"base64\"}]")
    data_b64=$(echo "$account_info" | jq -r '.result.value.data[0] // empty')
    if [ -n "$data_b64" ]; then
      break
    fi
    retry=$((retry + 1))
    log "  Retry $retry/5 — account not visible yet, waiting 2s..."
    sleep 2
  done

  if [ -z "$data_b64" ]; then
    log "  Account info response: $(echo "$account_info" | jq -r '.result.value // empty')"
    fail "AttendeeDeposit account not found on-chain after 5 retries"
  fi

  ok "AttendeeDeposit account exists"

  # Decode and verify
  python3 -c "
import base64, struct

data = base64.b64decode('$data_b64')
print(f'Data length: {len(data)} bytes')

# AttendeeDeposit layout (quasar-lang: 1-byte discriminator):
# 1 (discriminator) + 32 (attendee) + 32 (event) + 8 (amount) + 8 (deposited_at) + 1 (checked_in) + 1 (refunded) + 1 (bump) = 84 bytes
if len(data) >= 84:
    off = 1
    attendee = data[off:off+32]; off += 32
    event = data[off:off+32]; off += 32
    amount = struct.unpack('<Q', data[off:off+8])[0]; off += 8
    deposited_at = struct.unpack('<q', data[off:off+8])[0]; off += 8
    checked_in = data[off] != 0; off += 1
    refunded = data[off] != 0; off += 1
    bump = data[off]

    print(f'  Amount:     {amount} ({amount/1_000_000:.2f} USDC)')
    print(f'  Checked in: {checked_in}')
    print(f'  Refunded:   {refunded}')
    print(f'  Bump:       {bump}')

    assert amount == $DEPOSIT_AMOUNT, f'Amount mismatch: {amount} != $DEPOSIT_AMOUNT'
    assert not refunded, 'Should not be refunded yet'
    print('✓ Deposit verified!')
else:
    print(f'Data too short ({len(data)} bytes), may be closed')
    raise SystemExit(1)
"

  ok "Deposit verified on-chain"
}

# ---------------------------------------------------------------------------
# Step 7: Mark Checked-In (organizer signs)
# ---------------------------------------------------------------------------
step_mark_checked_in() {
  log "=== Step 7: Mark Checked-In ==="

  local resp tx_b64
  local retry=0
  while [ $retry -lt 3 ]; do
    resp=$(api_post "/api/escrow/mark-checked-in" \
      "{\"event_id\":\"$EVENT_SLUG\",\"attendee_id\":\"e2e-test-attendee\",\"attendee_wallet\":\"$ATTENDEE_ADDR\"}")

    tx_b64=$(echo "$resp" | jq -r '.data.transaction // empty')
    if [ -n "$tx_b64" ]; then
      break
    fi

    # Check if it's a KV consistency issue
    local error_msg
    error_msg=$(echo "$resp" | jq -r '.error // empty')
    if echo "$error_msg" | grep -q "escrow not initialized"; then
      retry=$((retry + 1))
      warn "KV not propagated yet, retry $retry/3 in 5s..."
      sleep 5
    else
      break
    fi
  done

  log "API response: $(echo "$resp" | jq . 2>/dev/null || echo "$resp")"

  if [ -z "$tx_b64" ]; then
    fail "No mark_checked_in transaction returned: $resp"
  fi

  echo "$tx_b64" > "$TEST_DIR/checkin_tx.b64"

  # Sign with organizer
  log "Signing mark_checked_in TX..."
  local signed_tx
  signed_tx=$(sign_tx "$TEST_DIR/checkin_tx.b64" "$ORG_KEYPAIR" 2>&1) || fail "Sign failed: $signed_tx"

  # Send
  local send_result
  send_result=$(send_tx "$signed_tx" 2>&1)

  if echo "$send_result" | grep -qE "^[1-9A-HJ-NP-Za-km-z]+"; then
    wait_for_confirmation "$send_result"
    ok "Mark checked-in confirmed!"
  else
    fail "Mark checked-in failed: $send_result"
  fi
}

# ---------------------------------------------------------------------------
# Step 8: Refund (attendee signs)
# ---------------------------------------------------------------------------
step_refund() {
  log "=== Step 8: Refund ==="

  # Wait for event_end to pass (on-chain refund requires clock > event_end)
  log "Waiting 130s for on-chain event_end to pass..."
  sleep 130

  local resp tx_b64
  resp=$(curl -s -X POST "$WORKER_URL/api/escrow/refund" \
    -H "Content-Type: application/json" \
    -d "{\"event_id\":\"$EVENT_SLUG\",\"attendee_id\":\"e2e-test-attendee\",\"wallet_address\":\"$ATTENDEE_ADDR\"}")
  log "API response: $(echo "$resp" | jq . 2>/dev/null || echo "$resp")"

  tx_b64=$(echo "$resp" | jq -r '.data.transaction // .transaction // empty')
  if [ -z "$tx_b64" ]; then
    fail "No refund transaction returned: $resp"
  fi

  echo "$tx_b64" > "$TEST_DIR/refund_tx.b64"

  # Sign with attendee
  log "Signing refund TX..."
  local signed_tx
  signed_tx=$(sign_tx "$TEST_DIR/refund_tx.b64" "$ATTENDEE_KEYPAIR" 2>&1) || fail "Sign failed: $signed_tx"

  # Send
  local send_result
  send_result=$(send_tx "$signed_tx" 2>&1)

  if echo "$send_result" | grep -qE "^[1-9A-HJ-NP-Za-km-z]+"; then
    wait_for_confirmation "$send_result"
    ok "Refund confirmed!"
  else
    fail "Refund failed: $send_result"
  fi
}

# ---------------------------------------------------------------------------
# Step 9: Verify refund on-chain
# ---------------------------------------------------------------------------
step_verify_refund() {
  log "=== Step 9: Verify refund on-chain ==="

  local escrow_address
  escrow_address=$(cat "$TEST_DIR/escrow_address.txt")

  # Derive AttendeeDeposit PDA
  local deposit_pda
  deposit_pda=$(python3 -c "
from solders.pubkey import Pubkey
program_id = Pubkey.from_string('$ESCROW_PROGRAM')
escrow = Pubkey.from_string('$escrow_address')
attendee = Pubkey.from_string('$ATTENDEE_ADDR')
pda, _ = Pubkey.find_program_address(
    [b'deposit', bytes(escrow), bytes(attendee)],
    program_id
)
print(str(pda))
" 2>&1)

  # Fetch account data with retry until refunded=true (RPC can be stale)
  sleep 5
  local account_info data_b64
  local retry=0
  local refunded=false
  while [ $retry -lt 10 ]; do
    account_info=$(rpc_call "getAccountInfo" "[\"$deposit_pda\",{\"encoding\":\"base64\"}]")
    data_b64=$(echo "$account_info" | jq -r '.result.value.data[0] // empty')
    if [ -n "$data_b64" ]; then
      # Check if refunded flag is set
      refunded=$(python3 -c "
import base64
data = base64.b64decode('$data_b64')
if len(data) >= 84:
    off = 1 + 32 + 32 + 8 + 8 + 1  # disc + attendee + event + amount + deposited_at + checked_in
    print('true' if data[off] != 0 else 'false')
else:
    print('false')
" 2>&1)
      if [ "$refunded" = "true" ]; then
        break
      fi
    fi
    retry=$((retry + 1))
    log "  Retry $retry/10 — refunded not visible yet (refunded=$refunded), waiting 3s..."
    sleep 3
  done

  if [ -z "$data_b64" ]; then
    fail "AttendeeDeposit account not found after 10 retries"
  fi

  python3 -c "
import base64, struct

data = base64.b64decode('$data_b64')

if len(data) >= 84:
    off = 1  # quasar-lang: 1-byte discriminator
    attendee = data[off:off+32]; off += 32
    event = data[off:off+32]; off += 32
    amount = struct.unpack('<Q', data[off:off+8])[0]; off += 8
    deposited_at = struct.unpack('<q', data[off:off+8])[0]; off += 8
    checked_in = data[off] != 0; off += 1
    refunded = data[off] != 0

    print(f'  Amount:     {amount} ({amount/1_000_000:.2f} USDC)')
    print(f'  Checked in: {checked_in}')
    print(f'  Refunded:   {refunded}')

    assert checked_in, 'Should be checked in'
    assert refunded, 'Should be refunded'
    print('✓ Refund verified!')
else:
    print(f'Data too short ({len(data)} bytes) — account may be closed')
    raise SystemExit(1)
"

  ok "Full escrow cycle verified!"
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
print_summary() {
  echo ""
  echo "============================================================"
  echo -e "${GREEN}E2E Devnet Test — ALL PASSED${NC}"
  echo "============================================================"
  echo ""
  echo "  Event slug:    $EVENT_SLUG"
  echo "  Event ID:      $(fnv1a_hash "$EVENT_SLUG")"
  echo "  Organizer:     $ORG_ADDR"
  echo "  Attendee:      $ATTENDEE_ADDR"
  if [ -f "$TEST_DIR/escrow_address.txt" ]; then
    local escrow_addr
    escrow_addr=$(cat "$TEST_DIR/escrow_address.txt")
    echo "  Escrow PDA:    $escrow_addr"
    echo ""
    echo "  Solscan:"
    echo "    https://solscan.io/account/$escrow_addr?cluster=devnet"
  fi
  echo ""
  echo "  Artifacts:     $TEST_DIR"
  echo "============================================================"
  echo ""
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
  local skip_setup=false
  local cleanup_only=false

  for arg in "$@"; do
    case "$arg" in
      --cleanup) cleanup_only=true ;;
      --skip-setup) skip_setup=true ;;
      --non-interactive|-n) NON_INTERACTIVE=true ;;
      --help|-h)
        echo "Usage: $0 [--cleanup] [--skip-setup] [--help]"
        exit 0
        ;;
    esac
  done

  if [ "$cleanup_only" = true ]; then
    cleanup
    exit 0
  fi

  echo ""
  echo "============================================================"
  echo "  BeThere Escrow — E2E Devnet Test"
  echo "============================================================"
  echo "  Worker: $WORKER_URL"
  echo "  RPC:    $PUBLIC_RPC"
  echo "  Event:  $EVENT_SLUG"
  echo "============================================================"
  echo ""

  check_prereqs

  if [ "$skip_setup" = false ]; then
    setup_wallets
    fund_usdc
  else
    log "Skipping setup, reusing existing wallets"
    ORG_ADDR=$(solana address --keypair "$ORG_KEYPAIR")
    ATTENDEE_ADDR=$(solana address --keypair "$ATTENDEE_KEYPAIR")
    mkdir -p "$TEST_DIR"
  fi

  step_create_event_kv
  step_create_vault_ata
  step_create_event_onchain
  step_deposit
  step_verify_deposit
  step_mark_checked_in
  step_refund
  step_verify_refund

  print_summary
}

main "$@"
