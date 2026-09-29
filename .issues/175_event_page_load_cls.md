# 175: The public event page has a load-time layout shift of ~0.95 on about half of loads

**Status:** fixed on develop (2026-09-29, session `event-checkin-42`; measured against `wrangler dev`, not yet on prod). Originally measured 2026-09-29 by session `event-checkin-ba`. Found while proving that the P2-d privacy notice adds no shift.

## Measurement

`/e/e2e-builders-night` at 390×844 in headless Chrome. The service worker was
bypassed. A `layout-shift` observer ran from the start of the document
(`buffered: true`), and each sample waited 1.5 s after `networkidle2`. The
probe is `/tmp/ec-ba/cls_probe.mjs`.

| Privacy notice | 0.0060 | 0.1056 | ~0.95 |
|---|---|---|---|
| shown (10 loads) | 1 | 2 | 7 |
| dismissed (10 loads) | 2 | 3 | 5 |

Both columns have the same distribution, so the notice is not the cause. The
largest entry (0.948) names these sources: the page's decorative background
(`pe-bg-aurora`, `pe-bg-orb-1/2/3`) and `pe-footer`, moving from y = 44 / 252
/ 715 / 750 to y = 0. That looks like the page shell rendering once and then
again after the event data lands, not like one late element. "Good" CLS is
≤ 0.1.

## Next step

- Record a performance trace (`performance_start_trace` with the web-perf
  skill) on a cold load.
- Find which render swaps the shell: the loading state against the loaded
  state in `public_event/page.rs`.
- Fix it by reserving the loaded layout, or by keeping the background layer
  out of the flow (`position: fixed; inset: 0`).
- Re-run the 20-sample probe. The pass bar is every sample ≤ 0.1.

## Cause (2026-09-29)

Three percentage-positioned boxes followed the height of `.center-page`.
When the loaded event (2,388 px tall at 390 px) replaced the skeleton, the
page grew and each one moved:

1. `.pe-bg-layer` was `position: absolute; inset: 0`, so the orbs
   (`top: 40%`, `bottom: -15%`, …) and the aurora (`inset: -25%`) were placed
   in % of the page. This was the 0.948 entry. The aurora's *visible* rect did
   not change (44 → 44, h800); its layout box did, and that is enough to count
   as a layout shift.
2. `.pe-skeleton` was shorter than the viewport, so `.pe-footer` sat at
   y = 750 in view and was pushed out.
3. `.center-page::before` (the ambient glow, on every centered page) used
   `top: -10%`. That was the residual 0.05 once 1 and 2 were fixed.

## Fix

- `.pe-bg-anim > .pe-bg-layer` is `position: fixed`: sized to the viewport,
  independent of content (`style-01-core.css`).
- `.pe-skeleton { min-height: 100vh }`, so the footer starts below the fold
  (`style-19-deposit.css`).
- `.center-page::before` uses `top: -10vh` instead of `-10%`
  (`style-01-core.css`). This applies to every `.center-page` route.

## Verification

Probe: `/tmp/ec-ba/cls_probe2.mjs` (same method as above, notice dismissed).
It now throws if the event did not render, so an empty or broken page cannot
read as CLS 0.

| Build | Loads | CLS values | Over 0.1 |
|---|---|---|---|
| before (`fae433ab`) | 6 | 0.1056 – 1.0467 | 6 |
| 1 + 2 only | 20 | 0.0062 – 0.0537 | 0 |
| 1 + 2 + 3 | 20 | 0.0000 ×11, 0.0002 ×9 | 0 |

Checked at 390×844 in a real render: the background layer stays 0/844 at the
top and at the bottom of the scroll, and there is no horizontal overflow. The
Playwright suite passed locally 51/51 against the rebuilt dist.

Two measurement traps met on the way. Both produce a false clean:
- Patching a hashed CSS file in `dist/` makes the browser block it, because
  `index.html` pins every stylesheet with SRI `integrity`.
- After a rebuild, `wrangler dev` keeps its old asset manifest and answers the
  new hashed files with `index.html` (strict MIME refuses the CSS, and SRI
  blocks the JS). The page is blank and CLS reads 0.0000. Restart wrangler
  after every build.

Not yet measured on prod. Re-run the probe against staging after the next
deploy.

Pre-existing, fixed on develop 2026-09-29 as a follow-up: at 390 px the sticky
"Reserve Your Spot" bar covered the "Powered by BeThere" footer at the very
bottom of the scroll. The `.pe-sticky-spacer` sat inside the loaded-event
block, but the footer is in the page shell after it. The spacer is gone; the
footer gets `padding-bottom: 4.5rem + safe-area` via
`.pe-bg-anim:has(.pe-sticky-cta)` (≤640 px), so the room exists only while
the bar renders. Probe `/tmp/ec-ba/sticky_probe.mjs`: footer text ends at
728 px, bar top 789 px (390×844); desktop and no-bar padding 0; Playwright
51/51.
