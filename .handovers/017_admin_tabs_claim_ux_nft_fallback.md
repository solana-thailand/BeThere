# Handover 017: Admin Dashboard Tabs, Claim Page UX, NFT Fallback, UI Refresh

> **Date**: 2026-04-25
> **Branch**: `develop`
> **Status**: Admin tabs + claim UX + NFT fallback done. Ready for non-NFT deployment.
> **Depends on**: Handover 016

## What Happened

Session focused on three areas: admin dashboard improvements, claim page UX overhaul, and NFT fallback mechanism so the system can ship without NFT secrets.

1. **Removed dead code** — unused `is_admin()` method on `StaffMember`
2. **Admin dashboard tabs** — In-Person / Online separation, tab-aware stats + list
3. **NFT fallback** — `nft_available` flag so claim page degrades gracefully without NFT secrets
4. **Claim page redesign** — event info, schedule timeline, wallet guidance, no emojis
5. **UI lively refresh** — gradients, animations, timeline layout, success glow effects

---

## AS-IS (Before This Session)

### Admin Dashboard
- Single flat list showing all attendees (in-person + online mixed)
- Check-in progress bar counted ALL attendees (not just in-person)
- Scanner button in admin body (redundant with header nav)
- No tab separation between participation types

### Claim Page
- Only showed wallet input or mint states — no event information
- No way to distinguish "NFT not configured" from "ready to mint"
- Fake wallet validation ("Address looks good!") based on length only
- Emojis everywhere (logo, states, footer)
- No event details, schedule, or links to event page

### NFT Dependency
- `POST /api/claim/{token}` would fail silently if NFT secrets missing
- Frontend had no visibility into whether NFT minting was available
- Entire claim flow was blocked until NFT secrets were configured

### UI/UX
- Flat dark background (`#0f0f0f`)
- Plain text logo, basic card styling
- No visual hierarchy or brand identity
- Schedule displayed as plain text lines

---

## TO-BE (After This Session)

### Admin Dashboard
- **Tab switcher** — In-Person (default) and Online tabs using existing `.tabs` / `.tab` CSS
- **Tab-aware stats** — Total / Checked In / Remaining counts scoped to active tab
- **Tab-aware progress bar** — e.g. "In-Person Check-In Progress: 45.2% (28 / 62)"
- **Tab-aware attendee list** — filtered by participation type + search query
- **Tab-aware recent check-ins** — only shows check-ins matching active tab
- **Cross-tab summary** — shows "62 Online attendees — switch tab to view"
- **Scanner button removed** from admin body (nav header has the link)

### Claim Page
- **NFT Coming Soon state** — when `nft_available == false`:
  - Attendee welcome card with check-in time
  - "NFT Badge Coming Soon" banner with shimmer animation
  - Full event info card: date, time, venue, schedule timeline
  - Wallet link (simple text link to phantom.app)
  - Bookmark reminder
- **Event info card** with timeline layout:
  - Date: Sunday, 26 April 2026
  - Time: 9:30 AM - 1:00 PM (ICT)
  - Venue: ContributeDAO (CDAO), 3rd Floor CP Tower, Phaya Thai
  - Schedule as timeline with highlighted speaker sessions
  - "View Full Event Details" link to genesis page
- **No emojis** — professional tone throughout
- **No fake wallet validation** — removed "Address looks good" feedback

### NFT Fallback
- **`nft_available: bool`** added to:
  - `ClaimLookupResponse` (domain) — computed from config
  - `ClaimLookupData` (frontend) — deserialized with `#[serde(default = "default_true")]`
- **Backend check** — `nft_available = !helius_api_key.is_empty() && !nft_metadata_uri.is_empty() && !nft_image_url.is_empty()`
- **Backwards compatible** — if backend doesn't send the field, frontend defaults to `true`

### UI/UX Lively Refresh
- **Gradient background** — body uses subtle dark gradient (`#0f0f0f` → `#111118` → `#0d0d14`)
- **Ambient glow** — soft radial purple glow behind content
- **Brand logo** — gradient text effect (purple-to-blue) with "Proof of Attendance" tagline
- **Timeline schedule** — left-bordered timeline with accent dots; highlighted items for speakers
- **Shimmer animation** — translucent light sweep on "Coming Soon" card
- **Success state** — SVG checkmark icon, green gradient border glow, structured details
- **Button improvements** — gradient primary buttons with hover lift + glow shadow
- **Wallet input** — focus glow ring
- **Powered-by badge** — "Powered by Solana" with gradient dot
- **Mobile responsive** — all new elements scale properly

---

## Files Changed

| File | Changes |
|------|---------|
| `worker/src/sheets.rs` | Removed unused `is_admin()` method + impl block from `StaffMember` |
| `worker/src/handlers/claim.rs` | Added `nft_available` computation + field to `ClaimLookupResponse` |
| `domain/src/models/api.rs` | Added `nft_available: bool` to `ClaimLookupResponse` |
| `frontend-leptos/src/api.rs` | Added `nft_available: bool` to `ClaimLookupData`; added `default_true()` helper |
| `frontend-leptos/src/pages/admin.rs` | `DashboardTab` enum; tab-aware `filtered_attendees`, `render_stats`, `render_recent_check_ins`; removed scanner button |
| `frontend-leptos/src/pages/claim.rs` | Added `NftComingSoon` state; event info card with timeline; removed emojis; wallet link; UI classes |
| `frontend-leptos/style.css` | +574 lines: gradients, ambient glow, brand logo, timeline, shimmer, success glow, powered badge, responsive |

---

## Architecture Decisions

### Why `nft_available` in claim lookup response?
The claim page is the only consumer of NFT status. Rather than a separate `/api/config` endpoint, embedding the flag in the existing response keeps it simple — one request, one response, all the data the page needs.

### Why `#[serde(default = "default_true")]`?
Backwards compatibility. If the backend is an older version without `nft_available`, the frontend still works (assumes NFT is available). This lets deployments upgrade backend and frontend independently.

### Why remove fake wallet validation?
Length + base58 checks give false confidence. A valid-looking address could still be wrong (different person's wallet, decommissioned, etc.). Real validation needs wallet connection (Wallet Standard). Better to not pretend.

### Why tabs instead of filters?
In-person is the primary use case at physical events — it deserves to be the default view with all dashboard features focused on it. Online is secondary. Tabs make the separation clear without cluttering the UI.

---

## How to Test

### Admin Tabs
1. Run `cd worker && bash deploy.sh dev`
2. Log in as admin, navigate to `/admin`
3. Verify: In-Person tab is default, shows only in-person attendees
4. Click Online tab — should switch to online-only attendees
5. Stats cards should update per tab (Total, Checked In, Remaining)
6. Progress bar should show tab-specific percentage
7. Recent check-ins should filter by tab
8. Search should work within the active tab

### Claim Page — NFT Coming Soon
1. Ensure NFT secrets are NOT set in `.dev.vars` (no `HELIUS_API_KEY`, etc.)
2. Check in an attendee via scanner
3. Open the claim URL from the QR code
4. Should see: Welcome card, "NFT Badge Coming Soon" banner, event info with timeline, wallet link
5. No emojis anywhere
6. No wallet input field
7. Timeline should show schedule with highlighted speaker sessions

### Claim Page — NFT Ready
1. Set `HELIUS_API_KEY`, `NFT_METADATA_URI`, `NFT_IMAGE_URL` in `.dev.vars`
2. Restart worker dev server
3. Open a claim URL for an unclaimed attendee
4. Should see: Welcome card + wallet input + "Claim NFT Badge" button
5. Minting flow should work as before

---

## Remain Work

### Before Next Event
- [ ] Design NFT artwork and upload to Arweave/IPFS
- [ ] Create metadata JSON and set `NFT_METADATA_URI` / `NFT_IMAGE_URL` secrets
- [ ] Decide: mainnet or devnet for first event
- [ ] Create verified NFT collection (~$3) for better wallet display
- [ ] Set production secrets: `npx wrangler secret put HELIUS_API_KEY` etc.
- [ ] Mobile-responsive testing of claim page (attendees use phones)
- [ ] Deploy to production: `cd worker && ./deploy.sh`

### Future Improvements
- [ ] Real wallet connection (Wallet Standard / adapter) instead of paste address
- [ ] Live stream / recording links in event info card
- [ ] Slide deck links after event
- [ ] SOL airdrop for gas (Phase 3 — refund)
- [ ] USDC refund transfer (Phase 3 — refund)
- [ ] Fund treasury wallet with SOL + USDC
- [ ] Multi-event support (configurable event name, schedule, links)

### Technical Debt
- [ ] Consider extracting event info into a config-driven system (env vars or JSON)
- [ ] Schedule timeline could be rendered from structured data instead of hardcoded
- [ ] Admin tabs could persist preference in localStorage
- [ ] Claim page analytics (how many people open the link vs. actually claim)

---

## Issues Ref
- None opened. All changes are incremental improvements from user feedback.

## Reflection

### What went well
- `DashboardTab` enum with `matches()` method made tab filtering clean and reusable
- `nft_available` flag is minimal and backwards-compatible
- CSS improvements make the page feel polished without touching any Rust logic

### What was tricky
- Leptos view macro parser chokes on `<` / `>` inside prop values — needed helper variables (e.g., `has_skipped = skipped > 0`)
- Balancing "lively" UI with "professional" tone — user explicitly said no emojis but wanted it to feel alive, which meant relying on gradients, animations, and color instead

### Key learning
- Don't fake wallet validation. The user correctly called out that base58 length checks are misleading — real validation requires wallet connection.