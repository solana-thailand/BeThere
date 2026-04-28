# Handover 020: Horizontal Scroll Fix + Inline Style Refactoring + Clippy Sweep

## What Happened

Continued from Phase E QA work (handover 019 context). The claim page had horizontal scroll overflow on mobile devices, scanner.rs and claim.rs had significant inline style debt, and there were 7 pre-existing clippy warnings. This session fixed all three.

## Changes Made

### 1. Horizontal Scroll Overflow Fix (`style.css`)

**Root causes identified:**

| Element | Problem | Fix |
|---------|---------|-----|
| `body` | Missing `overflow-x: hidden` (only `html` had it) | Added `overflow-x: hidden` |
| `.center-page::before` | Fixed `width: 600px` extended beyond narrow viewports | Added `max-width: 100vw` |
| `.page-container::before` | Fixed `width: 700px` extended beyond narrow viewports | Added `max-width: 100vw` |
| `.center-page::before` | No responsive override at 480px or 359px | Added `width:400px;height:400px` at 480px, `width:300px;height:300px` at 359px |

The pseudo-elements were the primary culprits — `600px` and `700px` fixed widths on radial gradient background glows extended beyond viewport on phones.

### 2. Scanner Component Refactoring (`scanner.rs`)

Extracted two reusable components from duplicated view code:

| Component | Parameters | Replaces |
|-----------|------------|----------|
| `ClaimQrCard` | `qr_src: String, claim_url: String, label: &'static str` | Duplicated QR blocks in `Success` + `AlreadyCheckedIn` (~25 lines each) |
| `AttendeeInfoCard` | `name: String, email: String` | Name+email blocks in `Found`, `AlreadyCheckedIn`, `NotApproved`, `NotInPerson` |

Both require `#[component]` attribute in Leptos 0.8 when used as tags in `view!` macros.

Added 16 new scanner CSS classes:

| Class | Purpose |
|-------|---------|
| `.scanner-state-header` | Centered header with check icon |
| `.scanner-state-loading` | Loading spinner state |
| `.scanner-attendee-info` | Semi-transparent attendee detail card |
| `.scanner-attendee-name` | Bold white name text |
| `.scanner-attendee-email` | Secondary email text |
| `.scanner-attendee-badges` | Flex row for badge pills |
| `.scanner-attendee-detail` | Detail line spacing |
| `.scanner-actions` | Flex button row |
| `.scanner-qr-wrapper` | Flex centering for QR card |
| `.scanner-qr-card` | White background card for QR image |
| `.scanner-qr-label` | Small gray label text |
| `.scanner-qr-img` | 240×240 QR image (160px @480w, 140px @359w) |
| `.scanner-qr-copy-btn` | Full-width copy button |
| `.scanner-result-detail-line` | Detail paragraph spacing |

QR code increased from 200px to 240px with responsive breakpoints.

### 3. Claim.rs Inline Style Extraction

| Metric | Before | After |
|--------|--------|-------|
| Inline `style="..."` | 36 | 8 |

22 new CSS classes added:

| Class | Replaced Style |
|-------|---------------|
| `.claim-container` | `display:flex;flex-direction:column;align-items:center;` |
| `.claim-state-full` | `width:100%;` |
| `.claim-shimmer-row` | `display:flex;align-items:center;gap:1rem;margin-bottom:1rem;` |
| `.claim-shimmer-col` | `flex:1;display:flex;flex-direction:column;gap:0.5rem;` |
| `.claim-shimmer-nft` | `width:72px;height:72px;border-radius:12px;flex-shrink:0;` |
| `.claim-shimmer-input` | `width:100%;height:42px;border-radius:8px;margin-bottom:0.5rem;` |
| `.claim-wallet-label` | `font-size:0.9rem;font-weight:600;color:var(--text-primary);display:block;margin-bottom:0.5rem;` |
| `.claim-wallet-row` | `display:flex;gap:0.5rem;` |
| `.claim-wallet-hint` | `font-size:0.75rem;color:var(--text-muted);margin-top:0.5rem;` |
| `.claim-minting` | `width:100%;display:flex;flex-direction:column;align-items:center;gap:1rem;padding:1.5rem 0;` |
| `.claim-minting-spinner` | `position:relative;width:64px;height:64px;` |
| `.claim-minting-shimmer` | `width:64px;height:64px;border-radius:50%;position:absolute;top:0;left:0;` |
| `.claim-minting-title` | `color:var(--text-primary);font-weight:600;` |
| `.claim-minting-detail` | `font-size:0.9rem;color:var(--text-secondary);` |
| `.claim-minting-hint` | `font-size:0.8rem;color:var(--text-muted);` |
| `.claim-share-x` | `border-color:#1da1f2;color:#1da1f2;display:flex;align-items:center;justify-content:center;gap:0.5rem;` |
| `.claim-share-x svg` | `width:18px;height:18px;` |
| `.claim-already-detail` | `margin-top:0.5rem;font-size:0.85rem;color:var(--text-secondary);` |
| `.claim-avatar-svg` | `image-rendering:pixelated;` |
| `.claim-retry-btn` | `margin-top:1rem;` |

Remaining 8 inline styles are per-instance shimmer width values (e.g., `style="width:60%;"`).

Also added `flex:1;min-width:0;` to `.claim-wallet-input` for proper flex sizing.

### 4. Admin.rs Inline Style Extraction

| Metric | Before | After |
|--------|--------|-------|
| Inline `style="..."` | 29 | 3 |

22 new admin CSS classes added (prefixed `admin-`):

`.admin-stat-value-success`, `.admin-stat-value-warning`, `.admin-actions-row`, `.admin-force-regen-row`, `.admin-force-regen-hint`, `.admin-count-row`, `.admin-count-text`, `.admin-empty-state`, `.admin-empty-state-sm`, `.admin-ticket-row`, `.admin-time-ago`, `.admin-cross-tab-summary`, `.admin-tab-switch-link`, `.admin-progress-header`, `.admin-progress-title`, `.admin-progress-pct`, `.admin-qr-result-card`, `.admin-qr-result-header`, `.admin-qr-result-title`, `.admin-qr-stats-row`, `.admin-qr-count-success`, `.admin-qr-count-warning`, `.admin-qr-count-label`, `.admin-section-heading`, `.admin-recent-email`, `.admin-badge-inline`, `.admin-checkin-time`.

Remaining 3 inline styles are dynamic progress bar width values (e.g., `width: {percentage}%`).

### 5. Clippy Warning Fixes (7→0)

| File | Warning | Fix |
|------|---------|-----|
| `auth.rs:121` | `collapsible_if` — nested `if let` + `if !empty` | Collapsed into single `if let ... && !empty` |
| `claim.rs:207` | `manual_is_multiple_of` — `(hash/96) % 3 == 0` | Replaced with `.is_multiple_of(3)` |
| `claim.rs:373` | `collapsible_if` — nested wallet check | Collapsed into `if let Some(ref wallet) = ... && !wallet.is_empty()` |
| `claim.rs:436` | `clone_on_copy` — `set_wallet_input.clone()` | Removed `.clone()` (WriteSignal implements Copy) |
| `claim.rs:442` | `collapsible_if` — nested JsFuture/asString/empty checks | Collapsed into `if let Ok(val) = ... && let Some(text) = val.as_string()` |
| `claim.rs:718` | `useless_format` — `format!("static string")` | Replaced with `.to_string()` |
| `scanner.rs:545` | `collapsible_if` — nested URL parsing | Collapsed into `if trimmed.starts_with("http") && let Ok(url) = ...` |

## Files Changed

| File | Change | Lines |
|------|--------|-------|
| `frontend-leptos/style.css` | Overflow fixes + 60 new CSS classes (scanner, claim, admin) | +105 |
| `frontend-leptos/src/pages/scanner.rs` | Extracted `ClaimQrCard` + `AttendeeInfoCard`, replaced inline styles | -117 +58 |
| `frontend-leptos/src/pages/claim.rs` | Replaced 28 inline styles with CSS classes + clippy fixes | -30 +26 |
| `frontend-leptos/src/pages/admin.rs` | Replaced 26 inline styles with CSS classes | -30 +28 |
| `frontend-leptos/src/auth.rs` | Collapsed nested if | -8 +8 |

## Commits

```
be3a5c9 fix: horizontal scroll overflow on claim page + scanner component refactoring
bdc80ca refactor(claim): extract 28 inline styles to CSS classes
2521e00 refactor: extract admin inline styles to CSS classes + fix all 7 clippy warnings
```

## Build Status

- **Frontend WASM**: ✅ builds clean (0 errors, 0 warnings)
- **Clippy**: ✅ 0 warnings
- **Dev server**: ✅ running on `localhost:8787`

## How to Dev/Test

```bash
# Build frontend
cd frontend-leptos && bash build.sh

# Dev server (port 8787)
cd worker && bash deploy.sh dev

# Test horizontal scroll
# Open claim page on Chrome DevTools → toggle device toolbar → iPhone SE (375px)
# No horizontal scrollbar should appear

# Test scanner QR centering
# Navigate to /staff → scan a valid QR → QR should be centered in success state
```

## Remain Work / Next Steps

### Before Next Event (Blockers)
- [ ] Generate real NFT artwork → upload to Arweave/Irys
- [ ] Set NFT secrets: `NFT_METADATA_URI`, `NFT_IMAGE_URL` via `npx wrangler secret put`
- [ ] Decide: mainnet or devnet for first event
- [ ] Update `EVENT_START_MS` / `EVENT_END_MS` for actual event time

### Visual QA
- [ ] Test QR code centering in `Success` and `AlreadyCheckedIn` states on mobile
- [ ] Test claim page on multiple device widths (320px, 375px, 414px)
- [ ] Test admin dashboard after refactoring — verify progress bars still render

### Technical Debt
- [ ] Wallet address validation (base58 check, not just length 32-44)
- [ ] Remaining 11 inline styles across shimmer/progress elements (per-instance values, low priority)
- [ ] Investigate `#[component]` requirement — why `ParticipantAvatar` works without it but `ClaimQrCard` doesn't

### Phase F — Next Features
- [ ] Analytics dashboard — check-in velocity, peak times heatmap
- [ ] Real-time admin auto-refresh — SSE or polling
- [ ] Multi-event support — event selector, multiple sheets
- [ ] Real wallet connection (Wallet Standard instead of paste address)
- [ ] Confetti animation on claim success
- [ ] Mobile PWA — service worker + manifest

## Issues Ref

- Horizontal scroll root cause: `.center-page::before` fixed `width:600px` + `body` missing `overflow-x:hidden`
- Leptos 0.8: functions used as component tags in `view!` require `#[component]` attribute
- `leptos_router::components::A` doesn't support `style` directly — use `attr:style`

## Reflection

The horizontal scroll was caused by a combination of factors — pseudo-elements with fixed widths on a page that needed to work at 320px viewports. The inline style extraction was straightforward but tedious; the key insight is that Leptos `view!` macros with many inline styles become hard to maintain and can't be overridden by media queries. The clippy warnings were all modern Rust idioms (collapsible if-let chains with `&&`) that the 2024 edition supports well.