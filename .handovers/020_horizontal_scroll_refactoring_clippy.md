# Handover 020: Horizontal Scroll Fix + Refactoring Sweep + Landing Repositioning

## What Happened

Continued from Phase E QA work (handover 019). The session covered four distinct areas:

1. **Horizontal scroll overflow** on the claim page — fixed
2. **Inline style debt** across scanner.rs, claim.rs, admin.rs — extracted to CSS classes
3. **7 clippy warnings** — all resolved
4. **Landing page repositioning** — from "check in + mint NFT" to deposit-commitment hook, then de-jargonified

Also cleaned up 10 stale stitch HTML mockup files.

## Changes Made

### 1. Horizontal Scroll Overflow Fix (`style.css`)

**Root causes:**

| Element | Problem | Fix |
|---------|---------|-----|
| `body` | Missing `overflow-x: hidden` (only `html` had it) | Added `overflow-x: hidden` |
| `.center-page::before` | Fixed `width: 600px` extended beyond narrow viewports | Added `max-width: 100vw` |
| `.page-container::before` | Fixed `width: 700px` extended beyond narrow viewports | Added `max-width: 100vw` |
| `.center-page::before` | No responsive override at 480px or 359px | Added overrides at both breakpoints |

### 2. Scanner Component Refactoring (`scanner.rs`)

Extracted two reusable components from duplicated view code:

| Component | Parameters | Replaces |
|-----------|------------|----------|
| `ClaimQrCard` | `qr_src: String, claim_url: String, label: &'static str` | Duplicated QR blocks in `Success` + `AlreadyCheckedIn` (~25 lines each) |
| `AttendeeInfoCard` | `name: String, email: String` | Name+email blocks in `Found`, `AlreadyCheckedIn`, `NotApproved`, `NotInPerson` |

Both require `#[component]` attribute in Leptos 0.8 when used as tags in `view!` macros.

Added 16 new scanner CSS classes (`.scanner-state-header`, `.scanner-attendee-info`, `.scanner-qr-wrapper`, `.scanner-qr-card`, `.scanner-qr-img`, etc.).

QR code increased from 200px to 240px with responsive breakpoints.

### 3. Claim.rs Inline Style Extraction

| Metric | Before | After |
|--------|--------|-------|
| Inline `style="..."` | 36 | 8 |

22 new CSS classes added (`.claim-container`, `.claim-state-full`, `.claim-shimmer-row`, `.claim-minting`, `.claim-wallet-label`, `.claim-share-x`, etc.).

Remaining 8 inline styles are per-instance shimmer width values (e.g., `style="width:60%;"`).

### 4. Admin.rs Inline Style Extraction

| Metric | Before | After |
|--------|--------|-------|
| Inline `style="..."` | 29 | 3 |

22 new admin CSS classes added (`.admin-actions-row`, `.admin-count-row`, `.admin-empty-state`, `.admin-progress-header`, `.admin-qr-result-card`, etc.).

Remaining 3 inline styles are dynamic progress bar width values (e.g., `width: {percentage}%`).

### 5. Clippy Warning Fixes (7 → 0)

| File | Warning | Fix |
|------|---------|-----|
| `auth.rs:121` | `collapsible_if` | Collapsed into single `if let ... &&` |
| `claim.rs:207` | `manual_is_multiple_of` | `.is_multiple_of(3)` |
| `claim.rs:373` | `collapsible_if` | Collapsed into `if let Some(ref wallet) = ... && !wallet.is_empty()` |
| `claim.rs:436` | `clone_on_copy` | Removed `.clone()` on `WriteSignal` |
| `claim.rs:442` | `collapsible_if` | Collapsed nested `JsFuture`/`asString`/`empty` checks |
| `claim.rs:718` | `useless_format` | Replaced `format!("static")` with `.to_string()` |
| `scanner.rs:545` | `collapsible_if` | Collapsed URL parsing chain |

### 6. Landing Page Repositioning

**Two-phase rewrite:**

Phase A — Deposit-commitment hook:

| Section | Before | After |
|---|---|---|
| **Hero** | "Check in. Mint. Prove you were there." | "Commit. Show up. Get your money back." |
| **Problem** | "Attendance tracking is broken" | "Events have a no-show problem" — 40% stat |
| **Step 1** | "Register Event" | "Put Down a Deposit" |
| **Step 2** | "Scan & Check In" | "Show Up & Scan" |
| **Step 3** | "Claim NFT" | "Get Your Money Back + A Badge" |
| **Organizers** | Check-in tool pitch | Deposit requirement, no-shows lose theirs |
| **Attendees** | Free NFT pitch | Deposit → show up → refunded + keep badge |
| **Footer** | "On-chain proof of attendance" | "Show up. Get refunded." |

Phase B — Jargon removal:

| Was (jargon) | Now (plain English) |
|---|---|
| "Get your SOL back" | "Get your money back" |
| "Lock a SOL deposit" | "Put down a small deposit" |
| "on-chain escrow" | "safely held until the event is over" |
| "verified on-chain" | "you're checked in" |
| "compressed NFT — permanent, on-chain proof" | "digital badge — yours forever, proof you were there" |
| "on-chain attendance portfolio" | "collect badges from every event" |
| "Claim NFT" (footer link) | "Claim Badge" |

### 7. Cleanup

Removed 10 stale stitch HTML design mockups from `.design/stitch/` (admin-attendees, admin-overview, admin-qr, claim-loading, claim-ready, claim-success, landing-desktop, landing-mobile, scanner-active, scanner-success).

## Files Changed

| File | Change |
|------|--------|
| `frontend-leptos/style.css` | Overflow fixes + 60 new CSS classes |
| `frontend-leptos/src/pages/scanner.rs` | Extracted components, replaced inline styles, collapsed ifs |
| `frontend-leptos/src/pages/claim.rs` | 28 inline styles → classes, clippy fixes |
| `frontend-leptos/src/pages/admin.rs` | 26 inline styles → classes |
| `frontend-leptos/src/pages/landing.rs` | Full copy rewrite — deposit hook + de-jargonified |
| `frontend-leptos/src/auth.rs` | Collapsed nested if |
| `.design/stitch/*` | Deleted (10 files) |

## Commits

```
be3a5c9 fix: horizontal scroll overflow on claim page + scanner component refactoring
bdc80ca refactor(claim): extract 28 inline styles to CSS classes
2521e00 refactor: extract admin inline styles to CSS classes + fix all 7 clippy warnings
4b462dd docs: handover 020 — horizontal scroll fix, inline style refactoring, clippy sweep
cf10d00 chore: remove stitch HTML design mockups
c846536 feat(landing): reposition around deposit-commitment hook — show up, get refunded
b15164b fix(landing): remove crypto jargon — plain English for everyone
```

## Deployed

**Production:** https://bethere.solana-thailand.workers.dev
**Version ID:** ba570c7f-eb1c-48c5-870f-983ebca1773e

## Build Status

- **Frontend WASM**: builds clean (0 errors, 0 warnings)
- **Clippy**: 0 warnings
- **Dev server**: running on `localhost:8787`

## How to Dev/Test

```bash
# Build frontend
cd frontend-leptos && bash build.sh

# Dev server (port 8787)
cd worker && bash deploy.sh dev

# Production deploy
cd worker && bash deploy.sh

# Test horizontal scroll
# Chrome DevTools → toggle device toolbar → iPhone SE (375px)
# No horizontal scrollbar should appear

# Test landing page
# Visit / — hero should say "Commit. Show up. Get your money back."
```

## Remain Work / Next Steps

### Before Next Event (Blockers)
- [ ] Generate real NFT artwork → upload to Arweave/Irys
- [ ] Set NFT secrets: `NFT_METADATA_URI`, `NFT_IMAGE_URL` via `npx wrangler secret put`
- [ ] Decide: mainnet or devnet for first event
- [ ] Update `EVENT_START_MS` / `EVENT_END_MS` for actual event time

### Deposit/Refund Smart Contract
- [ ] See `.issues/001_deposit_commitment_refund.md` — full plan with 4 phases (~10-15 days)
- [ ] Phase 1: Escrow program (Anchor/Rust)
- [ ] Phase 2: Backend integration (worker API)
- [ ] Phase 3: Frontend (Wallet Standard, deposit flow, combined refund+mint)
- [ ] Phase 4: Production (audit, mainnet deploy)

### Landing Page Polish
- [ ] Add screenshots/mockups of scanner and claim pages
- [ ] Add FAQ section
- [ ] SEO meta tags (og:image, description)
- [ ] "Claim Badge" footer link should point to claim flow, not login

### Visual QA
- [ ] Test claim page on multiple device widths (320px, 375px, 414px)
- [ ] Test admin dashboard after refactoring — verify progress bars still render
- [ ] Test QR centering in scanner success state on mobile

### Technical Debt
- [ ] Wallet address validation (base58 check, not just length 32-44)
- [ ] Remaining 11 inline styles across shimmer/progress elements (per-instance values, low priority)
- [ ] Investigate `#[component]` requirement — why `ParticipantAvatar` works without it but `ClaimQrCard` doesn't
- [ ] Landing page inline styles — should be extracted to CSS classes (currently ~30 inline styles in landing.rs)

## Issues Ref

- `.issues/001_deposit_commitment_refund.md` — deposit escrow smart contract plan

## Reflection

The horizontal scroll was caused by pseudo-elements with fixed widths on a page that needed to work at 320px viewports. The inline style extraction was tedious but straightforward — Leptos `view!` macros with many inline styles can't be overridden by media queries and become hard to maintain. The landing page rewrite went through two passes: first repositioning around the deposit hook (which is a much stronger value prop than "mint an NFT"), then removing crypto jargon so a non-technical person understands it immediately. The key insight: "SOL", "on-chain", "NFT", "escrow" are all inside-baseball terms that alienate the exact audience we want to reach.