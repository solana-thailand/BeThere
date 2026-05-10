# 014 — Walk-in Attendee Flow (Hybrid KV-based)

## Summary
BeThere currently requires attendees to pre-register via Google Sheet before the event. Walk-in attendees (who show up without pre-registering) cannot be properly handled. This issue tracks the hybrid KV-based walk-in solution: staff registers walk-ins via the admin scanner UI → backend creates KV attendee record → same deposit/NFT/refund loop as pre-registered attendees.

## Status: IN PROGRESS

### Phase 1 — Backend Walk-in Registration API ✅ Done
- [x] Add `WalkinAttendee` struct in `domain/src/models/attendee.rs`
- [x] Add `POST /api/walkin/register` endpoint (staff-only, requires auth)
- [x] Validate input: name (required), email (required), phone (optional)
- [x] Create KV attendee record with `walkin:{event_id}:{email}` key (90-day TTL)
- [x] Generate claim token (UUID v7) + reverse mapping `claim_walkin:{token}`
- [x] Return claim token + claim URL to staff UI

### Phase 2 — Deposit/Refund/NFT Flow Compatibility ✅ Done
- [x] Walk-in claim lookup: `lookup_walkin_by_claim_token()` checks KV first
- [x] Walk-in claim execution: `execute_walkin_claim()` mints NFT + updates KV (no sheet)
- [x] Deposit flow: wallet-based, works independently of attendee records
- [x] Refund flow: wallet-based, works independently of attendee records

### Phase 3 — Scanner UI ✅ Done
- [x] `WalkinRegisterRequest` / `WalkinRegisterResponse` in frontend API
- [x] "Register Walk-in Attendee" button in scanner Idle state
- [x] Walk-in registration form: name, email, phone
- [x] Walk-in success: QR code of claim URL for attendee to scan
- [x] "Scan Another" button returns to Idle

### Phase 4 — Optional: Post-event Sync
- [ ] Sync walk-in data to Google Sheet post-event (batch)
- [ ] Export walk-in attendee list as CSV

## Background
BeThere is an event check-in platform on Solana with deposit-backed attendance (USDC escrow). The current flow is:

1. Organizer creates event
2. Attendees pre-register via Google Sheet
3. Backend syncs sheet → creates KV attendee records with claim tokens
4. Attendees deposit USDC → staff checks them in → they claim NFT → refund deposit

Walk-in attendees (people who show up at the door without pre-registering) break this flow because they have no KV record and no claim token.

## Approach Analysis

| Approach | Description | Pros | Cons |
|----------|-------------|------|------|
| **1. On-the-spot registration** | Staff enters name/email → creates attendee in Google Sheet → normal flow | Full deposit + NFT + refund tracking | Requires Google Sheet write access |
| **2. Deposit-first walk-in** | Walk-in scans event QR → deposits USDC → gets temporary ticket → staff verifies | Self-serve, no staff data entry | Walk-in needs Solana wallet; no THB option |
| **3. Staff-override check-in** | Staff enters identifier → creates minimal KV record → generates claim token | Fastest to implement | Data split between KV and sheet |
| **4. Hybrid (recommended)** | Staff-side "Register Walk-in" → KV attendee record → same deposit/NFT/refund loop | Avoids sheet complexity, keeps full feature set | KV-sheet data sync needed later |

**Recommended: Hybrid KV-based (Approach 4)**

## Flow

1. Staff taps **"Register Walk-in"** in the scanner UI
2. Modal form: enters walk-in's name + email (or phone)
3. Backend `POST /api/walkin/register` creates a KV attendee record with key `walkin:{event_id}:{email}`
4. Backend generates a claim token (same as normal attendees)
5. Walk-in can now: deposit USDC → get checked in → claim NFT → refund deposit
6. If walk-in has a Solana wallet, they can scan the event QR and deposit self-serve

## Implementation Plan

### Phase 1 — Backend Walk-in Registration API
- [ ] Add `POST /api/walkin/register` endpoint (staff-only, requires auth)
- [ ] Validate input: name (required), email (required), phone (optional)
- [ ] Create KV attendee record with `walkin:{event_id}:{email}` key
- [ ] Generate claim token for walk-in (reuse existing claim token logic)
- [ ] Return claim token + attendee record to staff UI
- [ ] Add `walkin:true` flag to KV attendee record
- [ ] Ensure walk-in appears in attendee list alongside pre-registered attendees

### Phase 2 — Deposit/Refund/NFT Flow Compatibility
- [ ] Verify deposit flow works for walk-in KV records (no sheet dependency)
- [ ] Verify check-in flow works for walk-in attendees (scanner reads claim token)
- [ ] Verify NFT claim flow works for walk-ins
- [ ] Verify refund flow works for walk-ins
- [ ] Walk-in count included in event stats

### Phase 3 — Scanner UI (Frontend)
- [ ] Add "Register Walk-in" button to scanner page (`frontend-leptos/src/pages/scanner.rs`)
- [ ] Walk-in registration form modal: name, email, optional phone
- [ ] Show claim token (QR code) after successful registration
- [ ] Walk-in attendees visually distinguished with `walkin` badge in attendee list
- [ ] Walk-in count shown separately in event dashboard

### Phase 4 — Optional: Post-event Sync
- [ ] Sync walk-in data to Google Sheet post-event (batch)
- [ ] Export walk-in attendee list as CSV

## Security Considerations

- **Staff-only endpoint** — Same auth middleware as check-in endpoint
- **Rate limiting** — Prevent abuse (e.g., max 5 walk-in registrations per minute per staff)
- **Walk-in count cap** — Configurable per event (e.g., max 20 walk-ins) to prevent escrow abuse
- **Input validation** — Sanitize name/email/phone, validate email format
- **Claim token security** — Same entropy and TTL as pre-registered claim tokens
- **No privilege escalation** — Walk-in attendees cannot modify their own records

## Refs
- `docs/escrow_protocol.md` — Protocol design with deposit/NFT/refund loop
- Issue 010 — Deposit/refund escrow architecture
- Issue 013 — Escrow rug pull prevention (security context)
- `frontend-leptos/src/pages/scanner.rs` — Scanner UI (walk-in button target)
- `worker/src/api/` — Existing API endpoints (pattern reference)

## How to Test

```bash
# Phase 1: Backend
cd worker && cargo test --lib
# Test walkin register endpoint manually:
# curl -X POST /api/walkin/register \
#   -H "Authorization: Bearer <staff_token>" \
#   -d '{"event_id":"...", "name":"Test Walk-in", "email":"walkin@test.com"}'

# Phase 2: Verify existing flows
# Deposit → check-in → NFT claim → refund for walk-in attendee
# Compare with pre-registered attendee flow (should be identical)

# Phase 3: Frontend
cd frontend-leptos && cargo check --target wasm32-unknown-unknown
bash build.sh
# Manual: open scanner UI → tap "Register Walk-in" → fill form → verify QR appears

# E2E: full walk-in lifecycle on devnet
```
