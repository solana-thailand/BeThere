# Handover 019: Landing Page

## What Happened

After deploying the scanner fix and claim page UX improvements (handover 018), user wanted to create a public landing page that pitches BeThere — what it does, the problem it solves, and clear calls to action for organizers and attendees.

## Changes Made

### 1. Landing Page (`frontend-leptos/src/pages/landing.rs`)

New public marketing page at `/` with 6 sections:

| Section | Content |
|---------|---------|
| **Nav** | Sticky top bar with gradient "BeThere" logo + "Sign In" button → `/login` |
| **Hero** | "Check in. Mint. Prove you were there." + two CTAs: "Get Started" → `/login`, "How It Works" → `#how-it-works` |
| **Problem** | "Attendance tracking is broken" — 3 cards (paper wristbands, spreadsheets, no lasting proof) |
| **How It Works** | 3 numbered steps: Register Event → Scan & Check In → Claim NFT |
| **Organizers / Attendees** | Side-by-side pitch cards with CTAs |
| **Footer** | "BeThere × Solana Thailand" branding, GitHub link |

Design: dark minimal, uses existing CSS variables (`--bg-secondary`, `--accent`, etc.) and classes (`.card`, `.btn`, `.btn-primary`). Mobile-first responsive with `clamp()` typography and `auto-fit` grids.

### 2. Route Changes (`frontend-leptos/src/lib.rs`)

| Before | After |
|--------|-------|
| `/` → Login | `/` → Landing |
| — | `/login` → Login (new route) |

### 3. Auth Redirect Updates

All redirect-on-unauth paths updated from `/` to `/login`:

| File | Function | Change |
|------|----------|--------|
| `frontend-leptos/src/auth.rs` | `require_auth()` | `navigate("/")` → `navigate("/login")` |
| `frontend-leptos/src/auth.rs` | `logout()` | `set_href("/")` → `set_href("/login")` |
| `frontend-leptos/src/components.rs` | `ProtectedRoute` | `nav("/")` → `nav("/login")` |

Login page already redirects to `/staff` after auth — unchanged and correct.

### 4. Build Fix

`<A>` component from `leptos_router` doesn't support `style` attribute directly — changed to `attr:style` in landing page hero CTA.

## Files Changed

| File | Change |
|------|--------|
| `frontend-leptos/src/pages/landing.rs` | New — full landing page component (214 lines) |
| `frontend-leptos/src/pages/mod.rs` | Added `pub mod landing` |
| `frontend-leptos/src/lib.rs` | Route `/` → Landing, `/login` → Login, updated docs |
| `frontend-leptos/src/auth.rs` | Redirects `/` → `/login` |
| `frontend-leptos/src/components.rs` | ProtectedRoute redirect `/` → `/login` |

## Commits

```
7658af8 feat: landing page for BeThere — public marketing page at /
3933ef4 merge: develop → main (landing page)
```

## Deployed

**Production:** https://bethere.solana-thailand.workers.dev
**Version ID:** 1fb97e21-3d38-499a-ab39-5dbbb2ee4ffb

## How to Dev/Test

```bash
# Build frontend
cd frontend-leptos && ~/.cargo/bin/trunk build --release

# Dev server (port 8787)
cd worker && bash deploy.sh dev

# Production deploy
cd worker && bash deploy.sh

# Test routes
/          → Landing page (public)
/login     → Login page (Google OAuth)
/staff     → Scanner (protected)
/admin     → Admin dashboard (protected)
/claim/:token → Claim page (public)
```

## Remain Work / Next Steps

### Before Next Event
- [ ] Design real NFT artwork → upload to Arweave/IPFS
- [ ] Set NFT secrets: `NFT_METADATA_URI`, `NFT_IMAGE_URL` via `npx wrangler secret put`
- [ ] Decide: mainnet or devnet for first event
- [ ] Update `EVENT_START_MS` / `EVENT_END_MS` for actual event time

### Landing Page Improvements
- [ ] Add screenshots/mockups of scanner and claim pages
- [ ] Add FAQ section
- [ ] Add "Events we've powered" social proof section
- [ ] SEO meta tags (og:image, description)
- [ ] Analytics tracking (page views, CTA clicks)

### Hearts Feature Decision Needed
- [ ] User must choose: per-token counter, global event counter, or real-time shared?

### Technical Debt
- [ ] Wallet address validation (base58 check, not just length 32-44)
- [ ] Share button on claim success ("Share your NFT")
- [ ] Confetti animation on successful claim
- [ ] Claim count display ("42 of 50 attendees claimed")
- [ ] Real wallet connection (Wallet Standard instead of paste address)
- [ ] Claim page analytics (open rate vs claim rate)
- [ ] NFT image integration (replace SVG placeholder with `NFT_IMAGE_URL`)

## Issues Ref

- `<A>` component `style` attribute error → use `attr:style` instead

## Reflection

The landing page was straightforward — mostly static content with Leptos `view!` macros and inline styles. The main gotcha was that `leptos_router::components::A` doesn't support `style` directly (needs `attr:style`). Route restructuring was clean: moved login to `/login`, landing at `/`, updated all auth redirects. The SPA fallback on the worker side means no server changes were needed.