# 175: The public event page has a load-time layout shift of ~0.95 on about half of loads

**Status:** open (measured 2026-09-29 against `wrangler dev`, session `event-checkin-ba`; not measured on prod). Found while proving that the P2-d privacy notice adds no shift.

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
