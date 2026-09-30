# 171: Light text on the brand accent and warning fills fails WCAG AA

**Status:** deployed to prod `438c392d` (2026-09-30, `deploy/production/20260930T013304Z`, version `e2d7ce81`, session `event-checkin-b5`): the fix ships in `c93080eb`. Was: fixed on develop (2026-09-29, session `event-checkin-3e`). Owner picked option 1 (dark text on small fills). Found 2026-09-29 by the axe gate, session `event-checkin-ba`.

## Found by the gate

| Where | Element | Colours | Ratio (AA needs 4.5) |
|---|---|---|---|
| admin, staff | active header pill `.header-nav-label` | `#f4f0e4` on `--accent` `#ff5c39` | 2.69 |
| landing | active audience tab `.landing-persona-btn--active` | `#fff` on `--accent` | 3.06 |
| ticket | "Pay Deposit Now" `.ticket-action-btn` | `#fff` on `--warning` `#f59e0b` | 2.14 |

axe flags only small text here. Large bold CTAs need only 3:1, which the
accent passes, so the landing hero button is not listed. The ticket CTA is
the one that matters most: it is the attendee's next action.

## Options

1. **Dark text on the fill.** Keep the fills and use `--bg-primary`
   `#12152a` for the text: 5.87 on accent, 8.38 on warning. The brand colour
   stays the same, and the button reads as a light "chip".
2. **Darker fills, white text.** `#d4471f` gives 4.45 (just short) and
   `#b45309` on warning gives 5.02. The poppy accent then looks dimmer.
3. **Leave it.** Keep the allowlist entries and accept the AA miss.

**Recommendation:** option 1, for small-text fills only (pills, tabs,
`btn-sm`), leaving the large hero CTA as it is. This is not applied, because
it is a visible brand change.

## Already fixed in the same pass (not owner-gated)

- `--text-muted` `#7a7f96` → `#8e93aa`. It measured 3.48–4.27 on the card
  surfaces and now gives ≥ 4.53 on every surface up to `--bg-hover`, still
  below `--text-secondary`.
- The `.search-kbd-hint` `opacity: 0.5` (1.91:1) and the
  `.admin-sidebar-group-label` `opacity: 0.7` (3.4:1) were removed.
- `.ticket-ticker-step--active` now uses `--accent-hover` (it was 4.23:1 on
  its tint).
- The icon-only mobile hamburger and the header sign-out button now have
  accessible names.

## Fix (2026-09-29, owner decision: option 1)

- The active nav pill (`nav a[aria-current="page"], nav a.active`) and every
  `.ticket-action-btn` now use `--bg-primary` text. The ticket button got it
  on all its fills, not only the flagged warning one: white measured
  2.15–3.68:1 on accent/warning/info/success, dark measures 4.90–8.38:1.
- The landing persona tab no longer exists (P1-1 rebuilt the landing).
- `e2e/a11y-allowlist.json` is empty; the a11y specs pass without it.
- Large CTAs (hero, sticky bar) keep light text, as recommended.
