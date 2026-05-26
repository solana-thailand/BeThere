# Issue #036: Public Event Page Refactor — Modularize + UX Polish

## Summary

The `e/{slug}` public event page (`public_event.rs`) has grown to ~1800 lines in a single file with all types, helpers, data loading, rendering, and form logic combined. This refactor modularizes it following the ticket page pattern and addresses several UX gaps.

### Problems
1. **Monolith file** — ~1800 lines, violates 1024-line rule
2. **All inline styles** — No CSS classes for page-specific layout, making maintenance hard
3. **No OG meta tags** — Social link previews show nothing (high impact for sharing)
4. **Hardcoded share URL** — `bethere.solana-thailand.workers.dev` instead of `window.location.origin`
5. **No error retry** — Failed event fetch shows only "Go Home", no retry button
6. **Countdown disappears at 0** — Could show "Happening now!" state

## Scope

### Phase 1: Modularization (extract into directory)
Follow the `pages/ticket/` pattern — split into:

| File | Responsibility | Lines (est.) |
|------|---------------|-------------|
| `public_event/mod.rs` | Module index, re-exports | ~15 |
| `public_event/page.rs` | Top-level component, data loading, auth flow, event assembly | ~690 |
| `public_event/types.rs` | Structs, enums, API types | ~170 |
| `public_event/event_hero.rs` | NFT image + event name + tagline | ~80 |
| `public_event/details_card.rs` | Format badge, location, date/time, countdown | ~150 |
| `public_event/deposit_section.rs` | Deposit commitment card (USDC/THB/escrow) | ~120 |
| `public_event/capacity_indicator.rs` | Remaining spots display | ~80 |
| `public_event/registration_form.rs` | Registration form + validation + submit | ~360 |
| `public_event/registered_state.rs` | Already-registered view with redirect | ~80 |
| `public_event/share_button.rs` | Share button + copy feedback | ~50 |

### Phase 2: CSS Classes
Extract repeated inline styles into CSS classes in `style.css`:
- `.pe-card` — standard card wrapper (bg-card, border-radius, padding, shadow)
- `.pe-section-title` — h2 section title
- `.pe-detail-row` — icon + text flex row
- `.pe-badge` — format badge pill
- `.pe-method-row` — deposit method line (label + amount + via)

### Phase 3: UX Fixes
- [x] Add `<Meta>` tags for og:title, og:description, og:image, og:url
- [x] Fix hardcoded share URL → use `window.location.origin`
- [x] Add retry button on error states
- [x] Show "Happening now!" when countdown reaches 0 (event not completed)

## Files Changed

| File | Change |
|------|--------|
| `frontend-leptos/src/pages/public_event.rs` | **Delete** (replaced by directory) |
| `frontend-leptos/src/pages/public_event/mod.rs` | New — module index |
| `frontend-leptos/src/pages/public_event/page.rs` | New — orchestrator + data loading |
| `frontend-leptos/src/pages/public_event/types.rs` | New — all types extracted |
| `frontend-leptos/src/pages/public_event/event_hero.rs` | New — hero section |
| `frontend-leptos/src/pages/public_event/details_card.rs` | New — event details |
| `frontend-leptos/src/pages/public_event/deposit_section.rs` | New — deposit info |
| `frontend-leptos/src/pages/public_event/capacity_indicator.rs` | New — capacity |
| `frontend-leptos/src/pages/public_event/registration_form.rs` | New — reg form |
| `frontend-leptos/src/pages/public_event/registered_state.rs` | New — already registered |
| `frontend-leptos/src/pages/public_event/share_button.rs` | New — share button |
| `frontend-leptos/style.css` | Add `.pe-*` CSS classes |
| `frontend-leptos/src/pages/mod.rs` | Update module declaration |

## Acceptance Criteria

### Modularization
- [x] `public_event.rs` replaced by `public_event/` directory
- [x] Each file under 200 lines (except `page.rs` at ~690 and `registration_form.rs` at ~360)
- [x] `mod.rs` contains only module declarations and re-exports
- [x] All existing functionality preserved (no regressions)

### CSS Classes
- [x] Repeated card styles use `.pe-card` class
- [x] Repeated section titles use `.pe-section-title` class
- [x] Inline styles reduced by ~60%+ from original

### UX
- [x] OG meta tags render with event name, description, NFT image
- [x] Share URL uses `window.location.origin` (no hardcoded domain)
- [x] Error state has "Try Again" button
- [x] Countdown shows "Happening now!" when event is live (started but not completed)

### Build
- [x] `cargo check --target wasm32-unknown-unknown` passes (zero errors, zero warnings)

## Out of Scope
- Registration form redesign (fields, layout changes)
- Adding new registration fields
- Backend API changes
- PWA/offline support
