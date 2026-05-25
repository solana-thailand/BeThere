# Handover #074: Required Bank Info + Refund Proof Upload + Refund Queue Slip View

## What Happened

Three related improvements to the THB deposit refund flow, addressing trust, data quality, and admin UX gaps identified during production use.

### Problem 1: No slip visible in refund queue
Admin had to refund attendees without seeing their payment slip — no way to cross-check amount before refunding.

### Problem 2: Missing bank info for refunds
Some attendees deposited but didn't fill in bank_account/bank_name/account_name. Admin had to individually chat with each person to collect refund info.

### Problem 3: Organizer could fake refunds (trust gap)
Nothing prevented an organizer from clicking "Mark Refunded" and keeping the deposit. No proof of transfer was required.

## Where Is the Plan/Code/Test

### Files Changed

| File | Change |
|------|--------|
| `domain/src/models/deposit.rs` | Added `refund_proof_url: Option<String>` to `ThbDeposit`; added `refund_proof_url: String` to `MarkRefundRequest` |
| `worker/src/handlers/deposit/thb.rs` | Backend validation: bank info required on upload, refund proof required on mark-refund |
| `frontend-leptos/src/api/deposit.rs` | Added `bank_account`, `bank_name`, `account_name`, `refund_proof_url` to `ThbDepositInfo`; added `refund_proof_url` to `MarkRefundRequest` |
| `frontend-leptos/src/pages/admin_deposit.rs` | Refund Queue: slip link, bank info display, 2-step refund proof flow. Refunded tab: slip link, bank info, refund proof link |
| `frontend-leptos/src/pages/deposit.rs` | "Upload Slip" button disabled until all 3 bank fields are filled |

### Docs Updated

| File | Update |
|------|--------|
| `.issues/010_deposit_refund_escrow.md` | Updated THB refund flow with proof requirement |
| `docs/escrow_protocol.md` | Added refund proof to trust model, updated off-chain refund flow |
| `.issues/032_rolling_deposit_credit.md` | Updated status — backend done, frontend pending |

## Changes Detail

### 1. View Slip in Refund Queue + Refunded Tabs

Both the **Refund Queue** and **Refunded** tabs now show:
- "View Slip" link to the original uploaded payment slip
- "Refund Bank Info" section with account number, bank name, holder name
- `⚠ No bank info` warning badge when info is missing

### 2. Required Bank Info (frontend + backend)

**Frontend (`deposit.rs`):** "Upload Slip" button is disabled until `bank_account`, `bank_name`, and `account_name` are all filled. Hint text explains why.

**Backend (`thb.rs`):** `upload_thb_slip_handler` validates all 3 fields are non-empty — returns 400 with field name if missing.

### 3. Refund Proof Upload (full stack)

**Two-step admin flow:**
1. Admin clicks "Mark Refunded" → input field appears for refund proof URL
2. Admin pastes bank transfer receipt URL → clicks "✓ Confirm Refund"

**Backend:** `mark_refund_handler` validates `refund_proof_url` is non-empty, saves to `ThbDeposit.refund_proof_url`.

**Refunded tab:** Shows "View Refund Proof" link so anyone can audit past refunds.

## Reflection / Struggling / Solved

### Solved
- **Leptos lifetime issue in Refunded tab**: The `Show when=` closures capture references from iterator items backed by a temporary `Signal::get()` value. Fixed by collecting into a `Vec<_>` of owned tuples first, then iterating over those.
- **Partial move in Refund Queue tab**: `item_for_refund` was used for both display (bank info) and the click handler. Fixed by cloning bank fields before the `item.clone()`.

### Struggled
- The `refund_proof_url` is currently a text input — no file upload widget. Admin must upload receipt image separately and paste the URL. A future improvement could add R2 upload like the slip flow.

## Remain Work

- [ ] Deploy worker + frontend together
- [ ] Attendee-facing "Hold Deposit as Credit" button in deposit page (backend `hold_deposit_handler` exists, frontend missing)
- [ ] Credit balance display on ticket/profile page
- [ ] R2 upload widget for refund proof (instead of manual URL paste)
- [ ] Attendee confirmation flow: refund isn't "complete" until attendee confirms receipt (future trust improvement)

## Refs

- Issue #010: `.issues/010_deposit_refund_escrow.md`
- Issue #032: `.issues/032_rolling_deposit_credit.md`
- Escrow protocol: `docs/escrow_protocol.md`
