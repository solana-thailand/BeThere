# BeThere UX Roadmap — Prioritized Improvements

> **Source**: Full-platform UX audit session (2025-05-10)
> **Scope**: Landing, login, scanner, admin dashboard, deposit, claim, events management
> **Status**: All items are **open** unless marked with a commit hash

---

## Priority Tiers

| Tier | Label | Meaning |
|------|-------|---------|
| **P0** | Highest Impact | Blocks core conversion; must fix before public launch |
| **P1** | High Impact | Significantly improves user experience at events |
| **P2** | Medium Impact | Enhances professionalism and trust |
| **P3** | Nice-to-Have | Polish items for future iterations |

---

## P0 — Highest Impact

### P0-1. Public Event Page (`/e/{slug}`)

**Status**: ✅ Implemented (`worker/src/handlers/public_event.rs` + `frontend-leptos/src/pages/public_event.rs`)

> **Strategic Decision**: BeThere hosts its own public event page rather than relying solely on Lu.ma/Eventbrite.
> Neither platform supports on-chain deposits or attendance NFTs — they're listing tools, not conversion tools.
> The `/e/{slug}` page bridges the gap between "event listing" and "deposit flow."
> Future: CSV/API import from Lu.ma/Eventbrite for organizer convenience (growth plan item 4.8).
> Reference: Solana Thailand Genesis page at `solana-thailand.github.io/genesis` — similar community-driven approach.

The single highest-leverage UX improvement. Right now, attendees receive a deposit link with zero context — no event name, date, location, or countdown. A public event page at `/e/{slug}` would serve as:

- The canonical shareable link for organizers
- A landing page showing event details + countdown + deposit CTA
- Social media preview (Open Graph meta tags)

**Requirements**:
- Route: `GET /e/{slug}` → public page (no auth required)
- Display: event name, date/time, location, description, organizer info
- Countdown timer to event start
- CTA: "Secure Your Spot" → links to deposit flow
- Open Graph tags for link previews (Discord, Telegram, Twitter)
- If deposit disabled: simple "RSVP" or info-only page
- If event is past: show "Event ended" with claim CTA for NFT holders

**Files to create/modify**:
- `frontend-leptos/src/pages/public_event.rs` (new)
- `worker/src/handlers/mod.rs` (new public route)
- `worker/src/handlers/public_event.rs` (new — public API endpoint)

**Acceptance criteria**:
- [ ] `/e/{slug}` renders event details without login
- [ ] Countdown shows correct time to event start
- [ ] Deposit CTA deep-links to deposit page with event context
- [ ] OG tags render correctly when shared in Discord/Telegram
- [ ] 404 page for invalid/unknown slugs

---

### P0-2. Event Context Header on Deposit Page

**Status**: ✅ Implemented (`frontend-leptos/src/pages/deposit.rs` + `worker/src/handlers/deposit.rs`)

The deposit page currently jumps straight to payment options without showing what event the user is paying for. This is confusing and reduces trust.

**Requirements**:
- Add an event context header above the payment section:
  - Event name (large)
  - Event date + time
  - Location (if set)
  - "Your deposit of X USDC secures your spot at [Event Name]"
- This context is already available — the deposit page loads event data via API
- Quick implementation: add a section at the top of the deposit component

**Files to modify**:
- `frontend-leptos/src/pages/deposit.rs`

**Acceptance criteria**:
- [ ] Deposit page shows event name and date prominently
- [ ] Context is visible above the fold on mobile
- [ ] No regression to existing deposit flow

---

## P0.5 — Attendee Flow Improvements

### AF-1. Auto-redirect after registration

**Status**: ✅ Implemented

After clicking "Reserve Spot" on `/e/{slug}`, the attendee is auto-redirected to the deposit page instead of being shown a manual "Complete Deposit →" button.

---

### AF-2. Resume where left off

**Status**: ✅ Implemented

If an attendee closes the browser mid-flow, returning to `/e/{slug}` reads localStorage (`{attendee_id, event_id, event_slug}`) and redirects them to the correct page (deposit or ticket/QR).

---

### AF-3. Post-slip-upload redirect to ticket/QR page

**Status**: ✅ Implemented

After uploading a THB PromptPay slip, instead of showing a "Go Home" button, the attendee is auto-redirected to `/ticket/{attendee_id}?event_id={id}` which displays their QR code + pending approval status.

---

### AF-4. Dev-mode payment gating (hide Solana wallet in production)

**Status**: ✅ Implemented

USDC payment card on the deposit page is hidden unless the backend returns `dev_mode: true`. Health endpoint and public event endpoint now include `dev_mode`. Non-crypto attendees see only the THB option in production.

---

## P0.6 — Attendee Identity Verification (Google Sign-In)

### AF-5. Require Google Sign-In for registration and ticket access

**Status**: ✅ Implemented (commit `63bbf26` — `.issues/016_attendee_google_auth.md`, `.handovers/058_attendee_google_auth.md`)

**Security vulnerability**: Anyone who knows an email can register as that person, access their deposit page, get their QR code, and check in. The duplicate-email fix (commit 2499a78) made this worse by returning the existing attendee's `claim_token`.

**Fix**: Reuse the existing Google OAuth pipeline (staff login) for attendees. Dual-purpose: JWT = identity proof, roles checked per-page.

**Requirements**:
- `/e/:slug` hides registration form when not signed in, shows "Sign in with Google to register"
- After sign-in: email locked to Google account (read-only), registration form visible
- `POST /api/public/register` requires JWT, email taken from token (not body)
- New endpoint: `GET /api/my-registration/:slug` (JWT-required, lookup by email in event sheet)
- Auth callback redirects non-staff back to event page instead of rejecting
- Staff can register as attendee with same email (roles are per-page, not mutually exclusive)

**Files to create/modify**:
- `frontend-leptos/src/pages/public_event.rs` (sign-in gate, auth-aware form)
- `worker/src/handlers/auth.rs` (callback: redirect non-staff to event page)
- `worker/src/handlers/register.rs` (require JWT, use token email)
- `worker/src/auth.rs` (add `/api/public/register` to auth-required routes)
- `frontend-leptos/src/api.rs` (new `my_registration` API call)

---

## P0.5 — Attendee Flow Improvements

### AF-1 through AF-4

(See above — all ✅ Implemented)

---

## P0.7 — Post-Auth Navigation & Logout (Issue 017)

### AF-6. Deposit page "Back to event" navigation

**Status**: ✅ Implemented (commit `3c89fd3`) — `event_slug` added to `DepositStatusResponse`, deposit page links to `/e/{slug}` with `/` fallback.

### AF-7. Logout button for attendees

**Status**: ✅ Implemented (commit `1577828`) — `POST /api/auth/logout` clears JWT cookie. Logout button on deposit page (auth-gated), public event page (signed-in bar), and landing page (MyRegistrations section).

### AF-8. "My Registrations" on landing page

**Status**: ✅ Implemented (commits `3c89fd3`, `1577828`, `c9b6f1d`, `64ec532`) — `GET /api/my-registrations` endpoint + `MyRegistrations` component with color-coded status labels. Auth redirect fixed to respect event page URL for all users.

---

## P1 — High Impact

### P1-1. Scanner Haptic/Audio Feedback

**Status**: ✅ Implemented (`frontend-leptos/js/feedback.js` + `frontend-leptos/src/pages/scanner.rs`)

Staff scanning QR codes at real events need **instant** confirmation without looking at the screen. Currently there's only a visual overlay — no vibration or sound.

**Requirements**:
- On **successful** scan: `navigator.vibrate(100)` + short success beep (Web Audio API)
- On **failed** scan (already checked in, invalid QR): `navigator.vibrate([50, 50, 50])` + error tone
- On **walk-in registered**: distinct vibration pattern + chime
- Audio should be opt-in (first scan prompts "Enable sound?" or respects system settings)
- Must work on iOS Safari (limited vibrate support) and Android Chrome

**Files to modify**:
- `frontend-leptos/src/pages/scanner.rs`

**Acceptance criteria**:
- [ ] Success scan produces short vibration on Android
- [ ] Failure scan produces distinct vibration pattern
- [ ] Audio feedback works when enabled
- [ ] Graceful degradation on iOS (no crash, visual-only fallback)

---

### P1-2. Claim Flow Progress Indicator

**Status**: ✅ Implemented (`frontend-leptos/src/pages/claim.rs` — `ClaimStepper` component)

The claim flow is multi-step (connect wallet → deposit → quiz → claim NFT) but has no visible progress indicator. Users don't know how many steps remain or where they are.

**Requirements**:
- Step indicator at top of claim page: `[1. Connect] → [2. Deposit] → [3. Quiz] → [4. Claim NFT]`
- Current step highlighted, completed steps with checkmarks
- Steps can be conditional (skip quiz if event has no quiz)
- Mobile-friendly: horizontal stepper or minimal breadcrumb

**Files to modify**:
- `frontend-leptos/src/pages/claim.rs`

**Acceptance criteria**:
- [ ] Progress indicator shows at all steps of the claim flow
- [ ] Quiz step is hidden when event has no quiz configured
- [ ] Completed steps are visually distinct from current/future steps

---

### P1-3. Share CTA on NFT Mint Success

**Status**: ✅ Already implemented (share section in `frontend-leptos/src/pages/claim.rs` — Success state)

After successfully minting an NFT badge, users see a confirmation — but no way to share it. This is a free marketing opportunity.

**Requirements**:
- After NFT mint success, show "Share Your Badge" button
- Share options: copy link, Twitter/X post ("I just earned my [Event] badge! 🎫✨"), Telegram
- Badge image preview in the share card
- Link back to the public event page (P0-1) when available

**Files to modify**:
- `frontend-leptos/src/pages/claim.rs`

**Acceptance criteria**:
- [ ] "Share" button visible after NFT mint success
- [ ] Copy link copies the claim/badge URL
- [ ] Twitter share opens pre-filled tweet

---

## P2 — Medium Impact

### P2-1. Real-Time Admin Dashboard

**Status**: ❌ Not started

Admin dashboard stats (deposit count, check-in count, etc.) are loaded once on page load. Organizers must manually refresh during events.

**Approach**: Auto-poll every 30s during active events, or use Server-Sent Events (SSE) for live updates.

**Files to modify**:
- `worker/src/handlers/admin.rs` (add SSE endpoint or set cache headers)
- `frontend-leptos/src/pages/admin.rs` (add polling timer)

---

### P2-2. Batch/Manual Check-In for Staff

**Status**: ❌ Not started

Staff currently must scan 50 individual QR codes for group check-ins. A manual/batch mode would allow:

- Search attendee by name/email → manual check-in button
- Batch upload: paste a list of attendee IDs → check in all
- Useful for pre-event registration desks

**Files to create/modify**:
- `frontend-leptos/src/pages/scanner.rs` (add "Manual" tab)
- `worker/src/handlers/scanner.rs` (batch check-in endpoint)

---

### P2-3. Wallet Error Recovery Messages

**Status**: ❌ Not started

Wallet errors are generic and provide no recovery guidance. Users see "Transaction failed" without knowing why.

**Requirements**:
- Map common error codes to human-readable messages:
  - Wrong network → "Switch to Devnet/Mainnet in your wallet settings"
  - Insufficient funds → "You need at least X SOL for transaction fees + X USDC for deposit"
  - Transaction rejected → "You cancelled the transaction. Try again when ready."
  - Program error → "Something went wrong on-chain. Try again or contact support."
- Include a "What do I do?" action button for each error type

**Files to modify**:
- `frontend-leptos/src/pages/deposit.rs`
- `frontend-leptos/src/pages/claim.rs`

---

### P2-4. Live Event Showcase on Landing Page

**Status**: ✅ Implemented (`frontend-leptos/src/pages/landing.rs` + `worker/src/handlers/public_event.rs`)

The landing page has no evidence that the platform works. Showing real/upcoming events builds trust.

**Requirements**:
- Fetch active events from API (public endpoint)
- Show 3-6 upcoming events as cards on landing page
- Each card: event name, date, "View Event" → links to public event page (P0-1)
- If no events: show placeholder "Your event could be here" CTA for organizers

**Files to modify**:
- `frontend-leptos/src/pages/landing.rs`
- `worker/src/handlers/public_event.rs` (reuse from P0-1)

---

### P2-5. Walk-in Phase 4 — Post-Event Sync

**Status**: ❌ Not started (Phases 1-3 complete)

Walk-in attendees are stored in KV but not synced back to the Google Sheet after the event. This means:

- Post-event analytics in the Sheet are incomplete
- CSV exports miss walk-in data

**Requirements**:
- After event ends, batch sync walk-in records → Google Sheet (append rows)
- Admin button: "Sync walk-ins to Sheet"
- CSV export endpoint that includes both pre-registered and walk-in attendees

**Files to modify**:
- `worker/src/handlers/walkin.rs` (sync function)
- `worker/src/handlers/admin.rs` (export endpoint)

**Issue ref**: `.issues/014_walkin_attendee_flow.md`

---

## P3 — Nice-to-Have

### P3-1. Social Proof — Attendee Deposit Count

Show "X people have secured their spot" on the deposit page. Leverages social proof to increase conversions. Counter is public (no names exposed).

---

### P3-2. PWA Install Prompt

Mobile scanner users (staff) often use the browser. A PWA install prompt would let them add it to their home screen for faster access and full-screen mode.

---

### P3-3. Light Mode Toggle

Outdoor events with bright sunlight make the dark theme hard to read. A light/dark mode toggle would improve outdoor usability.

---

### P3-4. Thai i18n

Translate deposit and claim pages to Thai for local adoption in Thailand. Could start with key strings only (deposit amount, confirm, refund).

---

### P3-5. Event Cancellation Instruction (`cancel_event`)

The escrow program supports `cancel_event` (refund all + close), but there's no UI for it. Admins must refund attendees one by one. A "Cancel Event" button would batch-refund all depositors.

**Issue ref**: `docs/escrow_protocol.md` Q6

---

### P3-6. Load Testing (100+ Concurrent Deposits)

Production readiness requires validating that the worker + RPC can handle 100+ simultaneous deposit transactions. Use Artillery or k6 to simulate.

---

### P3-7. External Security Audit

Submit the on-chain escrow program to a Solana audit firm (e.g., Audit Arena, OtterSec) for external review before mainnet deployment with real funds.

---

## Relationship to Other Docs

| Document | Relationship |
|----------|-------------|
| `docs/business_flows_event_page.md` §12 | Known Gaps table — UX items are cross-referenced here |
| `docs/events_management.md` | Event data model used by P0-1 public event page |
| `docs/escrow_protocol.md` | Escrow instructions referenced in P3-5 |
| `docs/devnet_e2e_walkthrough.md` | E2E testing guide — new features need test flows added |
| `.issues/014_walkin_attendee_flow.md` | Walk-in Phase 4 (P2-5) |

---

*Document created from UX audit session. Last updated: 2026-05-10.*
