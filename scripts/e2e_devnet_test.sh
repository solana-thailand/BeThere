#!/usr/bin/env bash
# ============================================================================
# E2E Devnet Test — BeThere Escrow Full Cycle
# ============================================================================
# Tests the complete escrow flow on devnet:
#   0. Prerequisites check
#   1. Setup: generate wallets, airdrop SOL, fund USDC
#   2. Create event in KV with escrow fields
#   3. Initialize escrow on-chain: vault ATA + create_event (organizer signs)
#   4. Deposit USDC (attendee signs)
#   5. Verify deposit on-chain
#   6. Mark Checked-In (organizer signs)
#   7. Refund (attendee signs)
#   8. Verify refund on-chain
#
# Prerequisites:
#   - solana CLI 3.x+ (configured for devnet)
#   - curl, jq, python3
#   - pip3 install solders
#   - Staging deployment live at WORKER_URL (default: staging; prod refused)
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
# Defaults to STAGING, like flow-harness/src/context.rs. Step 2 writes an event
# into the target's KV, so production needs an explicit E2E_ALLOW_PROD=1.
WORKER_URL="${WORKER_URL:-https://bethere-staging.solana-thailand.workers.dev}"
PROD_URL="https://bethere.solana-thailand.workers.dev"
if [ "${WORKER_URL%/}" = "$PROD_URL" ] && [ "${E2E_ALLOW_PROD:-0}" != "1" ]; then
  echo "Refusing to run against production ($PROD_URL); set E2E_ALLOW_PROD=1 to override." >&2
  exit 2
fi
PUBLIC_RPC="https://api.devnet.solana.com"
ESCROW_PROGRAM="C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T"
USDC_MINT="4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
TIMESTAMP=$(date +%s)
EVENT_SLUG="e2e-escrow-${TIMESTAMP}"
# 2 min out: far enough to initialize and deposit (both reject a past
# event_end), near enough that step 7 can wait it out for the refund.
EVENT_END_S=$((TIMESTAMP + 120))
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

# Compute FNV-1a hash to match Rust's derive_on_chain_event_id.
# Intentionally FNV-1a — must match on-chain PDA seed derivation (see VULN-007).
# The slug arrives as argv, never interpolated into the program text (.issues/065).
fnv1a_hash() {
  python3 - "$1" <<'PYFNV'
import sys

h = 0xCBF29CE484222325
for c in sys.argv[1].encode():
    h ^= c
    h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
if h == 0:
    h = 1
print(h)
PYFNV
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

# Sign a base64-encoded unsigned TX with a keypair and return base64 signed TX.
# Both arguments are paths passed as argv; Python opens the keypair itself, so
# the secret key never reaches the process argument vector (.issues/073).
sign_tx() {
  local tx_b64_file="$1"
  local keypair_file="$2"

  python3 - "$tx_b64_file" "$keypair_file" <<'PYSIGN'
import base64
import sys

from solders.keypair import Keypair
from solders.message import Message
from solders.transaction import Transaction

with open(sys.argv[1]) as f:
    tx_bytes = base64.b64decode(f.read().strip())

with open(sys.argv[2]) as f:
    kp = Keypair.from_json(f.read())

# Parse unsigned transaction: compact-u16 num_sigs | sig placeholders | message
pos = 0
sig_count = tx_bytes[pos]
pos += 1
if sig_count > 0:
    pos += 64  # skip zero-filled signature placeholder
message = tx_bytes[pos:]

msg = Message.from_bytes(message)
tx = Transaction.new_unsigned(msg)
tx.sign([kp], msg.recent_blockhash)
print(base64.b64encode(bytes(tx)).decode())
PYSIGN
}

# Send a base64-encoded signed TX to the RPC.
send_tx() {
  local signed_b64="$1"

  python3 - "$signed_b64" "$PUBLIC_RPC" <<'PYSEND'
import json
import sys
import urllib.request

payload = json.dumps(
    {
        "jsonrpc": "2.0",
        "id": "e2e",
        "method": "sendTransaction",
        "params": [sys.argv[1], {"encoding": "base64", "skipPreflight": True}],
    }
).encode()
req = urllib.request.Request(
    sys.argv[2], data=payload, headers={"Content-Type": "application/json"}
)
result = json.loads(urllib.request.urlopen(req).read())
print(result.get("result", result.get("error", json.dumps(result))))
PYSEND
}

# Derive the AttendeeDeposit PDA. Seeds match bethere-escrow/src/state.rs:57 —
#   #[seeds(b"deposit", event: Address, attendee: Address)]
# where the "event" seed is the EventEscrow address. Addresses arrive as argv.
derive_deposit_pda() {
  local escrow_address="$1"
  local attendee_address="$2"

  python3 - "$ESCROW_PROGRAM" "$escrow_address" "$attendee_address" <<'PYDEPOSITPDA'
import sys

from solders.pubkey import Pubkey

program_id = Pubkey.from_string(sys.argv[1])
escrow = Pubkey.from_string(sys.argv[2])
attendee = Pubkey.from_string(sys.argv[3])
pda, _ = Pubkey.find_program_address(
    [b"deposit", bytes(escrow), bytes(attendee)], program_id
)
print(str(pda))
PYDEPOSITPDA
}

# Decode a base64 AttendeeDeposit account into shell-parseable `key=value` lines.
# Offsets are the tested ones from flow-harness/src/chain.rs:173, which match
# bethere-escrow/src/state.rs:57:
#   [0] discriminator=2, [1] version, [2..34] attendee, [34..66] event,
#   [66..74] amount, [74..82] deposited_at, [82] checked_in, [83] refunded,
#   [84] bump, [85..96] padding.
# The account data arrives as argv. Callers assert in shell, so the expected
# values are never interpolated into the program text.
decode_deposit() {
  python3 - "$1" <<'PYDECODE'
import base64
import struct
import sys

MIN_LEN = 96
DISCRIMINATOR = 2

data = base64.b64decode(sys.argv[1])
if len(data) < MIN_LEN:
    print(f"AttendeeDeposit too short: {len(data)} < {MIN_LEN}", file=sys.stderr)
    raise SystemExit(1)
if data[0] != DISCRIMINATOR:
    print(
        f"AttendeeDeposit discriminator = {data[0]}, expected {DISCRIMINATOR}",
        file=sys.stderr,
    )
    raise SystemExit(1)

print(f"len={len(data)}")
print(f"version={data[1]}")
print(f"amount={struct.unpack('<Q', data[66:74])[0]}")
print(f"deposited_at={struct.unpack('<q', data[74:82])[0]}")
print(f"checked_in={'true' if data[82] else 'false'}")
print(f"refunded={'true' if data[83] else 'false'}")
print(f"bump={data[84]}")
PYDECODE
}

# Pull one `key=value` field out of decode_deposit output.
deposit_field() {
  local fields="$1"
  local key="$2"
  printf '%s\n' "$fields" | sed -n "s/^${key}=//p"
}

# Poll the AttendeeDeposit account until `key` decodes to `want`, then print the
# decoded fields. Returns 1 if it never does. An undecodable account is retried,
# never read as "not yet" — public RPC lags behind the confirmation status.
poll_deposit_field() {
  local deposit_pda="$1" key="$2" want="$3"
  local attempt data_b64 fields
  for attempt in 1 2 3 4 5 6 7 8 9 10; do
    data_b64=$(rpc_call "getAccountInfo" "[\"$deposit_pda\",{\"encoding\":\"base64\"}]" \
      | jq -r '.result.value.data[0] // empty')
    if [ -n "$data_b64" ] && fields=$(decode_deposit "$data_b64" 2>/dev/null) \
      && [ "$(deposit_field "$fields" "$key")" = "$want" ]; then
      printf '%s\n' "$fields"
      return 0
    fi
    log "  Retry $attempt/10 — waiting for $key=$want on $deposit_pda..." >&2
    sleep 3
  done
  return 1
}

# Raw (base-unit) USDC held by an owner across all its token accounts for the
# mint. Base units, not the CLI's decimal string, so deltas compare exactly.
usdc_raw_balance() {
  rpc_call "getTokenAccountsByOwner" \
    "[\"$1\",{\"mint\":\"$USDC_MINT\"},{\"encoding\":\"jsonParsed\",\"commitment\":\"confirmed\"}]" \
    | jq -r '[.result.value[].account.data.parsed.info.tokenAmount.amount | tonumber] | add // 0'
}

# Raw balance of one token account (the vault ATA).
token_account_raw_balance() {
  rpc_call "getTokenAccountBalance" "[\"$1\",{\"commitment\":\"confirmed\"}]" \
    | jq -r '.result.value.amount // empty'
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

  # The refund deadline travels as `refund_deadline_hours`, relative to
  # event_end_ms — the events API has no absolute-timestamp field for it. The
  # Worker derives refund_deadline_ms = event_end_ms + hours * 3_600_000
  # (worker/src/handlers/deposit/usdc/handlers/status.rs:114). 168h = 7 days,
  # matching the Worker's own default (worker/src/db/events.rs:242).
  local event_end_ms refund_deadline_hours
  event_end_ms=$(( EVENT_END_S * 1000 ))
  refund_deadline_hours=168

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
  "refund_deadline_hours": $refund_deadline_hours,
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
# Step 3: Initialize the escrow on-chain (organizer signs)
# ---------------------------------------------------------------------------
# One transaction: create_associated_token_account_idempotent for the vault,
# then create_event. POST /api/escrow/init builds it; POST /api/escrow/confirm-init
# re-derives the PDA, checks it exists on-chain and only then writes
# escrow_address / on_chain_event_id / escrow_status back to the event. This is
# the same two calls the Manage Events escrow panel makes.
step_init_escrow() {
  log "=== Step 3: Initialize escrow on-chain ==="

  local resp tx_b64 escrow_address vault_address on_chain_id
  resp=$(api_post "/api/escrow/init" "{\"event_id\":\"$EVENT_SLUG\"}")
  log "API response: $(echo "$resp" | jq . 2>/dev/null || echo "$resp")"

  tx_b64=$(echo "$resp" | jq -r '.data.transaction // empty')
  escrow_address=$(echo "$resp" | jq -r '.data.escrow_address // empty')
  vault_address=$(echo "$resp" | jq -r '.data.vault_address // empty')
  on_chain_id=$(echo "$resp" | jq -r '.data.on_chain_event_id // empty')

  if [ -z "$tx_b64" ]; then
    fail "No transaction returned from escrow/init: $resp"
  fi

  log "Escrow PDA:        $escrow_address"
  log "Vault ATA:         $vault_address"
  log "On-chain event ID: $on_chain_id"
  echo "$tx_b64" > "$TEST_DIR/init_escrow_tx.b64"

  log "Signing init escrow TX..."
  local signed_tx
  signed_tx=$(sign_tx "$TEST_DIR/init_escrow_tx.b64" "$ORG_KEYPAIR" 2>&1) || fail "Sign failed: $signed_tx"

  local send_result
  send_result=$(send_tx "$signed_tx" 2>&1)
  log "Send result: $send_result"

  if ! echo "$send_result" | grep -qE "^[1-9A-HJ-NP-Za-km-z]+"; then
    fail "Failed to initialize escrow on-chain: $send_result"
  fi
  wait_for_confirmation "$send_result"

  # The file is the channel between steps, not a shell variable: steps 5, 6, 8
  # and the summary each re-read escrow_address.txt, so a single step can be
  # re-run in a fresh shell.
  echo "$escrow_address" > "$TEST_DIR/escrow_address.txt"
  echo "$vault_address" > "$TEST_DIR/vault_address.txt"

  local confirm_resp confirmed_address
  confirm_resp=$(api_post "/api/escrow/confirm-init" "{\"event_id\":\"$EVENT_SLUG\"}")
  confirmed_address=$(echo "$confirm_resp" | jq -r '.data.escrow_address // empty')
  if [ "$confirmed_address" != "$escrow_address" ]; then
    fail "escrow/confirm-init did not confirm $escrow_address: $confirm_resp"
  fi
  ok "Escrow initialized and confirmed: $escrow_address ($(echo "$confirm_resp" | jq -r '.data.escrow_status'))"
}

# ---------------------------------------------------------------------------
# Step 4: Deposit USDC (attendee signs)
# ---------------------------------------------------------------------------
step_deposit() {
  log "=== Step 4: Deposit USDC ==="

  # Step 4a: Initiate deposit (saves pending deposit status in KV)
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

  # Step 4b: Get the deposit TX from the Solana Pay callback
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

    # Step 4c: Submit the TX signature to the webhook. Verification is detached
    # (H8, usdc/handlers/webhook.rs): the first reply is always confirmed=false,
    # and re-sending the same signature is an idempotent no-op that answers
    # confirmed=true once the background check has marked the deposit verified.
    # So poll it — a record that never verifies is a failure, not a warning.
    local webhook_body confirm_resp confirmed="" attempt
    webhook_body="{\"event_id\":\"$EVENT_SLUG\",\"attendee_id\":\"e2e-test-attendee\",\"tx_signature\":\"$send_result\"}"
    for attempt in 1 2 3 4 5 6 7 8 9 10; do
      confirm_resp=$(api_post "/api/deposit/usdc/webhook" "$webhook_body")
      confirmed=$(echo "$confirm_resp" | jq -r '.data.confirmed // empty')
      [ "$confirmed" = "true" ] && break
      log "  Retry $attempt/10 — deposit not verified yet: $(echo "$confirm_resp" | jq -c '.error // .data' 2>/dev/null)"
      sleep 3
    done
    [ "$confirmed" = "true" ] || fail "Deposit never verified via webhook: $confirm_resp"
    ok "Deposit verified via webhook"
  else
    fail "Deposit failed: $send_result"
  fi
}

# ---------------------------------------------------------------------------
# Step 5: Verify deposit on-chain
# ---------------------------------------------------------------------------
step_verify_deposit() {
  log "=== Step 5: Verify deposit on-chain ==="

  local escrow_address
  escrow_address=$(cat "$TEST_DIR/escrow_address.txt")

  # Derive AttendeeDeposit PDA
  local deposit_pda
  deposit_pda=$(derive_deposit_pda "$escrow_address" "$ATTENDEE_ADDR" 2>&1) \
    || fail "PDA derivation failed: $deposit_pda"

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

  # Decode and verify. Assertions live in the shell so the expected amount is
  # never interpolated into the Python program text.
  local fields
  fields=$(decode_deposit "$data_b64") || fail "AttendeeDeposit decode failed"

  local amount checked_in refunded
  amount=$(deposit_field "$fields" amount)
  checked_in=$(deposit_field "$fields" checked_in)
  refunded=$(deposit_field "$fields" refunded)

  log "  Amount:     $amount ($((amount / 1000000)).$(printf '%06d' $((amount % 1000000))) USDC)"
  log "  Checked in: $checked_in"
  log "  Refunded:   $refunded"
  log "  Bump:       $(deposit_field "$fields" bump)"

  [ "$amount" = "$DEPOSIT_AMOUNT" ] || fail "Amount mismatch: $amount != $DEPOSIT_AMOUNT"
  [ "$refunded" = "false" ] || fail "Should not be refunded yet"
  ok "Deposit verified on-chain (version=$(deposit_field "$fields" version))"
}

# ---------------------------------------------------------------------------
# Step 6: Mark Checked-In (organizer signs)
# ---------------------------------------------------------------------------
step_mark_checked_in() {
  log "=== Step 6: Mark Checked-In ==="

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

  # Assert the flag here: step 7's refund also closes the AttendeeDeposit
  # account, so this is the last point at which checked_in can be read.
  local deposit_pda
  deposit_pda=$(derive_deposit_pda "$(cat "$TEST_DIR/escrow_address.txt")" "$ATTENDEE_ADDR" 2>&1) \
    || fail "PDA derivation failed: $deposit_pda"
  poll_deposit_field "$deposit_pda" checked_in true >/dev/null \
    || fail "AttendeeDeposit $deposit_pda never showed checked_in=true"
  ok "checked_in=true on-chain"
}

# ---------------------------------------------------------------------------
# Step 7: Refund (attendee signs)
# ---------------------------------------------------------------------------
step_refund() {
  log "=== Step 7: Refund ==="

  # refund requires clock >= event_end (bethere-escrow refund.rs:74). Wait
  # for that instant plus a margin for validator clock drift, not a fixed time.
  local wait_s=$(( EVENT_END_S + 15 - $(date +%s) ))
  if [ "$wait_s" -gt 0 ]; then
    log "Waiting ${wait_s}s for on-chain event_end to pass..."
    sleep "$wait_s"
  fi

  # Step 8 asserts the balance delta against this snapshot.
  usdc_raw_balance "$ATTENDEE_ADDR" > "$TEST_DIR/attendee_usdc_before_refund.txt"
  log "Attendee USDC before refund: $(cat "$TEST_DIR/attendee_usdc_before_refund.txt") (raw)"

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
# Step 8: Verify refund on-chain
# ---------------------------------------------------------------------------
step_verify_refund() {
  log "=== Step 8: Verify refund on-chain ==="

  # /api/escrow/refund builds refund + close_deposit in one transaction, so a
  # successful refund leaves no AttendeeDeposit to decode: the account is
  # closed and its rent returned. What proves the refund is therefore the
  # account being gone, the attendee's USDC rising by exactly the deposit, and
  # the vault (one depositor) back to zero. checked_in was asserted in step 6.
  local escrow_address vault_address deposit_pda
  escrow_address=$(cat "$TEST_DIR/escrow_address.txt")
  vault_address=$(cat "$TEST_DIR/vault_address.txt")
  deposit_pda=$(derive_deposit_pda "$escrow_address" "$ATTENDEE_ADDR" 2>&1) \
    || fail "PDA derivation failed: $deposit_pda"

  local attempt value="" before after vault
  for attempt in 1 2 3 4 5 6 7 8 9 10; do
    value=$(rpc_call "getAccountInfo" "[\"$deposit_pda\",{\"encoding\":\"base64\",\"commitment\":\"confirmed\"}]" \
      | jq -c '.result.value')
    [ "$value" = "null" ] && break
    log "  Retry $attempt/10 — AttendeeDeposit $deposit_pda still open, waiting 3s..."
    sleep 3
  done
  [ "$value" = "null" ] || fail "AttendeeDeposit $deposit_pda was not closed by the refund: $value"
  ok "AttendeeDeposit closed"

  before=$(cat "$TEST_DIR/attendee_usdc_before_refund.txt")
  after=$(usdc_raw_balance "$ATTENDEE_ADDR")
  log "  Attendee USDC: $before -> $after (raw)"
  [ $(( after - before )) -eq "$DEPOSIT_AMOUNT" ] \
    || fail "Attendee USDC rose by $(( after - before )), expected $DEPOSIT_AMOUNT"

  vault=$(token_account_raw_balance "$vault_address")
  log "  Vault USDC:    $vault (raw)"
  [ "$vault" = "0" ] || fail "Vault $vault_address still holds $vault after the only refund"

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
  step_init_escrow
  step_deposit
  step_verify_deposit
  step_mark_checked_in
  step_refund
  step_verify_refund

  print_summary
}

main "$@"
