# 182: Pre-take visual QA fixes (7 on-camera items from the 2026-09-29 baselines)

**Status:** deployed (2026-10-05, session `event-checkin-87`, owner go): release merge `da6da377` on `main`, prod version `69b5cf19`, staging `58f4f9cc` ran the same tree first. Before that, fixed on develop (2026-10-01, session `event-checkin-4f`): items 1-8 merged via `937d87bd` (owner pre-approved) and pushed; CI run 36801022129 re-took the 16 missing Linux baselines, committed as `66c8b033` after an image check that shows all 8 fixed. First fixed on `feature/qa-take-fixes` by `event-checkin-b5`, item 8 on `feature/hero-poster-overflow`.

Found in the visual QA of the committed e2e baselines (2026-09-29). All seven
are on camera for the 8 Oct take. Each was checked on a credential-free local
worker with the e2e seed at 390x844, in EN and TH.

| # | Item | Fix | Verified |
|---|---|---|---|
| 1 | The event page's sticky bar covers the WHEN row | The bar also stays hidden while the hero "Reserve" CTA is on screen. On first view it was a second, identical button, and it covered the WHEN row. `public_event/page.rs`, in the same scroll listener as the reserve-zone rule (c93080eb, `.issues/173` C4). | Scroll probe. At 844 px on the fixture the bar never shows: the hero CTA stays in view until the reserve zone does, so no gap. At 390x600 it shows only in the gap (y 800-1000: hero gone, reserve not reached) and hides again. The footer is clear at the bottom. |
| 2 | The PromptPay refund sentence sits inside the tick list, and the ticks misalign on wrapped lines | The THB refund line is now a `.pe-refund-note` below the list, above a divider. `.pe-refund-item` is top-aligned, and the tick is a one-line box. | EN and TH screenshots of the Deposit Commitment card. |
| 3 | The claim page shows the event link as a bare URL | Labelled `event.view_event_page` ("View Event Page →" / "ดูหน้างาน →"), no underline until hover. Hidden when the event has no link. | TH screenshot. |
| 4 | The claim page has an unexplained "♥ 0" | Removed. The count was local to one tab, started at 0 every visit and was shared with no one, so it read as a real count when it was not. Component, CSS and keyframes deleted. | The claim page renders without it. `rg heart` over src and styles is empty. |
| 5 | The landing footer has both "Built with Rust & Solana" and "Built on Solana" | Kept the brand-column "Built with 🦀 Rust & Solana". Removed the bottom-row line, its `.landing-footer-powered` CSS and the `landing.hero.built_on_solana` key (EN and TH). | EN footer screenshot. |
| 6 | The language chip at y=0 is cramped | `.lang-bar` has 6 px of top padding. The label is a bordered pill (`.lang-switch-chip`) inside the 44 px tap target, and the border turns accent on hover and focus. | EN and TH landing and claim screenshots. |
| 7 | Admin mobile START/END show raw `2030-01-22 12:00:00 UTC+07:00` | `card_datetime` in `events_page.rs` uses the shared day-first `utils::format_event_datetime` plus `local_tz_label`, giving `22 Jan 2030, 12:00 GMT+7`. `format_date_display` stays for the form's refund-deadline line. | Admin screenshot at 390 px. |

## Item 8: the event poster overflows the column on phones (2026-10-01, session `event-checkin-4f`)

Found in the item 1 screenshot during a review of this branch; not one of the
seven QA items. It is on its own branch, `feature/hero-poster-overflow`, stacked on
`feature/qa-take-fixes`, so the seven-item branch stays as reviewed.

- **Cause:** at `max-width: 480px`, `.pe-hero-img` got
  `max-width: calc(100vw - 2.5rem)`. That cap ignores the column's padding:
  at 390 px it allows 350 px in a 318 px column, and `margin: auto` cannot
  center an image wider than its box, so it spills right. It applies to the
  generated poster and to uploaded posters and badges alike (all use
  `.pe-hero-img`), so it is on camera for any event.
- **Fix:** the media query keeps only `max-height: 70vh`; the base rule's
  `min(460px, 100%)` already caps the width at the column.
- **Verified** on a credential-free local worker (e2e seed, `reseed-kv`
  synced 2), headless Chrome, DPR 2, EN and TH, served CSS byte-identical to
  `dist/`. Bounding rects of `.pe-hero-img` vs `.pe-hero` and `.pe-hero-cta`:

  | Width | Before (old rule re-injected) | After |
  |---|---|---|
  | 390 | img 36-386, column 36-354: overflow 32 px | img 36-354, centered |
  | 360 | 36-356 vs 36-324 | 36-324 |
  | 430 | 36-426 vs 36-394 | 36-394 |
  | 1280 | 440-840 inside 296-984 (unchanged) | the same |

  Same in TH. No page errors. The "before" run is the check that the probe
  can fail.
- **Gates:** frontend `cargo fmt --check` and `cargo test --locked` (316 tests,
  39 binaries) pass. CSS only, so clippy has nothing new to see. No committed
  baseline shows the event page (removed in `ec458a89`), so none to re-take.

## Gates

- Frontend: fmt, clippy `-D warnings` (default and `--features staff`) and
  every test binary are green, including `css_tokens_defined`,
  `css_class_audit` and `i18n_catalog`.
- e2e against the local worker: a11y 11/11 (the allowlist stays empty) and
  every non-visual spec 32/32. The `privacy-notice` spec failed once only
  because no macOS baseline existed. That run wrote one, and the notice
  component is untouched.

## Baselines

The language chip is on every attendee page, so these Linux baselines no longer
match and are removed for CI's `--update-snapshots=missing` to re-take:
landing, discover, privacy, faq, ticket and claim (mobile and desktop); admin
(the dates). The event baselines were already removed in `ec458a89`. Staff,
`privacy-notice` and the `landing-ticket-*` element crops are unaffected.
After the next CI run, commit the PNGs from the `visual-baselines` artifact.

## Browser re-check (2026-10-01, session `event-checkin-da`)

This is a second, independent look at the rebased branch (`91aa28db`, on
`develop` `6604beee`) before the merge. It follows
`docs/web-verification-runbook.md`: `frontend-leptos/build.sh`, then a
credential-free local worker. The worker ran with its own
`CARGO_TARGET_DIR`, `--env-file` empty, and the CI e2e `--var`s. It served the
e2e seed plus `reseed-kv` (2 events synced). The browser was headless Chrome
at 390x844, DPR 2, in EN and TH (`bethere.lang`). Service workers were
bypassed. The served wasm hash was checked against `dist/` after every
rebuild. No page errors on any page.

**Found and fixed: one regression from item 1.** The "hero CTA in view" signal
started as `true` and was only recomputed on scroll. On a screen short enough
that the hero CTA starts below the fold, the first screen showed neither the
hero CTA nor the sticky bar until the first scroll. Before this branch, the bar
was always on the first screen. The fix in `public_event/page.rs` moves the
check into one `measure` closure, run on scroll and once after mount through
`request_animation_frame`. It is not visible at 390x844, where the hero CTA is
on the first screen.

Probe results, taken from the DOM (bounding rects, 100 px scroll steps):

| Check | EN | TH |
|---|---|---|
| 1, 390x844 | First view: the hero CTA is visible, and the bar is absent. The bar never shows at any scroll step up to `max_y` 1646; the hero CTA stays in view until `#reserve` does. | The same (`max_y` 1610). |
| 1, 390x600, before the fix | First view: no bar, and the hero CTA is **not** visible (the regression). The bar shows from y=100. | The same. |
| 1, 390x600, after the fix | The bar shows at y=0, 100 and 800-1000, and never while the hero CTA or `#reserve` is visible. It never overlaps a visible WHEN row. | The bar shows at y=0, 100, 800 and 900. The rest is the same. |
| 2 | `.pe-refund-note` is outside `.pe-refund-list` and below its last item. No "PromptPay" text is left in the list. Both ticks sit on the first line of their item (1 and 4 lines). | The same with "พร้อมเพย์" (items of 1 and 3 lines). |
| 3 | The link text is "View Event Page →" and its href is the event link. No `http(s)://` text on the page. | "ดูหน้างาน →". |
| 4 | 0 `[class*=heart]` nodes and no ♥ glyph. | The same. |
| 5 | The footer has one "Built with" line. | One "สร้างด้วย" line. |
| 6 | The chip's top is at 15 px, and the tap target is 44 px high. | The same. |
| 7 | Cards show `22 Jan 2030, 12:00 GMT+7` and 3 more in that format. No raw `YYYY-MM-DD hh:mm:ss UTC±hh:mm`. | `22 ม.ค. 2573 12:00 GMT+7` (Buddhist-era year, from the shared formatter). |

The screenshots (not committed; fixture data only) are in
`/tmp/ec-qa-probe/out` (before the fix) and `/tmp/ec-qa-probe/out-fix`. The
probes are `/tmp/ec-qa-probe/items12.mjs` and `items37.mjs`. The before/after
run on item 1 is the check that the probe can fail.

Gates after the fix: frontend `cargo fmt --check` passes; clippy
`--target wasm32-unknown-unknown -D warnings` passes for the default build and
`--features staff`. `cargo test --locked` passes 316 tests in 39 binaries.
Playwright a11y is 11/11 and the allowlist is still `{}`. The baseline set is
unchanged by the fix: the event baselines were already removed, and the
language switch renders only on attendee paths (`is_attendee_path`), so the
staff baselines stay.
