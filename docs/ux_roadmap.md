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

### AF-9. Landing page auth-aware navigation

**Status**: ✅ Implemented — `frontend-leptos/src/pages/landing.rs` unified Google Sign-In flow. Nav bar adapts to auth state: shows "Sign In" (OAuth trigger) for guests, user email + role-based links (Dashboard/Scanner) + "Sign Out" for signed-in users. Hero CTA changed from "Sign In" to "Find Events ↓". Footer link relabeled "Staff Portal". No backend changes needed — auth callback already handles non-staff redirect to `/`.

---

## P0.9 — PDPA Consent & Data Collection (Issue 043)

> **Priority**: P1 — pre-mainnet legal compliance. Thailand PDPA effective June 1, 2022.
> Full plan: `.issues/043_pdpa_consent_data_collection.md`

### PDPA-1. Data Collection Consent Checkbox
**Current**: Registration form (`registration_form.rs`) has no consent checkbox. Personal data (name, email, phone, contact handle) is collected without explicit consent.
**Target**: Mandatory consent checkbox before submit. "I consent to BeThere collecting my name, email, and contact information for event registration, check-in, and NFT issuance."
**Impact**: PDPA Section 19 compliance. Cannot legally collect data without it.
**Effort**: ~3h (UI + backend validation + new sheet column AE)

### PDPA-2. Photo/Media Consent (Per-Event)
**Current**: No photo consent mechanism. Most Thai events take photos.
**Target**: Organizer enables "Collect photo consent" per event. Attendee sees opt-in checkbox: "I consent to being photographed/filmed during the event."
**Impact**: PDPA Section 20 (sensitive data). Photo consent is legally separate from data collection consent.
**Effort**: ~3.5h (event config + UI + new sheet column AF)

### PDPA-3. Privacy Policy Page (`/privacy`)
**Status**: ✅ Implemented — `/privacy` route with PDPA-compliant privacy policy (`frontend-leptos/src/pages/privacy.rs`)
**Current**: No privacy policy page exists.
**Target**: Public `/privacy` route with data practices disclosure — what we collect, why, where it's stored, blockchain immutability notice, data subject rights, contact info.
**Impact**: PDPA Section 23 (privacy notice before collection). Legal requirement.
**Effort**: ~2.75h (new page + route + footer link)

### PDPA-4. Data Retention & Deletion
**Current**: No deletion mechanism. Data lives forever in Google Sheets and on-chain.
**Target**: `POST /api/privacy/delete-request` clears PII from sheet row + KV. On-chain data disclosed as immutable in privacy policy.
**Impact**: PDPA Section 29 (right to erasure). Can ship post-mainnet.
**Effort**: ~4h (API + UI + policy update)

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

**Leptos libraries to evaluate**:
- `leptos_sse` — server signals synced through Server-Sent-Events, simplifies SSE integration in Leptos
- `leptos_server_signal` — alternative using websockets for server-pushed state
- `leptos-use` (hooks: `use_event_listener`, `use_document_visibility`) — pause/resume polling when tab is hidden

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

**Status**: ✅ Implemented (commit `bd9601e`) — `wallet_error.rs`, structured error JSON in `solana_wallet.js`, error classification + user-friendly messages across all wallet call sites.

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

**Status**: ✅ Implemented (commit `bd9601e`) — CSV export endpoint, Google Sheet sync with idempotency, admin UI buttons, walkin list handler.

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

> **Upgraded to P1** — now part of Solana Mobile support ([Issue 042](../.issues/042_solana_mobile_support.md) Phase B).
> PWA is a prerequisite for Solana dApp Store listing.

Mobile scanner users (staff) often use the browser. A PWA install prompt would let them add it to their home screen for faster access and full-screen mode.

**Leptos libraries to evaluate**:
- `leptos-use` — provides `use_window_size` and browser API hooks useful for PWA detection

**PWA requirements for Solana Mobile:**
- `manifest.json` with `display: standalone`, icons (192x192, 512x512)
- Service worker for shell caching
- `<meta name="apple-mobile-web-app-*">` tags for iOS
- HTTPS (already served via Cloudflare Workers)

---

### P3-3. Light Mode Toggle

Outdoor events with bright sunlight make the dark theme hard to read. A light/dark mode toggle would improve outdoor usability.

**Leptos libraries to evaluate**:
- `leptos_darkmode` — manages `dark` class for Tailwind CSS, persists preference in localStorage, respects system media query
- Note: requires designing a light theme variant of the CSS custom properties (`--bg-primary`, `--text-primary`, etc.)

---

### P3-4. Thai i18n

Translate deposit and claim pages to Thai for local adoption in Thailand. Could start with key strings only (deposit amount, confirm, refund).

**Leptos libraries to evaluate**:
- `leptos_i18n` — compile-time type-safe translations; missing keys fail at compile time
- `leptos-fluent` — alternative using fluent-templates (Mozilla Fluent format)

---

### P3-5. Event Cancellation Instruction (`cancel_event`)

**Status**: ✅ Implemented (handover 074 — THB batch refund + USDC refund queue + cancel status). Commit `bd9601e`. Note: USDC refunds still require attendee signature (on-chain constraint).

**Issue ref**: `docs/escrow_protocol.md` Q6

---

### P3-6. Load Testing (100+ Concurrent Deposits)

Production readiness requires validating that the worker + RPC can handle 100+ simultaneous deposit transactions. Use Artillery or k6 to simulate.

---

### P3-7. External Security Audit

Submit the on-chain escrow program to a Solana audit firm (e.g., Audit Arena, OtterSec) for external review before mainnet deployment with real funds.

---

## Leptos Ecosystem — Recommended Libraries

> Source: [awesome-leptos](https://github.com/leptos-rs/awesome-leptos) review (2026-06)
> Cross-referenced with current `frontend-leptos/Cargo.toml` dependencies.

### Phase 1 — Immediate Value (Minimal Effort)

| Library | Purpose | Replaces in BeThere |
|---------|---------|-------------------|
| `leptos-use` | Reactive hooks for browser APIs (clipboard, visibility, local storage, events, websocket) | Hand-rolled `web-sys` bindings in scanner, admin, deposit pages |
| `leptos-captcha` | Self-hosted proof-of-work captcha (no reCAPTCHA dependency) | No bot protection exists — needed for public reservation flow |

### Phase 2 — Going Public

| Library | Purpose | Related Roadmap Item |
|---------|---------|---------------------|
| `leptos_i18n` | Compile-time type-safe translations (EN + TH) | P3-4 |
| `leptos-captcha` | Bot protection on registration/deposit | N/A |
| `leptos-hotkeys` | Declarative keyboard shortcuts | Scanner efficiency (staff) |
| `leptos_sse` | Server-Sent Events integration | P2-1 |

### Phase 3 — Polish

| Library | Purpose | Related Roadmap Item |
|---------|---------|---------------------|
| `leptos_darkmode` | Dark/light mode toggle with Tailwind | P3-3 |
| `leptos-struct-table` | Auto-generated sortable tables from structs | Admin panel tables |
| `leptos-obfuscate` | Email obfuscation for bot protection | Admin panel emails |
| `leptos-toaster` | Animated toast notifications (Sonner-inspired) | Existing `Toast` component in `components.rs` |

### Not Recommended (skip)

| Library | Reason |
|---------|--------|
| Thaw, leptix, shadcn/ui ports | Custom design system — full rewrite required |
| `leptos-leaflet`, `leptos_maplibre` | No maps in BeThere |
| `leptos-tea` (Elm Architecture) | Already using Leptos signals — migration not worth it |
| `leptos-image` | No image optimization pipeline |

## Relationship to Other Docs

| Document | Relationship |
|----------|-------------|
| `docs/business_flows_event_page.md` §12 | Known Gaps table — UX items are cross-referenced here |
| `docs/events_management.md` | Event data model used by P0-1 public event page |
| `docs/escrow_protocol.md` | Escrow instructions referenced in P3-5 |
| `docs/devnet_e2e_walkthrough.md` | E2E testing guide — new features need test flows added |
| `.issues/014_walkin_attendee_flow.md` | Walk-in Phase 4 (P2-5) |
| `docs/research_technology_review.md` §14 | Leptos ecosystem library analysis and rationale |
| `.issues/042_solana_mobile_support.md` | Full Solana Mobile plan (MWA + PWA + dApp Store) — P2.6 section |
| `.issues/043_pdpa_consent_data_collection.md` | PDPA compliance plan (consent + photo + privacy policy + deletion) — P0.9 section |

---

## P0.8 — Registration Capacity & Track Gating (Issue 024)

### RC-1. Capacity indicator on public event page
**Current**: Registration form always shows both tracks for hybrid events.
**Target**: Show remaining in-person spots. Hide in-person option when full. Auto-select online.
**Impact**: Prevents frustration from registering for a full track. Clear expectations.
**Effort**: ~2h (frontend + backend capacity count)

### RC-2. Claim timing gate for online attendees
**Current**: Online attendees get claim URL immediately after registration.
**Target**: Online claim page shows "Claims open after event ends on [date]" with countdown.
**Impact**: Prevents attendees from completing everything before the event occurs.
**Effort**: ~1h (backend gate in claim endpoint)

### RC-3. Deposit deadline countdown on deposit page
**Current**: No urgency indicator for completing deposit.
**Target**: Show countdown timer: "Complete your deposit within 23h 45m to keep your in-person spot."
**Impact**: Reduces "seat hoarding" — encourages timely deposits.
**Effort**: ~2h (frontend countdown + backend deadline field)

### RC-4. Organizer capacity controls on event form
**Current**: No capacity fields on create/edit event form.
**Target**: Add in-person capacity, online capacity, online open mode selector.
**Impact**: Gives organizers control over spot allocation and NFT supply.
**Effort**: ~3h (form fields + backend validation)

### RC-5. Manual online toggle in staff UI
**Current**: No way to manually open/close online registration.
**Target**: Toggle button in admin panel for "Open Online Registration".
**Impact**: Organizer control over when online track becomes available.
**Effort**: ~1h (toggle button + API endpoint)

---

*Document created from UX audit session. Last updated: 2026-06-02.*

---

## P1.5 — "Less is More" UI Simplification (Issue 033)

> **Philosophy**: Simple is complicated enough at scale. Stop building for 100x. The best engineers use boring technology.
> Full audit and phased plan: `.issues/033_less_is_more_ui_simplification.md`

### LM-1. Deposit Page → 2-Step Wizard (Phase 1A) ✅
**Status**: ✅ Implemented — Deposit page uses 2-step wizard (`frontend-leptos/src/pages/deposit/`)
**Current**: 12 interactive elements at once in ChoosePayment state.
**Target**: Step 1 = pick method (USDC or THB). Step 2 = payment form for chosen method.
**Impact**: Halves visual complexity on the most intimidating page.

### LM-2. Ticket Page → 1 Deposit Status Slot (Phase 1B)
**Current**: 5 mutually exclusive colored banners.
**Target**: One notice slot showing the relevant status.
**Impact**: Eliminates rainbow-of-banners confusion.

### LM-3. Claim Success → NFT + View + Done (Phase 1C)
**Current**: 10+ sections (3 explorer links, tweet preview, cNFT paragraph, deposit refund).
**Target**: NFT claimed ✓ + asset ID + "View NFT" + optional share button.
**Impact**: Celebration moment stays focused.

### LM-4. Events Page → Extract Form Component (Phase 2A) ✅
**Status**: ✅ Implemented — `EventFormComponent` extracted, used by EventsPage (`frontend-leptos/src/pages/event_form.rs`)
**Current**: 2,572 lines, 35 fields, 9 sections.
**Target**: `<EventForm>` component, EventsPage ≤1024 lines.

### LM-5. Admin Escrow → Shared Step Component (Phase 2B) ✅
**Status**: ✅ Implemented — `EscrowStepCard` shared component (`frontend-leptos/src/pages/admin_escrow.rs`)
**Current**: 3 identical step cards copy-pasted, 30+ inline styles.
**Target**: One `<EscrowStep>` component, CSS classes only.

### LM-6. Scanner → Settings Gear (Phase 2C) ✅
**Status**: ✅ Implemented — Settings gear icon with popover for flash/audio toggles (`frontend-leptos/src/pages/scanner.rs`)
**Current**: Flash/Audio toggles always visible.
**Target**: Gear icon popover. Bottom sheet = stats + manual + walk-in.

### ✅ LM-0. Admin Attendee List Redesign
**Done**: Two-row card layout. Name is hero element. Commit `ef2aa89`.

---

## P2.5 — Learning & Credentials UX (Issue 038)

> **Prerequisite**: Phases 10–12 shipped (mainnet, platform fees, multi-org). Curriculum features are future work.
> Full vision: `.issues/038_curriculum_design_vision.md`

### LC-1. Learning Pathway Progress
**Current**: Adventure shows completed levels in a flat list. No sense of progression toward a credential.
**Target**: Visual progress bar or pathway map showing levels completed → credits earned → certificates unlocked.
**Impact**: Motivates completion. Turns a game into a credential pursuit.
**Effort**: ~4h (frontend signal + progress bar component)

### LC-2. Credit Balance Display
**Current**: No concept of learning credits anywhere in the UI.
**Target**: "My Credits" section in landing page (auth-aware nav) showing accumulated credits from adventure levels + events attended.
**Impact**: Makes credits tangible. First step toward stackable credentials.
**Effort**: ~1d (backend credit aggregation + frontend display)

### LC-3. Certificate / Badge Gallery
**Current**: NFT badges exist on-chain but there's no user-facing gallery.
**Target**: "My Badges" page showing earned cNFT badges organized by credential tier (Foundations → Core → Practitioner).
**Impact**: Social proof, shareable credentials, LinkedIn integration potential.
**Effort**: ~2d (new page + Solana RPC for badge metadata)

### LC-4. Admin: Curriculum Dashboard
**Current**: Admin shows event stats (checked-in, deposits). No learning analytics.
**Target**: Per-event competency dashboard — pass rates per level, average completion time, credit distribution.
**Impact**: Organizers see educational outcomes, not just attendance.
**Effort**: ~3d (new admin tab + analytics queries)

---

## P2.6 — Solana Mobile UX (Issue 042)

> **Prerequisite**: Phase 10 (mainnet deployment). Mobile wallet integration is post-mainnet.
> Full plan: `.issues/042_solana_mobile_support.md`

### MOB-1. Mobile Wallet Adapter Registration
**Current**: Wallet detection only checks `window.solana` / `window.phantom` (browser extensions). No mobile wallet support.
**Target**: Register `@solana-mobile/wallet-standard-mobile` in `index.html`. MWA wallets (Phantom, Solflare, Seed Vault Wallet) appear as connectable options on Android Chrome.
**Impact**: Unblocks all wallet-dependent flows on mobile (deposit, claim, refund, escrow init, on-chain check-in).
**Effort**: ~5h (JS interop + wallet detection update + manual testing)

### MOB-2. Mobile Wallet Error Messages
**Current**: Wallet errors show generic "Failed to connect" messages. No mobile-specific guidance.
**Target**: Detect Android Chrome context. Show contextual messages:
- "Install Phantom or Solflare from Google Play" (no wallet detected on mobile)
- "Open this page in Chrome to connect your wallet" (non-Chrome browser)
- "Wallet not found" → link to MWA-compatible wallet list
**Impact**: Reduces support burden. Mobile users are new to Solana — need hand-holding.
**Effort**: ~2h (context detection + conditional error messages)

### MOB-3. Responsive Deposit Page (Mobile-First)
**Current**: Deposit page works on desktop. Mobile layout has cramped wallet selector + Solana Pay QR.
**Target**: Stack layout for mobile — wallet selector → amount → confirm → TX status. Larger touch targets. Sticky bottom CTA.
**Impact**: Deposit is the highest-friction mobile flow. Better layout = higher conversion.
**Effort**: ~4h (CSS refactor + mobile-specific layout)

### MOB-4. PWA Home-Screen Install
**Current**: No PWA manifest or service worker.
**Target**: Add `manifest.json`, service worker, PWA icons. Show install prompt on 2nd visit.
**Impact**: Staff can install scanner app. Attendees can add ticket page to home screen.
**Effort**: ~4.5h (see P3-2 upgrade above)

### MOB-5. Solana dApp Store Listing
**Current**: Not listed anywhere outside direct URL.
**Target**: Submit BeThere as a web app to Solana dApp Store. Prepare app metadata, screenshots, and description.
**Impact**: Discovery channel for Solana Mobile users (Seeker, Saga). Free distribution.
**Effort**: ~3h (listing only, no TWA wrapper)
