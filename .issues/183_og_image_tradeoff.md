# 183: OG image: per-event social cards, tradeoff before code

**Status:** in progress (2026-10-08, `event-checkin-8a`). Owner picked
option B on 2026-10-08. Built on `feature/183-og-per-event` (`99447935`
Worker splice + card storage, `0e804e7c` editor draws the card); not merged,
not deployed, not probed with a crawler. Filed 2026-10-01 by
`event-checkin-53`. Tracked in `.plans/039` F4 "OG image".

## Built on `feature/183-og-per-event` (2026-10-08, `event-checkin-8a`)

**Correction to the premise below.** "`/e/{slug}` never reaches the Worker"
was true on 2026-10-01 and stopped being true on 2026-10-06:
`worker/wrangler.toml` now has `not_found_handling = "none"`, so a path that
is not a file falls through to the Worker, which answers with the embedded
shell when `crawl::route_kind` says it is a page (`/e/:slug` is in
`crawl::APP_ROUTES`). `run_worker_first` was therefore **not** touched, and
no marker block was added to `index.html`: the splice finds the tags by
content (`property="og:title"`, …) and falls back to the stock page if any
is missing or doubled. Also since plan 042 0.6 the stock image is
`/og-image.png` 1200×630, not `badge.svg`, so the last fallback in the image
order is that file, not `/api/badge.png`.

What exists on the branch:

- Worker (`worker/src/og_meta.rs` pure, `worker/src/og_page.rs` I/O, hooked
  in `lib.rs` before the stock shell): for `GET /e/{slug}` of an event that
  is not draft/archived/private, read KV-first through
  `resolve_event_by_slug` (the `/api/public/event/{slug}` path) and replace
  `og:title`, `og:description`, `og:url`, `og:image` (+ type, width, height,
  alt) and the `twitter:` twins, HTML-escaped. Same headers as the stock
  shell. Unknown slug or any read error → stock page.
- Image order: raster poster (stored PNG/JPEG, or an https URL ending in
  .png/.jpg/.jpeg; SVG/WebP skipped; width/height tags dropped because the
  size is unknown) → `og/{event_id}.png` → stock. At most three R2 HEADs in
  parallel per view.
- `POST /api/events/{id}/poster?kind=og`: organizer role for that event,
  `image/png`, PNG signature + IHDR exactly 1200×630, 2 MB cap, key
  `og/{resolved event id}.png`, `poster_url` untouched (so no
  `expected_updated_at` race with the editor). `GET /api/storage/og/{id}`
  serves it `image/png`, `public, max-age=3600`.
- Staff editor: after a successful create/update, draws the card on a canvas
  (title wrapped to 3 lines with Thai-safe breaks, date + venue, uploaded or
  generative poster art) and uploads it. Failure is logged, never blocks the
  save. Canvas `web-sys` features are behind the `staff` feature.
- Tests: `worker/tests/og_splice.rs` (14), `domain/tests/og_card.rs` (8),
  `frontend-leptos/tests/og_card_layout.rs` (11).

Not verified yet: the canvas render (only a browser shows it; Thai glyphs
depend on the Anuphan subset loading), the crawler probe on staging with two
events, the platform debuggers, and the per-view cost (`.benchmarks/007`
method). Events never opened in the editor since the merge have no card and
show the stock image until saved once.

## What a crawler sees today (probed on prod, 2026-10-01)

`curl -A "facebookexternalhit/1.1" https://bethere.solana-thailand.workers.dev/discover`
returns the static `frontend-leptos/index.html` head:

- `og:image` = `twitter:image` = `/api/badge.svg`, served as `image/svg+xml`,
  declared 400×400, with `twitter:card = summary_large_image`.
- The same card goes out for every URL, because every path except `/api/*`,
  the wasm and jsQR is served asset-first (`worker/wrangler.toml`
  `run_worker_first`). `/e/{slug}` never reaches the Worker.

Two defects follow:

1. **No image renders.** Facebook, X and LINE do not accept SVG for
   `og:image`, so shared links show a text-only card. This rests on the
   platforms' documented image formats; it has not been re-tested with each
   platform's debugger.
2. **Per-event meta is invisible to crawlers.** `public_event/page.rs:556`
   sets `og:image` (poster, then badge) with `leptos_meta` *after the wasm
   runs*. Crawlers don't run JS, so that code only affects the live DOM.

So the handoff framing ("resvg-wasm vs a client-generated PNG") covers only
half the job. Neither option shows anything until the HTML the Worker returns
for `/e/{slug}` carries the event's own tags.

## Part 1: per-event tags in the HTML (every option needs this)

- Add `"/e/*"` to `run_worker_first`. Keep `"/api/*"` in the list
  (`run-worker-first-array-swallows-api`, `.issues/144`).
- Handler: fetch `index.html` from `ASSETS`, read the event KV-first (the same
  path `/api/public/events/{slug}` uses), and replace one marked block
  (`<!-- og:begin -->…<!-- og:end -->`) with `og:title`, `og:description`,
  `og:url`, `og:image` (+ width/height/alt) and the `twitter:*` twins.
  Escape every value as HTML. Unknown slug → serve the stock page unchanged
  (the SPA shows its own not-found).
- A string splice on a 9.6 KB file costs far below 1 ms. `lol-html` is not
  needed and would cost bundle size. Keep the response headers the asset path
  sends today, so CSP/HSTS/XFO survive (`_headers` `/*` mirrors
  `SECURITY_HEADERS`). The post-deploy smoke must check Content-Type
  (`deploy-smoke-test-content-type`).
- **Cost:** every `/e/{slug}` view becomes a Worker request (one KV read plus
  one ASSETS fetch). That counts against the free-plan request quota, not
  against CPU. Measure it with the `.benchmarks/007` method before calling it
  cheap.
- **Tests:** a worker test that pins the splice (marker present in
  `index.html`, values escaped, fallback on a missing slug), plus a
  `frontend-leptos/tests/` guard that the marker block exists.

## Part 2: where the PNG comes from

| | A. resvg-wasm in the Worker | B. PNG made in the organizer's browser (recommended) |
|---|---|---|
| Worker CPU | A full SVG render at 1200×630 plus PNG encode. Unmeasured, but QR decode of a 1080×1920 image already takes 33 ms against a 10 ms cap (`free-plan-cpu-cap-is-binding`). Only workable as render-once-on-save with overruns tolerated. | None. |
| Worker bundle | resvg + tiny-skia + a font. Text needs an embedded font, and a Thai-capable one is hundreds of KB. Unmeasured against the 3 MiB gzip budget (~1.5 MiB headroom, `worker_size_budget.sh`). | None. |
| Thai text | Only with the embedded font; shaping quality to be proven. | The browser's own fonts (Anuphan is already loaded). |
| Stale image | The Worker can re-render on every update. | Regenerated only when the editor saves. Events made through the API or the seed have none until opened in the editor. Falls back cleanly (below). |
| Trust | Server-made. | Organizer-made. That is the same trust as the poster upload the organizer can already do. |
| New code | Render module, font asset, cache/R2 key. | Canvas draw of the existing `utils/poster.rs` SVG + title, `toBlob`, and upload through the poster route under a new key `og/{event_id}.png`. Needs `web-sys` canvas features in the staff build only. |

**Recommendation: B.** It costs nothing on the constraint that binds (Worker
CPU) and nothing in the Worker bundle, and it gets Thai glyphs right for free.
Revisit A only on a paid plan, and measure it first (a `.benchmarks/` record)
before anyone quotes a number.

**Image order in the Part 1 splice:** a raster poster (png/jpg; skip an
uploaded svg, and skip webp until LINE support is confirmed) → `og/{event_id}.png`
→ `/api/badge.png` (the 512×512 twin that already exists for Crossmint).

## Part 0: quick win, owner's call on timing

Point the static `index.html` tags at `/api/badge.png` and declare 512×512.
Also switch `twitter:card` to `summary`, since the image is square. That
single-file change makes every shared link show an image today, before Parts 1
and 2 exist. It is not demo-flow code, but it needs a deploy, so it is the
owner's call whether it lands before the 6 Oct freeze.

## Verify (when built)

- `curl -A facebookexternalhit/1.1 …/e/<slug>` shows that event's tags. Probe
  two events, so a splice that serves the active event can't read green
  (`ticket-api-defaults-to-active-event`).
- Run the Facebook Sharing Debugger and the LINE share preview on staging;
  both render the image.
- Open `/e/<slug>` at 390×844: the SPA still boots and the CSP is unchanged.
