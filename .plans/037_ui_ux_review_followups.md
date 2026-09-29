# Plan 037: Follow up on the 2026-09-29 UI/UX review

**Created:** 2026-09-29, session `event-checkin-5f`.
**Trigger:** the UI/UX review handoff of 2026-09-29, which lists six task
groups and a backlog.

**Owner decisions (already made; do not re-ask):**
- Attendee pages are English + Thai, using `leptos_i18n`:
  - the attendee can pick the language, and the choice persists in
    localStorage;
  - with no stored choice, fall back to `navigator.language` (`th` means TH,
    anything else means EN);
  - dates follow the chosen locale.
- Staff, admin and scanner pages stay English.

## 1. claim_token served as contact_handle (bug)

- [x] Root cause, from a read-only probe of the dev sheet header:
  - the sheet is a Luma export, so the `contact_handle` standard slot L is
    `claim_token`;
  - unmapped keys borrowed the standard slots.
- [x] Fix: `ColumnMapping::resolve` everywhere. Regression test with the
  real header row. Commit `31c5d469`, `.issues/167`.
- [ ] Staging repro. The fix is not on staging and this session may not
  deploy, so this waits for the next staging deploy (owner).
- [ ] Part B of `.issues/167`: the dedup falls back to a shared sheet across
  events. Needs a product decision.

## 2. EN + TH for attendee pages

Phase 1: infrastructure.
- [x] Add `leptos_i18n` 0.6 with `csr` only: no ICU formatting data and no
  cookie. The catalog is `locales/{en,th}.json` through
  `[package.metadata.leptos-i18n]` and `load_locales!()`.
  - [ ] Owner decision: `load_locales!` is deprecated upstream. The
    recommended `build.rs` + `leptos_i18n_build` path always pulls ICU
    datagen networking (ureq/rustls/webpki-roots, ISC and
    CDLA-Permissive-2.0), which `deny.toml` rejects. Moving needs per-crate
    licence exceptions (build-only; nothing ships) or staying on the macro
    until upstream splits the dependency.
- [x] Wrap `App` in `I18nContextProvider`. The switch (`src/locale.rs`):
  - sits in the flow above attendee pages only, and is labelled in the
    target language;
  - the pick persists in localStorage `bethere.lang`;
  - with no pick, it follows `navigator.languages`;
  - it restores in an Effect, because the provider's own Effect overwrote a
    direct set.
- [x] Dates: `format_event_day{,_parts}`, `format_event_datetime` and
  `series_nav::short_date` follow the locale (`en-GB` / `th-TH`, with the
  Buddhist-era year) through Intl, so they add no bytes.
- [x] Bundle: +31.5 KB br4 over HEAD (1,828,205 → 1,859,707), plus ~3 KB for
  the view-interpolation switch. Baseline updated.

Phase 2: the scattered Thai strings.
- [x] `public/discover.rs`: all strings, status pills, date chips.
- [x] `login.rs`: all strings except the wallet button (phase 3,
  `wallet_signin.rs`).
- [x] `public/feedback.rs`: all strings. The stored answer values stay the
  Google Form's Thai wording; only the labels translate
  (`tests/i18n_catalog.rs` pins TH label = stored value).
  - Verified: an EN click submits the Thai value (intercepted request).
- [x] `ticket/series_nav.rs`.
- Of the other files on the list, the Thai hits in
  `deposit/{choose_payment,thb_payment,already_deposited}.rs`,
  `ticket/{in_person_view,announcement}.rs` and `landing/page.rs` are only
  `฿` or test fixtures. Their English UI copy is phase 3.
- Verified in headless Chrome at 390×844 against local `wrangler dev`:
  - `/discover`, `/login`, `/login?next=/feedback`, `/feedback` in both
    languages: a live switch, persistence across reload, TH browser →
    Thai, an explicit EN pick sticking;
  - no horizontal scroll;
  - the feedback progress counter re-renders.

Phase 3: every string on the attendee routes comes from the catalog.
- [x] Done by four parallel agents, one page group and one namespace each,
  cherry-picked onto develop:
  - `/e/{slug}` (`event`, 138 keys);
  - deposit (`deposit`, 181);
  - ticket + claim (`ticket` 184, `claim` 96);
  - landing, site header, privacy, data-privacy, past events, recap,
    post-event form, wallet sign-in (`landing`, `privacy`, `recap`,
    `wallet`).
- [x] The deposit promise has one home in both languages
  (`deposit_copy::{never_forfeited, thb_refund_window}(locale)`). The test
  also scans the catalog and the Thai phrasing.
- [x] The switch also shows on `/data-privacy`, `/past-events`,
  `/events/*/recap` and `/events/*/post-event-register`.
- [x] `claim/page.rs` was 1,122 lines; the success screen moved to
  `claim/success.rs`, leaving 962.
- [x] Bundle: plain keys render through `locale::tr()` (one
  `Signal<&str>` type instead of ~700 distinct closure types), which saved
  ~60 KB. The whole bilingual attendee app costs +102 KB br4 over the phase-2
  baseline (~27 KB of it is the catalog text); it is now 93.7% of the 2 MiB
  budget and the baseline was updated deliberately. twiggy shows no single
  hotspot; the rest is the extra reactive views.
- Verified in headless Chrome at 390×844, TH browser → EN switch, on local
  `wrangler dev` fixtures, for: `/`, `/e/{slug}` (hybrid, THB deposit),
  `/ticket` (checked in), `/deposit` (unpaid), `/claim` (NFT not yet set),
  `/privacy`, `/data-privacy`, `/past-events`, `/events/*/recap`.
  - Every page renders in Thai, switches live, sets `<html lang>`, and has
    no page errors or horizontal scroll.
  - The English left in TH is brands, organizer content, technical proper
    nouns (privacy notice) and deliberate loanwords.
- Not yet opened in a browser: every deposit / ticket / claim sub-state the
  agents listed (slip pending, USDC flows, refund, quiz, mint success). The
  wallet modal.

Owner decisions raised by phase 3:
- [ ] Privacy notice PDPA section numbers (§5 "s.37 technical-impossibility
  exemption", /data-privacy "s.29 erasure", "s.38 contract exemption") look
  wrong against the Act (erasure is usually s.33, contract basis s.24(3)).
  EN and TH both carry them as written. Needs legal review.
- [ ] Deposit page EN copy "Don't lose your deposit — claim it back" (USDC
  refund window) contradicts "never forfeited" for THB readers only if
  shown on THB; it is the USDC escrow path, where forfeiture is real. Check
  the wording anyway.
- [ ] Thai term consistency: the landing page says เหรียญตรา and the ticket
  page says "badge".
- [x] `utils::format_timestamp` (receipt "Date") and
  `deposit::types::format_refund_deadline` (`MM/DD`) are still US-style.
  - Done 2026-09-29 (`event-checkin-1b`): both delegate to
    `utils::format_event_datetime`, so they read `20 Sept 2026, 10:04` /
    `20 ก.ย. 2569 10:04` in the attendee's language (was `Sep 20, 2026,
    10:04 AM` and `10/01 13:00`). Staff pages that use `format_timestamp`
    (scanner, admin, slip queue) get the `en-GB` form too. Checked by
    rendering `/deposit/:id` with a mocked verified USDC deposit in EN and
    TH (Asia/Bangkok); Playwright 51/51.
- [x] Still English on attendee pages, from shared code: `wallet_error.rs`
  messages, `components::postponed_banner`, the escrow cluster-mismatch
  toast, and `dev_profile.rs` "Connect Wallet →".
  - Done: wallet messages and the cluster mismatch come from
    `locales/*/wallet.json` (`tx_*`, `cluster_mismatch`). Staff pages pass
    `Locale::en`. The postponed banner and badge use `status.postponed`.
    Guard: `frontend-leptos/tests/wallet_error_messages.rs`.
  - Checked on the local e2e worker: a postponed event shows "Postponed" /
    "เลื่อนจัด" on `/e/{slug}` and on the landing card.
  - Done later: `/profile` is an attendee path and the whole page reads
    from `locales/*/profile.json`, "Connect Wallet →" included. The
    link-callback banner (`?email_link=` / `?linked=` / `?error=`) is
    parsed once and translated at render time
    (`pages::profile_link_result`), so it follows a saved TH pick. Interest
    tags and role options are stored values and stay as they are. Guard:
    `frontend-leptos/tests/profile_link_result.rs`. Checked on the local e2e
    worker in EN and TH, with a live switch. Cost: +4,867 B br4.
  - Removed `translate_api_error`, `api_error_message` and
    `wallet_error_message`: they had no callers, only English text.

## 3. Admin on mobile (390 px)

- [x] The header title is hidden at ≤480 px (the active Admin icon says where
  you are). Nav links and sign-out are ≥44 px.
- [x] The sidebar nav wraps as chips instead of running off the edge
  ("Cancellat…"). The `Alt+N` hints are hidden on phones.
- [x] Event card: the actions flow as a wrapping row (they were 4 full-width
  rows). List cards had the title squeezed and the actions off-screen; they
  now wrap under the title.
- [x] Attendee toolbar: Generate QR Codes + Export CSV, then a `<details>`
  "More ▾" menu (Refresh, Flush Cache, Export Audience, Select All Pending,
  walk-in export/sync). The menu closes on pick and spans the row on phones.
- [x] Attendee row: fixes, not a menu.
  - The name was starved to zero width; it now has its own line and wraps.
  - The badges wrap.
  - The actions wrap left-aligned; with `flex-end` they had overflowed off
    the card's left edge.
  - All actions are ≥44 px. The checkbox's hit area grows to 44 px through
    `::after`, with no visual change.
  - Why not a menu: all row actions stay one tap away. A menu for Delete may
    still be worth it (owner call).
- [x] The filter pills are ≥44 px.
- Verified (headless Chrome, local `wrangler dev`, hybrid event, 69
  attendees):
  - 390×844, Events and In-Person views: no truncated text, no horizontal
    scroll, no element past the viewport, and 0 tap targets under 44 px apart
    from the 22 px checkbox visual, whose extended hit area was confirmed with
    `elementFromPoint` ±8 px.
  - 1280×900: no regressions.

## 4. Admin information design (desktop)

- [x] The metrics are named by their denominator:
  - "In-Person checked in · 26 of 32 in-person registrants";
  - "All tracks checked in · 26 of 69 registrants, online included" (was
    "Check-in Velocity").
- [x] The selected event shows once: the list skips it, since the detail
  card is above.
- [x] The raw SHEET ID is gone. The detail card has "Copy sheet ID" (the
  sidebar already opens the sheet); the list cards drop the field.
- [x] ORGANIZERS shows up to two emails and "+N more"
  (`organizers_label`, `tests/admin_organizers_label.rs`).
- Noticed, not changed: the event cards render `utils::escape_html(name)`
  as a text node, so a name containing `&` shows `&amp;`.

## 5. Design tokens

- [x] Undefined tokens. There were 15, not the 2 in the review; the claim
  quiz's `--fg`/`--muted`/`--primary`/`--bg` had no fallback, so its
  heading/icon colours and selected-option border silently vanished.
  - Each maps onto an existing token (`--text-primary`, `--text-muted`,
    `--accent`, `--bg-primary`, `--border`, `--bg-card`, `--danger`).
  - `--radius-md` and `--font-mono` become real tokens.
  - The guard `tests/css_tokens_defined.rs` fails on any undefined
    `var()` (negative control done). The one documented exception is the
    confetti keyframe variables, which nothing sets.
- [x] `admin_feedback.rs`: 128 inline styles down to 5 (the data-driven bar
  widths).
  - 78 named `afb-*` classes in `style-23-admin-feedback.css`; the scope and
    filter toggles become `is-active` classes; bar colours become classes.
  - Rules are scoped under `.admin-feedback-page`. Unscoped, they lost to
    `.admin-section-header h3` and the page shifted by 98 px.
  - Verified by pixel diff of the section at 1280 and 390 against the
    previous build: identical height; the only pixels that changed are the
    two token recolours (subtitle `#94a3b8` → `--text-muted`, active pill
    indigo → `--accent`).
- [x] Toast: classes; an always-present `role="status" aria-live="polite"`
  region (content changes inside it, so it is announced). Verified in the
  browser: same colours and position.
- [ ] Breakpoints: 8 sets → 360/480/768. Not started; it touches every
  stylesheet and needs a per-file visual diff.
- [ ] Remaining hardcoded colours in other Rust files.

## 6. Accessibility

- [x] Clickable non-interactive elements. 30 found.
  - 13 collapsible section headers (11 in `event_form.rs`, the audit panel,
    the on-chain events panel) are now keyboard-operable: `role="button"`,
    `tabindex`, `aria-expanded`, and Enter/Space through
    `utils::is_activation_key`. They stay `<div>`s, so there is no visual
    change.
  - The rest are backdrop click-to-dismiss layers (fine as they are) and the
    payment method cards, which already contain a real button.
  - The admin "switch tab to view" was styled as a link with an empty click
    handler; it is now plain text.
- [x] ImageLightbox: `role="dialog"`, `aria-modal`, `aria-label`. Focus goes
  to the close button on open, Tab is trapped, Escape closes, and focus
  returns to the opener. Visibility flips instantly on open (the fade is
  kept), because a `visibility` transition refused focus on the first frame.
- [x] SIWS wallet modal and quiz import modal: `role="dialog"` and a labelled
  close button; the wallet modal closes on Escape.
- [x] `alt`: every meaningful image already has alt text (most via the
  catalog); the empty ones are deliberately decorative and commented.
- Verified in headless Chrome: the lightbox focus sequence (open → close
  button, Tab → stays, Escape → back to "Full Screen"); SIWS
  role/label/Escape; event-form header `aria-expanded` toggles on Enter and
  on Space.
- [ ] Adventure overlays have no dialog role. The game has its own key
  handling; left alone.

## Backlog

- LM-2: one status-banner slot on the ticket page.
- LM-3: a shorter claim success screen.
- P2-2: batch/manual check-in for staff.
- Light mode, after §5.
