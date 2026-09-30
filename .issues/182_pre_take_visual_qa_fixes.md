# 182: Pre-take visual QA fixes (7 on-camera items from the 2026-09-29 baselines)

**Status:** in progress: fixed on `feature/qa-take-fixes` (2026-10-01, session `event-checkin-b5`; rebased onto develop `6604beee` by `event-checkin-da`), not yet merged to develop. Not pushed. Closes once the owner merges it and CI re-takes the baselines. The Linux baselines for the changed pages are removed so CI can re-take them.

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
