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

Phase 3: every string on these routes comes from the catalog:
- `/e/{slug}`
- deposit, ticket, claim
- discover, landing, feedback, privacy

Done when:
- switching works on every page without a reload;
- wasm32 clippy and `cargo test` pass;
- each page is opened at 390×844 in both languages.

## 3. Admin on mobile (390 px)

- [ ] The header title and nav labels are not truncated.
- [ ] Event card: fold its 4 buttons into primary actions plus an overflow
  menu.
- [ ] Attendee toolbar: primary actions (Generate QR Codes, Export CSV) plus
  a "···" menu.
- [ ] Attendee row: primary action plus a menu.
- [ ] Touch targets are ≥ 44 px, with no horizontal scroll.

## 4. Admin information design (desktop)

- [ ] Label the two progress denominators, or merge them.
- [ ] Show the selected event once.
- [ ] Move the SHEET ID behind a copy button.
- [ ] ORGANIZERS shows names, not a bare count.

## 5. Design tokens

- [ ] Resolve undefined `var(--color-primary)` / `var(--color-text-muted)`.
- [ ] Move inline colours into classes, starting with `admin_feedback.rs`.
- [ ] Reduce breakpoints to 360 / 480 / 768.
- [ ] Toast: classes plus `aria-live="polite"`.

## 6. Accessibility

- [ ] Clickable `<div>` → `<button>`, starting with `event_form.rs` and
  `adventure/page.rs`.
- [ ] ImageLightbox gets `role="dialog"` and a focus trap.
- [ ] `alt` text on meaningful images.

## Backlog

- LM-2: one status-banner slot on the ticket page.
- LM-3: a shorter claim success screen.
- P2-2: batch/manual check-in for staff.
- Light mode, after §5.
