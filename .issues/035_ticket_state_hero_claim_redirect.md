# Issue #035: Ticket State-Driven Hero + Claim Page Redirect

## Summary

The ticket page for in-person attendees has several UX problems:
- QR code dominates visual space even after check-in (when it's no longer needed)
- NFT claimed state is buried in a tiny green box
- Duplicate claim CTA — banner at top AND button in status section
- Backend ID (`api_id`) shown to attendees (irrelevant)

### Additional Deposit UX Improvements (added 2026-05)
- **Escrow-aware deposit display** — When on-chain escrow is closed/cancelled, USDC deposit is no longer available. The public event page and ticket page now hide USDC and show only THB when escrow is closed.
- **Clearer payment method labels** — Deposit section on public event page shows THB (via PromptPay) and USDC (via Solana) as separate labeled lines instead of a single combined string.
- **USDC struck-through when escrow closed** — Shows as crossed-out with "(closed)" label so attendees understand what happened.
- **Backend: `deposit_amount_usdc` added to ticket API response** — The ticket endpoint now returns both USDC and THB amounts.
- **Backend: `escrow_status` added to public event API response** — Frontend can now detect escrow state.

The claim page also has redundancy:
- "Already Claimed" state duplicates what the ticket page already shows
- No easy way to get back to the ticket page after claiming

## Scope

### Ticket Page — State-Driven Hero Layout

Rewrite the in-person section with a **state-driven hero**:

| State | Hero | QR |
|-------|------|----|
| Not approved | Pending approval badge | Hidden |
| Deposit pending | Deposit status banner | Hidden |
| Approved, no check-in | **QR code (large)** | Expanded |
| Checked in, not claimed | **"Claim Your NFT" CTA** | Collapsed |
| Checked in, claimed | **NFT badge card** (asset ID + Orb link) | Collapsed |
| Deposit deadline expired | Appropriate deposit banner | Hidden |

Additional changes:
- Remove duplicate claim CTA banner at top
- Remove backend `api_id` from info rows
- Add collapsible QR toggle (`show_qr` signal) — collapsed by default after check-in

### Claim Page — Redirect + Back Link

1. **AlreadyClaimed state → redirect to ticket page** instead of rendering a separate view
   - Uses `deposit_api_id` + `deposit_event_id` signals to build `/ticket/{api_id}?event_id={eid}`
   - Shows spinner + "Redirecting..." fallback with manual link

2. **Success state → add "Back to Ticket" link** after confetti/celebration
   - Same URL pattern using deposit signals
   - Appears after deposit refund link (if applicable)

### Explorer Link

Previous session already updated:
- `solanafm.com/token/` → `orbmarkets.io/token/{id}/metadata?cluster=...`
- Renamed `solanafm_asset_url` → `orb_nft_url` in `utils.rs`

## Files Changed

| File | Change |
|------|--------|
| `frontend-leptos/src/pages/ticket/page.rs` | State-driven hero, collapsible QR, polling, delegates to OnlineView/InPersonView (420 lines) |
| `frontend-leptos/src/pages/ticket/view_data.rs` | Shared `TicketViewData` struct with `from_data()` builder (161 lines) |
| `frontend-leptos/src/pages/ticket/qr_section.rs` | QR section (collapsible + fullscreen overlay) extracted (179 lines) |
| `frontend-leptos/src/pages/ticket/online_view.rs` | Online attendee view extracted (260 lines) |
| `frontend-leptos/src/pages/ticket/in_person_view.rs` | In-person attendee view extracted (384 lines) |
| `frontend-leptos/src/pages/ticket/action_cards.rs` | Deposit card: escrow-aware, show USDC+THB separately |
| `frontend-leptos/src/pages/public_event.rs` | Deposit section: escrow-aware, labeled payment methods |
| `frontend-leptos/src/api/types.rs` | Add `deposit_amount_usdc` to `AttendeeData` |
| `worker/src/handlers/attendee.rs` | Add `deposit_amount_usdc` to ticket API response |
| `worker/src/handlers/public_event.rs` | Add `escrow_status` to public event API response |
| `frontend-leptos/src/pages/claim.rs` | AlreadyClaimed → redirect, Success → add back link |

## Acceptance Criteria

### Ticket Page
- [x] Pre-check-in: QR code is the hero, no claim/NFT distractions
- [x] Post-check-in, not claimed: Claim CTA is the hero, QR collapsed
- [x] Post-check-in, claimed: NFT badge card is the hero with Orb link, QR collapsed
- [x] Collapsible QR toggle works (expand/collapse after check-in)
- [x] No duplicate claim CTAs
- [x] No backend ID shown in info rows

### Deposit UX (Escrow-Aware)
- [x] Public event page shows THB (via PromptPay) and USDC (via Solana) as separate labeled lines
- [x] When escrow closed, USDC is hidden/struck-through, only THB shown
- [x] Ticket deposit card shows both payment methods when escrow is open
- [x] Ticket deposit card shows USDC unavailable message when escrow is closed
- [x] Registration form deposit label is escrow-aware
- [x] `cargo check --target wasm32-unknown-unknown` passes (WASM build verified green, zero errors/warnings)

### Claim Page
- [x] AlreadyClaimed on claim page redirects to ticket page
- [x] Claim success shows "Back to Ticket" link

## Out of Scope

- Video section deduplication (~100 lines duplicated between online/in-person)
- Confetti/celebration sharing between claim and ticket pages
- Online attendee section (already well-structured)
