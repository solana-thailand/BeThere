# Issue #035: Ticket State-Driven Hero + Claim Page Redirect

## Summary

The ticket page for in-person attendees has several UX problems:
- QR code dominates visual space even after check-in (when it's no longer needed)
- NFT claimed state is buried in a tiny green box
- Duplicate claim CTA — banner at top AND button in status section
- Backend ID (`api_id`) shown to attendees (irrelevant)

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

## Files to Change

| File | Change |
|------|--------|
| `frontend-leptos/src/pages/ticket.rs` | State-driven hero, collapsible QR, remove ID row, remove duplicate CTA |
| `frontend-leptos/src/pages/claim.rs` | AlreadyClaimed → redirect, Success → add back link |

## Acceptance Criteria

- [ ] Pre-check-in: QR code is the hero, no claim/NFT distractions
- [ ] Post-check-in, not claimed: Claim CTA is the hero, QR collapsed
- [ ] Post-check-in, claimed: NFT badge card is the hero with Orb link, QR collapsed
- [ ] Collapsible QR toggle works (expand/collapse after check-in)
- [ ] No duplicate claim CTAs
- [ ] No backend ID shown in info rows
- [ ] AlreadyClaimed on claim page redirects to ticket page
- [ ] Claim success shows "Back to Ticket" link
- [ ] `cargo check --target wasm32-unknown-unknown` passes

## Out of Scope

- Video section deduplication (~100 lines duplicated between online/in-person)
- Confetti/celebration sharing between claim and ticket pages
- Online attendee section (already well-structured)
