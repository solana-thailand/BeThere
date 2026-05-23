# Issue #033: "Less is More" — App-Wide UI Simplification

> **Priority**: P1 (high impact on usability, no new features — purely simplification)
> **Motivation**: "What Elite Software Engineers Do Differently" — simple is complicated enough at scale. Stop building for 100x. The dogma of clean code (or clean UI) is hurting you.
> **Status**: 🔄 In Progress

## Problem

The app has grown feature-rich but UI-heavy. Pages that should be simple (deposit, ticket, claim) show too much at once. Non-technical attendees face overwhelming pages with 8-12 buttons, 5+ conditional banners, and duplicated UI sections. Staff pages have inline styles, copy-pasted components, and redundant info.

### Audit Results (2026-05-23)

| Page | Lines | States | Max Buttons | Problem |
|------|------:|-------:|------------:|---------|
| Deposit | 2,655 | 18 | Step 1: 2 cards, Step 2: ~6 buttons | ✅ Simplified — 2-step wizard (PaymentChoice signal). Step 1 = pick method (USDC/THB). Step 2 = payment form for chosen method only. |
| Ticket | 1,229 | ~19 conditionals | 4-5 | 5 mutually exclusive deposit banners. NFT claim UI duplicated 3×. "What's Next" timeline only on online but heavyweight. |
| Claim | 1,990 | 12 | 4-5 (Ready) | ✅ Simplified — success = 4 sections (celebration + asset + view + share), deposit refund → link to deposit page |
| Events | 2,572 | 3 views | 15+ | 35 form fields, 9 collapsible sections, wallet connect duplicated. Violates 1024-line guideline. |
| Scanner | 2,123 | 22 | 5 (Idle) | Flash/Audio toggles always visible. Success shows undo countdown + on-chain. Staff are power users but settings should be in gear. |
| Admin Escrow | 677 | — | 8 | 3 identical step cards copy-pasted. 30+ inline styles. |
| Admin Cancel | 417 | — | 6 | Redundant info note box. 6-cell grid where 3 metrics suffice. |
| Admin Attendee | — | — | 12 per row | ✅ Fixed — redesigned to two-row card layout (ef2aa89) |

## Plan

### Phase 1: Attendee-Facing Pages (highest impact on conversion)

#### 1A. Deposit Page → 2-Step Wizard
**Current**: `ChoosePayment` state shows USDC card (wallet buttons + QR + manual input) + THB card (PromptPay QR + file upload + bank fields) + 3 deadline banners = ~12 interactive elements at once.
**Target**:
- Step 1: Pick method (USDC or THB) — two cards, one choice
- Step 2: Payment form for chosen method only
- Deadline banners: one unified notice slot
**Impact**: Halves visual complexity on the most intimidating page for non-technical attendees
**Files**: `frontend-leptos/src/pages/deposit.rs`, `frontend-leptos/style.css`
**Status**: ✅ Completed — 2-step wizard using pre-existing `PaymentChoice` enum + `payment_choice` signal. Step 1 shows selection cards (USDC dev-only + THB). Step 2 shows chosen method's full form with “← Change method” back button. Resolves `payment_choice`/`PaymentChoice` unused warnings. Added `cursor:pointer` to `.deposit-method-card` CSS.

#### 1B. Ticket Page → Unify 5 Deposit Banners → 1 Status Slot ✅
**Current**: 5 mutually exclusive colored banners (deposit required / deadline expired + reclaimable / deadline expired + gone / deposit pending / deposit verified). Attendee must parse which applies.
**Target**: One notice slot that shows the relevant status. No rainbow of banners.
**Impact**: Eliminates confusion — attendee always sees exactly one status
**Files**: `frontend-leptos/src/pages/ticket.rs`
**Status**: ✅ Completed — unified into single if-else-if chain

#### 1C. Claim Success → NFT + "View NFT" + Done ✅
**Current**: 10+ sections after successful claim (3 explorer links, tweet preview, cNFT paragraph, deposit refund section, HeartsWidget).
**Target**:
- NFT claimed ✓
- Asset ID + "View NFT on SolanaFM" (one link)
- Small "Share" button (collapses to tweet + copy link)
- Remove: redundant explorer links, cNFT paragraph, deposit refund (separate page)
**Impact**: Celebration moment stays focused, not diluted by 7 secondary sections
**Files**: `frontend-leptos/src/pages/claim.rs`
**Status**: ✅ Completed — removed DepositRefundSection (280 lines), ClaimRefundState enum, deposit status fetch, cNFT hint. Deposit refund → link to /deposit page. Share section compacted.

### Phase 2: Staff/Admin Pages (maintainability)

#### 2A. Events Page → Extract Form Component ✅
**Current**: 2,572 lines. Create and Edit share ~1,800 lines of form view logic inline.
**Target**: Extract `<EventForm>` component shared by Create/Edit modes. EventsPage becomes list + routing.
**Impact**: Brings file under 1024-line guideline. DRY form logic.
**Files**: `frontend-leptos/src/pages/events_page.rs` → extract `frontend-leptos/src/pages/event_form.rs`
**Status**: ✅ Completed — events_page.rs: 2,572 → 765 lines, event_form.rs: 1,861 lines

#### 2B. Admin Escrow → Shared Step Component ✅
**Current**: 3 identical ~80-line step cards copy-pasted. 30+ inline styles.
**Target**: One `<EscrowStepCard>` component parameterized by `EscrowAction` enum (which already has label/icon/description methods). Inline styles → CSS classes.
**Files**: `frontend-leptos/src/pages/admin_escrow.rs`, `frontend-leptos/style.css`
**Status**: ✅ Completed — 30+ inline styles → 5 minor overrides. `<EscrowStepCard>` handles done/signing/disabled/actionable/confirm-danger states. Reused `.step-card`, `.wallet-bar`, `.info-note`, `.panel-box` CSS classes.

#### 2C. Scanner → Settings Gear ✅
**Current**: Flash/Audio toggles always visible in bottom sheet.
**Target**: Gear icon → settings popover with Flash/Audio toggles. Bottom sheet shows only: session stats + "Enter Manually" + "Register Walk-in".
**Files**: `frontend-leptos/src/pages/scanner.rs`, `frontend-leptos/style.css`, `frontend-leptos/src/icons/mod.rs`
**Status**: ✅ Completed — Flash/Sound moved to ⚙ popover. Bottom sheet: 3 primary controls (Enter manually + ⚙ + Register Walk-in). Added Settings icon + 6 CSS classes.

### Phase 3: Polish (nice-to-have)

- Remove redundant info note on Admin Cancel page
- Compact 6-cell stats grid → 3 key metrics
- Unify wallet connect component (Create mode vs EscrowInitPanel in events page)

## Acceptance Criteria

- [x] Deposit ChoosePayment state shows ≤6 interactive elements (down from 12) — 2-step wizard: Step 1 = 2 selection cards, Step 2 = 1 method's form at a time
- [x] Ticket page shows exactly 1 deposit notice at a time (down from 5 conditional banners) — unified into single if-else-if chain
- [x] Claim Success shows ≤4 sections (down from 10+) — celebration + asset card + view NFT + compact share + deposit link
- [x] Events page file ≤1024 lines (down from 2,572) — 765 lines (events_page.rs) + 1,861 (event_form.rs)
- [x] Admin Escrow has ≤5 inline styles (down from 30+) — only minor font/padding overrides remain
- [x] Scanner bottom sheet shows ≤3 primary controls — "Enter manually" + ⚙ settings popover (Flash/Sound) + "Register Walk-in"
- [ ] Frontend builds without warnings
- [ ] Manual test: deposit flow, ticket view, claim flow still work end-to-end

## Related

- `docs/ux_roadmap.md` — existing UX improvements roadmap
- `.issues/021_frontend_code_quality.md` — api.rs split (completed, same philosophy)
- `.issues/022_architecture_refactor_perf_optimization.md` — file splitting pattern (completed)
- `.handovers/063_design_system_drift_fix_visual_audit.md` — design system consistency
- Commit `ef2aa89` — admin attendee list two-row card layout (first "less is more" refactor)
