# 023 — Landing Page Swimlane & Demo Removal

> **Date**: 2025-07-14
> **Branch**: `feat/role-based-demo`
> **Commits**: `a67ef94`, `23e1412`, `666c3a1`, `2e0d1f2`

## What Happened

Replaced the standalone `/demo` route with an interactive swimlane embedded in the landing page "How it Works" section. The demo page was a full role-based interactive walkthrough (1064 lines) — the swimlane achieves the same goal (showing all 3 user journeys) in a lighter, more accessible format.

## Changes

### Removed
- `frontend-leptos/src/pages/demo.rs` — 1064-line interactive demo (role picker + step-by-step journeys)
- `/demo` route from `lib.rs` and `mod.rs`
- All `/demo` CTAs across landing page (hero, swimlane, FAQ, footer)

### Added
- **Swimlane component** in `landing.rs` — 3-role tabbed walkthrough (Organizer/Staff/Attendee)
- **12 mockup cards** — visual storyboards for each role+step combination
- **`swimlane-fade-in` CSS animation** — 0.25s fade+slide on step change
- **Step `desc` field** now rendered as subtitle under each mockup card (was previously dead code)
- **Consistent data across all mockups**: 150 registered, 142 checked in, 8 no-shows

### Updated CTAs
| Location | Before | After |
|----------|--------|-------|
| Hero | "Try Free Demo" → `/demo` | "Join Waitlist" → `#waitlist` |
| Swimlane | "Try the full interactive demo →" → `/demo` | "Join the waitlist →" → `#waitlist` |
| FAQ | "Still curious? Try the demo →" → `/demo` | "Ready to try? Join the waitlist →" → `#waitlist` |
| Footer | "Try Demo" → `/demo` | "FAQ" → `#faq` |

## Architecture

The swimlane is defined in `landing.rs` using:

```
SwimlaneRole enum (Organizer, Staff, Attendee)
  → label(), emoji(), accent(), accent_bg(), accent_border(), steps()
SwimlaneStep struct { icon, title, desc }
swimlane_mockup(role, step) → IntoView  // 12 mockup card variants
```

State management: two signals (`active_role`, `active_step`) control which mockup is displayed. The mini-swimlane at the bottom lets users switch roles with a single click.

## Rationale

- Landing page should be self-contained — no need for a separate demo route
- Swimlane gives visitors the key insight (3 perspectives) in seconds vs minutes
- Removes a 1064-line file that was maintenance overhead
- All CTAs now funnel to waitlist or sign-in (clearer conversion paths)

## Struggles / Solved

- **WASM type mismatch**: `match` arms with different `view!` structures produce different concrete types. Fix: `.into_any()` on each arm to erase to `AnyView`.
- **Stale `dist/`**: Previous build artifacts caused "No such file or directory" in minification. Fix: `rm -rf dist/` before rebuilding.
- **Adventure CSS committed**: Previous commit `23e1412` accidentally bundled other teams' files. Resolved by including adventure CSS in `2e0d1f2`.

## Build Status

- `cargo check`: ✅ Clean (0 warnings in our code)
- `cargo test`: ✅ 34/34 passed
- WASM release build: ✅ (4.8M wasm, 57K js)

## Remaining Work

- [ ] Mobile responsiveness — step flow dots may overflow on small screens
- [ ] CSS transitions between role tab switches
- [ ] Auto-cycle animation (optional — auto-advance steps when section in view)
- [ ] Create `og:image` (1200×630px) for social sharing
- [ ] Add Cloudflare Web Analytics snippet to `index.html`
- [ ] Deploy to production

## How to Dev/Test

```bash
# Local dev
cd frontend-leptos && ~/.cargo/bin/trunk serve
# Scroll to "How it Works" section — click role tabs and step dots

# WASM build
cd frontend-leptos && bash build.sh

# Deploy
cd worker && bash deploy.sh
```

## Ref

- `.issues/005_growth_marketing_plan.md` — growth strategy context
- `.handovers/019_landing_page.md` — original landing page implementation
