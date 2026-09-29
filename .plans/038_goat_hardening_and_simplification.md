# Plan 038: GOAT hardening + simplification package

**Created:** 2026-09-29, session `event-checkin-ba`.
**Trigger:** the "GOAT hardening + simplification" handoff, which follows
Wave 1 (`.plans/037`).
**Rule of the package:** cut first, then add.

**Standing constraints:**
- No push, no deploy, no secrets from agent sessions.
- Turnstile widgets and secrets are created by the owner.
- Every frontend change is checked in a browser at 390×844.
- The frontend first-load budget is binding: see `.issues/169`. Every "add"
  item below must report its br4 delta.

## DevEx

- [x] **D1:** `b3a00edc`. Added `[profile.fast-dev]`, an explicit rust link
  (`data-cargo-profile-dev`) and `frontend-leptos/serve.sh`. A view edit went
  from 120.6 s to 10.0 s; hot reload takes 9 s. The release dist is
  byte-identical.
- [x] **D2:** `0e1c6fae`. Removed `CARGO_BUILD_JOBS=1`. A cold build went from
  73.9 s to 35.9 s, peak RSS from 2.8 to 2.4 GB, and the wasm is
  byte-identical.
- [x] **D3:** `.issues/168`. sccache works when the target-dir path is the
  same (135/135 hits) and cannot hit across paths. The comment is fixed. The
  global `incremental=false` trade-off is the owner's call.
- [ ] **D4:** cargo-heal in the fix loop. Use it as needed; it is not a
  deliverable. The memory `cargo-heal-not-adopted` records a 2026-09-13
  evaluation.
- [x] **D5:** loop discipline, followed throughout.

## P0

- [x] **P0-1:** `8513ea26`. `utils::poll_policy`: 2.5 s in the event window,
  15 s outside it, `?poll_ms=` to override, 0 requests while hidden.
  Checked in a browser.
- [x] **P0-2:** Turnstile (`.issues/170`). Inert until the owner sets both
  keys; +5,209 B br4. Scope decision:
  - `/api/waitlist`, which is anonymous and appends to a sheet;
  - `/api/public/register`, which is JWT-gated but still scriptable;
  - **not** staff walk-in (staff-authed, and a door challenge is friction).

  It is enforced only when `TURNSTILE_SECRET_KEY` is set.
- [x] **P0-3:** `e2e/visual.spec.ts` + `e2e/a11y.spec.ts` over 7 pages ×
  390/1440, a fixture (`e2e/fixtures/seed.sql`), and CI wiring (DEV_MODE with
  a synthetic admin, `--update-snapshots=missing`, and a `visual-baselines`
  artifact).
  - Locally: 42/42 green, three runs.
  - The axe self-test proves the gate fails on injected violations.
  - **Open:** commit the Linux baselines from the first CI run's artifact.
    This machine has no container runtime, so it cannot render Linux
    baselines.
  - Found and fixed along the way: `.issues/172` (the `/api/auth/me` per-IP
    limit) and the muted-text contrast.
  - Owner decision: `.issues/171` (brand CTA fills).

## P1: cut

- [x] **P1-1:** landing rebuild, one screen = one decision. Measured at 390×844,
  headless Chrome, with the P0-3 fixture plus two probe events:

  | | Before | After |
  |---|---|---|
  | Page height | 4,235 px | 3,211 px |
  | Above the fold | brand eyebrow, 3 audience tabs, headline, paragraph, Solana pill, 3 stat cards, 2 CTAs | headline, one value line, **one** CTA, 2 event cards, "See all" |
  | First event card | below the fold | 2nd card ends at 781 px, "See all" at 834 px (EN and TH) |

  - Signed-in visitors see "My registrations" directly under the hero.
  - How it works is three icons on one line.
  - Organizers reach the existing "host events" link and the waitlist.
  - Copy deviates from the brief: "0 บาทล่วงหน้า" is false for deposit
    events, and "never forfeited" is THB-only (the guard
    `deposit_promise_has_one_home` caught it). So the value line is
    "Deposit to reserve · show up · get 100% back".
  - 21 dead CSS classes were removed (the class audit enforces it).
  - First load −7,055 B br4.
  - Before/after evidence is these numbers plus local screenshots (not
    committed: public repo, and Mac-only renders).
- [~] **P1-2:** attendee flow audit (`.issues/173`). The happy path is 4 taps
  plus form fields, with no dead ends.
  - Fixed: C1 (the claim countdown read "28901h"; it now reuses the event
    page's day-aware formatter) and C2 ("Recommended" on the only payment
    option).
  - `/claim` was added to the visual + a11y page list (45 e2e tests).
  - **Awaiting owner approval:** C3–C5, C7 (LM-2 ticket banners), C8 (LM-3
    claim trim). C6 folds into P2-b.

## P2: add

- [x] **P2-a:** "Show ticket / Hide" on each ready registration on the
  landing (1 tap from the landing).
  - The QR is an inline SVG path from `utils::qr_gen::qr_svg_path`: no
    image request.
  - The API adds `qr_url` exactly when the ticket page would show a QR.
  - Verified: jsQR decodes the rendered SVG to the exact check-in URL (EN and
    TH), no overflow at 390 px, and both states have snapshots and an axe
    pass.
  - Cost: +3,319 B br4.
  - Found and fixed: `.issues/174` (free events sent attendees to "Complete
    Deposit") and an unnamed scanner `<select>` (surfaced by the second
    fixture event).
  - The short display code still needs its own issue (not filed yet).
- [x] **P2-b:** labelled When / Where / Format / Capacity rows in the event
  details card.
  - When names the viewer's zone ("(GMT+7)"; a London viewer sees "5:00 AM
    (GMT+0)").
  - Where links to the organizer's map URL, else a Maps search of the
    venue.
  - Capacity replaces the big card, with "เต็มแล้ว" when full, and is hidden
    once the event ended.
  - The inline-styled format pill is gone; `capacity_indicator.rs` and 8
    dead CSS classes are deleted.
  - +2,009 B br4. Checked in EN, TH, full and London in headless Chrome.
- [x] **P2-c:** TH/EN toggle. Wave 1's `LanguageSwitch` bar is at the top
  right of every attendee page in this session's 390 px captures. The pick
  persists under `bethere.lang`, and every probe reported
  `scrollWidth <= innerWidth`. No change needed.
- [x] **P2-d:** first-visit privacy notice on attendee pages.
  - Copy follows `docs/pdpa_ropa.md`: one sign-in cookie, cookie-free
    analytics, no tracking cookies. It is a notice, not a consent prompt,
    because nothing is optional.
  - Links to /privacy. The dismissal persists under
    `bethere.privacy_notice`.
  - Fixed position and after `<main>`; z-index 70, above the sticky CTA.
  - 20 samples show it adds no layout shift. They also exposed a
    pre-existing ~0.95 CLS on the event page (`.issues/175`).
  - `e2e/privacy-notice.spec.ts` covers visibility, the link, axe, a
    snapshot and dismissal across reloads.
  - Every other spec pre-dismisses it. +2,742 B br4.
- [x] **P2-e:** a "Marketing emails" section on /profile.
  - It shows the combined state from the new no-store `GET
    /api/privacy/marketing-consent` and says "ยกเลิกได้ทุกเมื่อ".
  - Withdrawal uses the existing audited unsubscribe (attendee rows plus the
    developer profile).
  - The premise was partly stale: an opt-out already existed at
    /data-privacy, linked only from /privacy.
  - Found: unticking the profile's own "contact me" box cleared only the
    developer profile. It now withdraws everywhere (guard test).
    `set_marketing_consent` rewrites only rows that change.
  - Opt-in stays at registration (the per-event context).
  - Checked in the browser (EN and TH), and D1 rows were read back.
    +3,328 B br4.
- [x] **P2-f:**
  - "View a sample event →" in the landing's no-live-events state. It links
    to `SAMPLE_EVENT_SLUG` (a new `[vars]` entry, empty by default, so the
    button stays hidden). **Owner:** seed a demo event and set the var; none
    exists today.
  - The footer shows "v0.2.0 · <sha>" from `build.sh` (`BETHERE_GIT_SHA`
    via `option_env!`), hidden when unknown.
  - Both checked in the browser: sha = HEAD; the button appears only with
    the var set. +898 B br4.

## P3

- [x] **P3-a:** `/faq` with Attendees / Organizers tabs over native
  `<details>` accordions.
  - Content: 9 attendee questions (the moved landing FAQ plus account,
    ticket, claim, cancellation and data) and 4 organizer questions (host,
    capacity, check-in, settlement).
  - The promises interpolate from `deposit_copy`. No claim-TTL number is
    stated, because prod's value is configurable and unknown here.
  - The landing's FAQ section and the header nav FAQ links are gone. The
    footer links `/faq`. 5 dead CSS classes deleted.
  - Checked EN and TH at 390 px (no overflow). `/faq` joins the visual +
    a11y list (51 e2e). +6,246 B br4.
- [x] **P3-b:** on /discover, an event with an organizer poster renders as
  a card.
  - The 4:5 poster is 88×110 with the date chip opaque on its corner; rows
    without a poster stay plain. Fixed size and `loading="lazy"`, so no
    layout shift.
  - Create-event shows a nudge while the poster is empty. It makes no OG
    claim: there is no per-event `og:image` today, so the brief's "feeds OG
    images" is not true yet.
  - The fixture gives `e2e-free` an inline-SVG poster, so snapshots cover
    the card. 51/51 e2e.
- [x] **P3-c:** no change needed. The premise was stale: `js/lazy_assets.js`
  already loads jsQR only when `BarcodeDetector` is missing.
  - Probe: 0 jsQR requests on /, /discover, /e/…, /ticket, /admin and
    /staff.
  - `/jsqr-1.4.0.js` serves br (33,223 B) and identity when asked.
  - **Early Hints is owner-gated:** it is a Cloudflare zone setting, and the
    site is on workers.dev. Unverified whether it is available there.
- [ ] **P3-d:** fluid type scale.
- [ ] **P3-e:** LINE in-app browser auth. Needs staging and a phone, so it is
  owner-run.

## Separate design issues (filed; not in this package)

- `.issues/176`: waitlist queue with email auto-promotion.
- `.issues/177`: public organizer profiles and attribution.
- `.issues/178`: short booking display code. P2-a is QR-only.

## Found along the way

- `.issues/169`: `size_budget_guards` is red on develop, because the
  baseline is past the warn line (from Wave 1 `e1161fdd`).
- At 390 px, the `/dashboard/live` header has a large empty gap above the
  controls. This predates this plan.
- The ticket page with a pending deposit shows "Ready for Check-In", then
  "Deposit Required", then "Verifying", all at once. This is input for P1-2
  (LM-2).
