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
- [x] Staging repro. **Done (2026-10-03, `event-checkin-0b`)**, see the last
  paragraph of this item. The fix is not on staging and this session may not
  deploy, so this waits for the next staging deploy (owner).
  **Blocked (2026-09-29, `event-checkin-8c`):** the fix is now on staging (`bb8ac906`) and prod, but the repro needs a sheet with the Luma header shared with staging, which staging lacks (owner), and it would append rows to that sheet.
  **Owner question (2026-09-30, `event-checkin-aa`):** "may we share a sheet
  with the Luma header row with the staging service account, knowing the
  repro appends a registration row to it?"
  **Reopen trigger (2026-10-01, `event-checkin-45`):** the owner shares a sheet with the
  Luma header with the staging service account (the same answer opens the
  two-event run below).
  **Partial run (2026-10-02, `event-checkin-4f`):** the owner shared sheet
  `19DIsrxl…` with staging. Staging can read it (sync `success`,
  `total_in_sheet` 0 before the run). But its header row is BeThere's own
  named layout (`contact_handle` at 11, `claim_token` at 21), **not** a Luma
  export, so it cannot reproduce 167 part A. On it, event `r037-a-1790927701`
  appended a row with the handle at 11 and the claim token at 21. A
  sheet→D1 sync and a duplicate registration both returned the UUID
  `…5b2d81`, not `@r037_handle_alpha`. Correct mapping, but not the Luma case.
  **Reopen trigger:** the owner puts the real Luma header row on the sheet
  (or shares a Luma-export sheet), or OKs this session writing that header to
  the throwaway sheet. Then rerun: register, sync, check that `claim_token`
  stays a UUID and the handle lands in the Luma handle column.
  **Done (2026-10-03, `event-checkin-0b`).** The owner lifted the row-1
  protection; the session wrote the 33-column Luma header (from
  `domain/tests/unmapped_column_aliasing.rs`) to sheet `19DIsrxl…`, which
  also cleared the 2 old test rows. On staging, event `r037-c-1791038583`
  (created, activated, KV visible): one registration appended one row.
  `claim_token` (L, index 11) holds the UUID `…24190d`; `api_id`, name,
  email, `created_at`, `approval_status`, `ticket_name` and
  `Participation_Type` landed in their Luma columns. `qr_code_url` (10),
  `amount_tax` (12), the handle column (30) and `payment_status` (32) stayed
  empty. A sheet→D1 sync returned `updated 1, errors 0`, and a duplicate
  registration returned the same attendee and the same UUID token, not
  `@r037_handle_gamma`. So 167 part A holds end to end.
  **Correction to the reopen note above:** the handle does *not* land in the
  Luma `Contact Handle / โปรดระบุ Username` column. The header has no handle
  key `ColumnMapping` knows, and the regression test asserts index 30 stays
  empty. The handle lives in D1 only. Aliasing that header is a product call
  (organizers would then see handles in their sheet); it is not part of 167.
  Event C is archived; its attendee row stays in staging D1 and one row in the sheet.
- [x] Part B of `.issues/167`: the dedup falls back to a shared sheet across
  events. Needs a product decision.
  **Closed (2026-10-06, `event-checkin-42`):** every sub-step below is done; `58fc5186` is on `main` and on prod since `438c392d`.
  **Blocked:** the owner decided on 2026-09-29 (`.issues/167`), but the implementation is in `worker/`, peer `event-checkin-16`'s area.
  - [x] Built 2026-09-30 (session `event-checkin-aa`), on branch
    `feature/167-empty-roster` (`16de5c3c`, off `develop` `04ab57b5`; the
    peer gate was stale, since `event-checkin-16` is gone). An event created
    on or after `fa0dca12` (2026-08-11T18:59:34Z, D1 attendee writes made
    authoritative) reads an empty D1 roster as empty; older events keep the
    sheet fallback. Every per-event roster read passes
    `EmptyRoster::for_event`; the organizer's sheet sync forces `ReadSheet`.
    Guard `worker/tests/empty_roster_policy.rs` (4, floored); two mutants
    turn it red. Workspace clippy, 79 worker binaries and 104 Python tests
    are green. Details are in `.issues/167`.
  - [x] Merge: `58fc5186` on `develop`; on prod in release `438c392d` (2026-09-30, `event-checkin-b5`).
    **Answered (2026-09-30):** the RTM #6 event shares no sheet. The prod
    date question is moot: a read-only prod D1 query shows the only events
    created after the cutoff are the RTM #6 event (57 D1 attendees, so its
    roster read is unchanged) and a ComfyUI draft (0). `fa0dca12` reached
    `main` on 13 Aug (`f3c66652`); Cloudflare keeps only 10 deployments, so
    the exact prod date cannot be read back. The staging smoke's roster read
    ran through the new path and passed.
  - [x] A staging run with two events sharing one sheet (optional now).
    **Done (2026-10-02, `event-checkin-4f`)** on staging, on sheet
    `19DIsrxl…` (the header doesn't matter here), with `dev-token`. Events
    `r037-a-1790927701` (A) and `r037-b-1790927850` (B) were both created
    after the cutoff. Before any registration on B, B's roster read 0, not
    A's row. The same account then registered on B and got a new attendee
    (`…1cb7fe`) and claim token (`…e13e83`), not A's (`…7dcbeb`/`…5b2d81`), so
    there was no cross-event dedup. Each roster lists only its own attendee.
    B's sheet sync counts 1 of the sheet's 2 rows and leaves B's roster at 1.
    The sheet holds both rows, each with its own handle and token. Both
    events are archived afterwards; their D1 rows and the 2 sheet rows remain.
    Both former owner questions (merge timing, prod date of `fa0dca12`) are
    answered above.
    **Gate (2026-10-01, `event-checkin-a6`):** the same one as the staging
    repro at the top of this section. Staging has no sheet shared with its
    service account (no `CONTACTS_SHEET_ID`), and the run appends rows to
    whatever sheet it uses. The code needs no deploy; it is on staging and
    prod. Unit coverage is `worker/tests/empty_roster_policy.rs`.
    **Reopen trigger:** the owner shares a sheet with the staging service
    account (the same answer unblocks both items).

## 2. EN + TH for attendee pages

Phase 1: infrastructure.
- [x] Add `leptos_i18n` 0.6 with `csr` only: no ICU formatting data and no
  cookie. The catalog is `locales/{en,th}.json` through
  `[package.metadata.leptos-i18n]` and `load_locales!()`.
  - [x] Owner decision (2026-09-29): stay on `load_locales!` until upstream
    splits the ICU datagen dependency. `load_locales!` is deprecated upstream. The
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
- [ ] **Owner, 2026-09-29: legal review first; text unchanged until then.** **Blocked (2026-10-08, `event-checkin-fe`):** only the lawyer's review of `privacy.json` (EN + TH) remains.
  Privacy notice PDPA section numbers (§5 "s.37 technical-impossibility
  exemption", /data-privacy "s.29 erasure", "s.38 contract exemption") look
  wrong against the Act (erasure is usually s.33, contract basis s.24(3)).
  EN and TH both carry them as written. Needs legal review.
  **Blocked:** owner; waiting on legal review.
  **Owner question (2026-09-30, `event-checkin-aa`):** "has the legal review
  of the PDPA section numbers (s.37, s.29, s.38 vs. s.33 and s.24(3)) come
  back, and with which numbers?"
  **Reopen trigger (2026-10-01, `event-checkin-45`):** the legal review returns section
  numbers; then change EN and TH together.
  **Still waiting (2026-10-08, `event-checkin-19`):** the owner's legal review of the section numbers.
  - [x] Citations checked against the Act (2026-10-08, `event-checkin-c0`):
    `docs/pdpa_citation_check.md`, for the reviewer. Erasure is s.33 and
    the contract basis is s.24(3), as suspected. The `/privacy` §5 "s.37
    technical-impossibility exemption" has no counterpart in the Act, so
    that line needs a decision on the basis, not just a new number. No
    catalog text was changed; the item stays open on the legal review.
- [x] **Kept (2026-09-29):** it renders only in `usdc_payment.rs`, the escrow
  path where an unclaimed deposit is really lost.
  Deposit page EN copy "Don't lose your deposit — claim it back" (USDC
  refund window) contradicts "never forfeited" for THB readers only if
  shown on THB; it is the USDC escrow path, where forfeiture is real. Check
  the wording anyway.
- [x] Thai term consistency: the landing page says เหรียญตรา and the ticket
  page says "badge".
  - Done 2026-09-29 (owner: one word): "badge" everywhere, the most-used
    form (28 of 43). เหรียญตรา (9) and แบดจ์ (6) replaced in the landing,
    event, recap and privacy catalogs; "NFT badge" word order and spaces
    around the loanword follow the existing ticket strings.
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
  - Done 2026-09-29 (owner: yes): Delete moved into a per-row "⋯"
    `<details>` menu (44 px summary), keeping its tap-again-to-confirm step
    inside. The menu opens under its own button at 390 and 1280 px.
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
- [x] Breakpoints: 8 sets → 360/480/768. Not started; it touches every
  stylesheet and needs a per-file visual diff. Held 2026-09-29 (session
  `event-checkin-4e`): it moves layout on every demo-facing page one week
  before the 6–8 Oct freeze; start it after the take.
  **Blocked:** the 6–8 Oct demo freeze; start after the take. No owner question; it is dated.
  **Reopen trigger (2026-10-01, `event-checkin-45`):** the 8 Oct take is filmed.
  **Done outside the landing (2026-10-08, `event-checkin-19`, branch
  `feature/037-breakpoints`):** the trigger was met (take filmed 8 Oct, plan
  044 1.5). Every width query outside `style-23-landing.css` is now
  `max 359 / 480 / 767` or `min 481 / 768` (12 queries in 10 files moved):
  - 720 → 480 (live dashboard header): its controls fit on one row down to
    481; on 767 they broke onto their own row at 721–767.
  - 640 → 767, 641 → 768 (sticky CTA + its caption, quiz editor and the event
    form grid, inbox rows, notification hint), 600 → 767 (community-link
    rows), 768 → 767 (at exactly 768 both `max 768` and `min 768` matched).
    480 was tried for `.quiz-settings-grid` and rejected: the event form
    reuses it and went to a ragged two columns at 481–640.
  - 380 → 359 (ticket series neighbours stay two-up on 360–430 phones);
    `min 420` → `min 481` (deposit method grid): at 420–480 a single
    PromptPay card sat in half the width with its button wrapped; now full.
  - Method: one develop build on the local fixture worker; per page and width
    the media rules were rewritten in place and screenshotted before/after
    (16 pages, 17 widths 360–800, plus the admin Quiz and Edit Event tabs).
    No new horizontal overflow, no page errors. Then the real rebuild was
    checked against the predicted images (deposit 0 px, the rest noise).
    The 390 and 1440 e2e baselines cannot change: no moved edge lies between
    the old and new value at either width.
  - Guard: `tests/breakpoint_set.rs` (floored, 4); it names all 12 old
    queries when run on develop's stylesheets.
- [x] Breakpoints, landing (2026-10-08, session `event-checkin-f0`, after
  pull 157 merged): the 13 width queries in `style-23-landing.css` moved
  900/860 → max 767, 560 → max 480, `min 901` → `min 768`; `min 1240` (the
  side rail) stays and joins the set as a min-only "wide" edge. The exemption
  is gone; the guard is now 359/480/767 (max) and 481/768/1240 (min).
  - A/B on the live prod landing, rules rewritten in place, 15 widths
    (481–1024): only 481–560 and 768–900 change, as intended. One defect
    surfaced: at 768–792px the desktop swimlane overflowed by up to 25px
    (four step columns at their longest word plus a fixed 200px route
    column). Fixed with `minmax(120px, 200px)` for the route column and a
    wrapping route header (the LIVE/DEVNET tag drops under the name when
    narrow). No overflow at any width; 1024 and 1440 are pixel-identical to
    before, and the e2e baselines (390, 1440) are untouched.
- [x] Remaining hardcoded colours in other Rust files (2026-09-29, session
  `event-checkin-4e`). 20 inline text colours in 6 files now use tokens:
  `#94a3b8`/`#64748b` → `--text-muted`, `#cbd5e1` → `--text-secondary`,
  `#fff` → `--text-primary` (wallet sign-in modal, NFC check-in, profile,
  landing nav badge, registration heads-up, admin track progress).
  - Guard: `tests/inline_text_colour_tokens.rs` (floored, 3). Restoring the
    old `landing/nav.rs` turned it red with 2 hits.
  - Verified at 390×844 on the local e2e worker: computed colours are the
    tokens (`rgb(233,228,211)`, `rgb(142,147,170)`), no page errors; all 19
    visual snapshots unchanged.
  - **Left on purpose (design call, not a token swap):** Solana brand
    green/purple (`#14F195`, `#9945FF` and their rgba tints) on the NFC,
    claim, event and profile pages; wallet and Google logo fills; the
    transaction-kind colours in `api/admin.rs`; the claim quiz's pixel-art
    palette in `claim/widgets.rs`; the status greens/reds in `nfc_checkin.rs`
    (`#4ade80`, `#f87171`) that have `--success`/`--danger` equivalents of a
    different shade. Moving them changes the look, so it needs an owner nod.
  - Checked, not a bug: the box before "Tap NDEF / Web Wallet" on
    `/checkin/nfc` is `IconName::Phone` (the Feather smartphone outline) at
    `icon-xs`, not a missing glyph.

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
- [x] Adventure overlays have no dialog role. The game has its own key
  handling; left alone.
  Done (2026-09-29, `event-checkin-8c`): the level-select, intro, level-complete, NPC/sign and puzzle cards have `role="dialog"`, `aria-modal` and an `aria-label` (the SIWS pattern). The key handling is unchanged, and there is no focus trap because any key dismisses. Guard: `tests/adventure_overlay_dialogs.rs` (2 tests, floored); removing the puzzle card's role turned it red. At 390×844 on the local e2e worker, Chrome's accessibility tree reads "dialog: Hello, Rust!" and "dialog: Select Level", any key and Escape still dismiss, the page has no errors, and the screenshots look unchanged.

## Backlog

- LM-2: one status-banner slot on the ticket page.
- LM-3: a shorter claim success screen.
- P2-2: batch/manual check-in for staff.
- Light mode, after §5.
