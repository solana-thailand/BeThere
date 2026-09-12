#!/usr/bin/env bash
# Issue 070 step 6 — runtime Worker-log PII probe.
#
# `worker/tests/log_pii_guard.rs` proves no *source* site logs a raw identifier.
# This script proves the same thing about the *running* log stream, which is
# where the classes a source scan cannot see show up: values interpolated into
# a message, identifiers a handler reads back out of D1, error `Display` output,
# and the request path itself.
#
# It drives a local worker with disposable sentinel identifiers, so every hit in
# the captured log is a leak by construction. Nothing real is involved: the
# fixture emails end in `.invalid`, the wallet/signature sentinels are not valid
# keys, and the Google credentials in the probe `.dev.vars` below are unusable.
#
# Recipe (see also the "Local D1 verification harness" notes):
#
#   cd worker
#   cp .dev.vars .dev.vars.real.bak            # keep the real credentials safe
#   $EDITOR .dev.vars                          # fake every secret; DEV_MODE=1,
#                                              # GOOGLE_SERVICE_ACCOUNT_TOKEN_URI
#                                              # -> http://127.0.0.1:9/token so
#                                              # no Sheets call can succeed
#   mv ~/.pnp.cjs ~/.pnp.cjs.bak               # wrangler esbuild vs Yarn PnP
#   cargo build -p event-checkin-worker --target wasm32-unknown-unknown --release
#   wasm-bindgen --target bundler --no-typescript --remove-name-section \
#     ../target/wasm32-unknown-unknown/release/event_checkin_worker.wasm \
#     --out-dir build/worker
#   npx wrangler d1 migrations apply bethere-db --local
#   npx wrangler dev --local --port 8788 > /tmp/pii-probe-dev.log 2>&1 &
#   ../scripts/verify/pii_log_probe.sh                     # then grep, below
#   ../scripts/verify/pii_log_probe.sh --grep /tmp/pii-probe-dev.log
#
# Restart `wrangler dev` after every rebuild — its watcher is not reliable.
#
# What this probe cannot reach: the unusable Google credentials that make it
# safe also mean every Sheets helper fails at the token fetch, so nothing past
# that point runs — including its error paths. That blind spot hid a real leak
# (`"contact not found: {email}"`, logged by four deposit-credit call sites as
# `error = %e`). Those paths are covered by the source guard instead —
# `error_messages_do_not_embed_identifiers` in `worker/tests/log_pii_guard.rs`.
# Widen that guard, not this probe, when adding a fallible helper.
#
# Known residual, not fixable here: the *platform* request log (the
# `[wrangler:info] GET /api/claim/<token>` lines locally, Cloudflare's request
# metadata in production) records the raw URL, so a capability token in a path
# is visible there regardless of what the Worker logs.
set -u

# ---- grep mode -------------------------------------------------------------
# `--grep <logfile>` reports sentinel hits in a captured log. Worker tracing
# lines are the verdict; `[wrangler:info]` access-log lines are the platform
# residual described in the header and are reported separately.
if [ "${1:-}" = "--grep" ]; then
  LOG="${2:?usage: pii_log_probe.sh --grep <logfile>}"
  SENTINELS='0feed070|ZpiiWa11et|ZpiiSig|zpii\.|pii\.probe\.staff|66900000001|770001234|9990001234567|Zpiiattendeename|zpii_handle|zpii-gh-login|Zpiitg'
  echo "=== leaks in Worker tracing output (must be empty) ==="
  grep -nE "$SENTINELS" "$LOG" | grep -vE '^[0-9]+:\[wrangler:' || echo "(none)"
  echo "=== platform access-log residual (expected, not a Worker log) ==="
  grep -cE '^\[wrangler:info\]' "$LOG"
  exit 0
fi


BASE="${BASE:-http://localhost:8788}"
AUTH='Authorization: Bearer dev-token'
JSON='Content-Type: application/json'
EVENT_ID="pii-probe-event"

# ---- sentinels -------------------------------------------------------------
# PII sentinels (must NEVER appear in the log stream)
ATT_EMAIL="zpii.attendee.one@example.invalid"
ATT2_EMAIL="zpii.attendee.two@example.invalid"
WALKIN_EMAIL="zpii.attendee.walkin@example.invalid"
WAITLIST_EMAIL="zpii.waitlist.one@example.invalid"
ATT_NAME="Zpiiattendeename Sentinel"
PHONE="+66900000001"
WALLET_A="ZpiiWa11etAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
WALLET_B="ZpiiWa11etBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
SIG="ZpiiSig333333333333333333333333333333333333333333333333333333333333333333333333333333333"
CLAIM_TOKEN="0feed070-dead-7eef-8eef-0000000c1a1b"
TG_ID="770001234"
GH_LOGIN="zpii-gh-login"
HANDLE="@zpii_handle"
BANK_ACCT="9990001234567"
ATT_ID="0a77e11d-0000-7000-8000-000000000001"
ATT2_ID="0a77e11d-0000-7000-8000-000000000002"
ESCROW="ZpiiEscrwAddr444444444444444444444444444444"

req() { # method path [body]
  local m="$1" p="$2" b="${3:-}"
  if [ -n "$b" ]; then
    curl -s -o /dev/null -w "%{http_code} $m $p\n" -m 25 -X "$m" "$BASE$p" -H "$AUTH" -H "$JSON" -d "$b"
  else
    curl -s -o /dev/null -w "%{http_code} $m $p\n" -m 25 -X "$m" "$BASE$p" -H "$AUTH"
  fi
}
pub() { # method path [body]  (no auth header)
  local m="$1" p="$2" b="${3:-}"
  if [ -n "$b" ]; then
    curl -s -o /dev/null -w "%{http_code} $m $p\n" -m 25 -X "$m" "$BASE$p" -H "$JSON" -d "$b"
  else
    curl -s -o /dev/null -w "%{http_code} $m $p\n" -m 25 -X "$m" "$BASE$p"
  fi
}

echo "=== creating the probe event ==="
curl -s -o /dev/null -w "%{http_code} POST /api/events\n" -m 25 -X POST "$BASE/api/events" \
  -H "$AUTH" -H "$JSON" \
  -d "{\"name\":\"PII Probe Event\",\"slug\":\"$EVENT_ID\",\"tagline\":\"disposable\",\"link\":\"https://example.invalid/p\",\"event_start_ms\":1777100000000,\"event_end_ms\":1777200000000,\"sheet_id\":\"piiProbeSheetIdDoesNotExist\",\"sheet_name\":\"Attendees\",\"staff_sheet_name\":\"staff\"}"

echo "=== seeding disposable fixture into local D1 ==="
npx wrangler d1 execute bethere-db --local --command "
DELETE FROM attendees WHERE event_id='$EVENT_ID';
DELETE FROM contacts WHERE email LIKE 'zpii.%';
DELETE FROM thb_deposits WHERE event_id='$EVENT_ID';
DELETE FROM credit_ledger WHERE email LIKE 'zpii.%';
DELETE FROM claim_locks WHERE event_id='$EVENT_ID';
DELETE FROM escrow_index WHERE event_id='$EVENT_ID';
INSERT INTO attendees (id,event_id,email,name,claim_token,contact_channel,contact_handle,deposit_status,deposit_amount_thb,deposit_tx_hash,bank_name,bank_account_number,bank_account_name,qr_url)
 VALUES ('$ATT_ID','$EVENT_ID','$ATT_EMAIL','$ATT_NAME','$CLAIM_TOKEN','Telegram','$HANDLE','verified',1000,'$SIG','ZpiiBank','$BANK_ACCT','$ATT_NAME','https://example.invalid/qr');
INSERT INTO attendees (id,event_id,email,name,claim_token,checked_in_at,checked_in_by,deposit_status,deposit_amount_thb)
 VALUES ('$ATT2_ID','$EVENT_ID','$ATT2_EMAIL','Zpiisecond Sentinel','0feed070-dead-7eef-8eef-0000000c1a2c','2026-09-12T10:00:00Z','pii.probe.staff.s1@example.invalid','refund_pending',1000);
INSERT INTO contacts (email,name,contact_channel,contact_handle,deposit_credit_thb,deposit_credit_since)
 VALUES ('$ATT_EMAIL','$ATT_NAME','Telegram','$HANDLE',1000,'2026-09-01T00:00:00Z');
INSERT INTO thb_deposits (attendee_id,event_id,amount_thb,uploaded_at,attendee_name,bank_account,bank_name,account_name,verified)
 VALUES ('$ATT_ID','$EVENT_ID',1000,'2026-09-01T00:00:00Z','$ATT_NAME','$BANK_ACCT','ZpiiBank','$ATT_NAME',1);
INSERT INTO credit_ledger (email,currency,delta,reason,event_id) VALUES ('$ATT_EMAIL','thb',1000,'hold_deposit','$EVENT_ID');
INSERT INTO claim_locks (event_id,token,lock_id,wallet,expires_at) VALUES ('$EVENT_ID','$CLAIM_TOKEN','lock-1','$WALLET_A','2030-01-01T00:00:00Z');
INSERT INTO escrow_index (escrow_address,event_id) VALUES ('$ESCROW','$EVENT_ID');
" >/dev/null 2>&1 && echo "seeded" || echo "SEED FAILED"

echo "=== public endpoints ==="
pub POST /api/waitlist "{\"email\":\"$WAITLIST_EMAIL\"}"
pub POST /api/auth/wallet/nonce "{\"wallet_address\":\"$WALLET_A\"}"
pub POST /api/auth/wallet/verify "{\"wallet_address\":\"$WALLET_A\",\"signature\":\"$SIG\",\"message\":\"probe $WALLET_A\",\"nonce\":\"zpiinonce0001\"}"
pub GET "/api/claim/$CLAIM_TOKEN"
pub POST "/api/claim/$CLAIM_TOKEN" "{\"wallet_address\":\"$WALLET_B\"}"
pub GET "/api/wallet/$WALLET_A/nfts"
pub GET "/api/public/ticket/$ATT_ID"
pub GET "/api/deposit/status/$ATT_ID?event_id=$EVENT_ID"
pub GET "/api/quiz/$CLAIM_TOKEN/status"
pub POST "/api/quiz/$CLAIM_TOKEN/submit" '{"answers":[]}'
pub GET "/api/adventure/$CLAIM_TOKEN/status"
pub GET "/api/auth/callback?code=zpiioauthcode0001&state=zpiistate0001"
pub GET "/api/public/event/pii-probe-event"
pub GET "/api/public/event-series/$EVENT_ID"

echo "=== webhooks (WEBHOOK_SECRET bearer) ==="
curl -s -o /dev/null -w "%{http_code} POST /api/deposit/usdc/webhook\n" -m 25 -X POST "$BASE/api/deposit/usdc/webhook" \
  -H "Authorization: Bearer pii-probe-webhook-secret" -H "$JSON" \
  -d "{\"event_id\":\"$EVENT_ID\",\"attendee_id\":\"$ATT_ID\",\"tx_signature\":\"$SIG\"}"
curl -s -o /dev/null -w "%{http_code} POST /api/escrow/onchain-webhook\n" -m 25 -X POST "$BASE/api/escrow/onchain-webhook" \
  -H "Authorization: Bearer pii-probe-webhook-secret" -H "$JSON" \
  -d "[{\"signature\":\"$SIG\",\"slot\":1,\"timestamp\":1777100000,\"feePayer\":\"$WALLET_A\",\"description\":\"probe transfer from $WALLET_A\",\"type\":\"TRANSFER\",\"accountData\":[],\"instructions\":[{\"programId\":\"$ESCROW\",\"accounts\":[\"$WALLET_A\"],\"data\":\"zpiidata\"}]}]"

echo "=== authed attendee endpoints ==="
req GET /api/auth/me
req POST /api/auth/wallet/bind "{\"wallet_address\":\"$WALLET_B\",\"signature\":\"$SIG\",\"message\":\"bind $WALLET_B\"}"
req POST /api/public/register "{\"slug\":\"pii-probe-event\",\"name\":\"$ATT_NAME\",\"email\":\"$ATT_EMAIL\",\"participation_type\":\"in_person\",\"contact_channel\":\"Telegram\",\"contact_handle\":\"$HANDLE\",\"consent_given\":true}"
req GET /api/my-registrations
req GET "/api/my-registration/pii-probe-event"
req GET /api/my-profile
req POST /api/privacy/delete-request "{}"
req POST /api/privacy/unsubscribe-marketing "{}"
req POST /api/auth/telegram/verify "{\"id\":$TG_ID,\"first_name\":\"Zpiitgfirst\",\"last_name\":\"Zpiitglast\",\"username\":\"$GH_LOGIN\",\"auth_date\":1777100000,\"hash\":\"zpiihash0001\"}"
req GET /api/auth/telegram/state
req GET "/api/auth/github?redirect=/profile"
req POST /api/auth/social/unlink '{"platform":"github"}'
req POST /api/checkin/nfc/verify "{\"event_id\":\"$EVENT_ID\",\"tag_id\":\"zpiinfctag0001\"}"
req GET /api/my-notifications
req POST "/api/deposit/hold" "{\"event_id\":\"$EVENT_ID\",\"attendee_id\":\"$ATT_ID\"}"
req GET "/api/deposit/credit-balance"
req POST "/api/deposit/request-credit-refund" "{\"bank_name\":\"ZpiiBank\",\"bank_account_number\":\"$BANK_ACCT\",\"bank_account_name\":\"$ATT_NAME\"}"
req GET "/api/deposit/credit-refund-request"

echo "=== authed staff/admin endpoints ==="
req GET "/api/attendees?event_id=$EVENT_ID"
req POST "/api/checkin/$ATT_ID?event_id=$EVENT_ID"
req POST "/api/attendee/$ATT_ID/undo-checkin?event_id=$EVENT_ID"
req POST /api/walkin/register "{\"event_id\":\"$EVENT_ID\",\"name\":\"$ATT_NAME\",\"email\":\"$WALKIN_EMAIL\",\"phone\":\"$PHONE\"}"
req GET "/api/walkin/list?event_id=$EVENT_ID"
req GET "/api/walkin/export?event_id=$EVENT_ID"
req POST /api/walkin/sync "{\"event_id\":\"$EVENT_ID\"}"
req GET "/api/refund/queue?event_id=$EVENT_ID"
req GET "/api/refund/held?event_id=$EVENT_ID"
req GET "/api/refund/refunded?event_id=$EVENT_ID"
req GET "/api/deposit/credit-used?event_id=$EVENT_ID"
req GET "/api/dashboard/live?event_id=$EVENT_ID"
req GET "/api/events/$EVENT_ID/audit"
req GET /api/audit/global
req GET "/api/events/$EVENT_ID/summary"
req GET "/api/events/$EVENT_ID/notifications"
req GET /api/events/readiness
req POST "/api/generate-qrs" "{\"event_id\":\"$EVENT_ID\"}"
req GET "/api/events/$EVENT_ID"
req PUT "/api/events/$EVENT_ID" "{\"organizer_emails\":[\"$ATT2_EMAIL\"]}"
echo "=== done ==="
