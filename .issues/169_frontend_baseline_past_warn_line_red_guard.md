# 169: The frontend size baseline is past the warn line, and `size_budget_guards` is red on develop

**Status:** fixed on develop (2026-09-29, session `event-checkin-b2`), not
pushed, not deployed. The owner chose the recommendation below, on the free
route (no paid plan, no toolchain change). The attendee first load is
1,226,416 B br4 (58.5% of the 2 MiB budget, was 1,994,214), and
`size_budget_guards` is green again. Found 2026-09-29 by `event-checkin-ba`.

## What fails

```
worker/tests/size_budget_guards.rs:247  frontend_budget_thresholds_are_coherent
BASELINE_BYTES (1965134) is already past the warn line — a gate that is amber
on a clean tree is a gate nobody reads
```

The warn line is `CEILING_BYTES × WARN_PCT` = 2,097,152 × 90% = **1,887,436**
bytes br4. `e1161fdd` (the EN+TH switch, Wave 1 Task 2) moved the baseline to
1,965,134 to account for +102 KB of i18n. It did not run the workspace suite
that holds this guard. The frontend gate itself still passes: it prints
"past the warn line … still shippable".

## Measured first load (2026-09-29, `frontend_size_budget.sh`, br4)

- Total: 1,970,397. That is 83 KB over the warn line and 127 KB under the
  ceiling.
- The wasm alone is 1,843,269 (94%). Everything else (JS glue, snippets and 23
  stylesheets) is about 127 KB.

## Options

1. **Shrink the wasm by ≥ 83 KB br.** This is the recommendation.
   Candidates, none of them measured yet:
   - the remaining `t!` sites (see the `leptos-i18n-traps` memory: `t!` per
     site bloats and `locale::tr` cut it);
   - route-level code splitting for admin, scanner and dashboard, which an
     attendee never runs.
2. **Raise `CEILING_BYTES`.** `.size-budget` says this is allowed but must not
   be quiet. It is a product call: 2 MiB ≈ 5.6 s at 3 Mbps.
   **Owner decision.**
3. Raise `WARN_PCT` or edit the guard. **Rejected:** that silences the gate
   instead of answering it.

## Why it matters now

The GOAT-hardening package adds attendee UI (P2-a inline QR, P2-b meta rows,
P2-d cookie banner, P3-a FAQ). Each addition moves the first load toward the
2 MiB fail line, so the budget is now the binding constraint for that work.
P1-1 (landing cuts) and P3-c (jsQR lazy-load) help only a little: jsQR is not
in the first-load set that the gate measures.

## Update 2026-09-29, later: measured where the wasm goes

**Current state:** first load 1,989,103 B br4. The GOAT package added
+18.7 KB (Turnstile, inline QR, meta rows, privacy notice, consent, FAQ,
cards), and P1-1 cut −7 KB. The warn line needs −101.7 KB.

**wasm-opt is not the answer.** `frontend-leptos/optimize-wasm.sh` records
that `-O2` saves 8.9 KB br4 and that `-Oz` makes the transfer worse.

**Attribution:**
- **Method:** a release build with symbol names kept
  (`CARGO_PROFILE_RELEASE_STRIP=false`, a separate target dir), then
  `twiggy top`, with items grouped by the frontend module they name.
- **Units:** raw bytes, before wasm-bindgen and brotli.

| Bucket | Raw bytes | Share |
|---|---|---|
| `pages::*` render code | 3,185,384 | 60.1% |
| `api::*` (types, serde) | 584,356 | 11.0% |
| `.rodata` (strings: both catalogs, adventure levels, …) | 559,695 | 10.6% |
| reactive_graph / tachys / std / serde | ≈ 0.5 MB | ≈ 10% |

**Pages only staff or admins can open:** 1,531,084 B, 48% of page code and
about 29% of the attributed wasm. The largest are:

| Page | Raw bytes |
|---|---|
| `event_form` | 272,722 |
| `admin` | 224,110 |
| `campaigns_page` | 179,616 |
| `scanner` | 112,969 |
| `admin_deposit` | 110,796 |
| `events_page` | 80,392 |
| `escrow_init` | 61,506 |
| `form_builder` | 59,654 |
| `admin_escrow` | 57,016 |

`quiz_editor` (95,394) is organizer-only too, but was not counted above. So
every attendee downloads the whole admin app. Assuming it compresses like
the rest, that share is on the order of 0.5 MB br4, about 5× the gap.

## Recommendation

Split the staff/admin routes out of the attendee bundle. That clears the
warn line with room to spare, and it helps the ticket page on venue data,
which is the moment the budget exists for. It is an architectural decision
(owner):
- (a) a second Trunk app for `/admin`, `/staff` and `/dashboard` with its
  own `index.html`, sharing `domain` + `api`; or
- (b) a toolchain that supports wasm splitting (Leptos `#[lazy]` routes via
  cargo-leptos `--split`; Trunk has no equivalent today).

Until then the choices are to raise `CEILING_BYTES` (an owner call; see
Options) or to leave develop red.

## Resolution (2026-09-29, `event-checkin-b2`)

Owner call: take the recommendation, do not take a paid route. Picked (a),
built as one crate with two Trunk builds rather than a second crate:

- **`staff` cargo feature** (`frontend-leptos/Cargo.toml`, off by default).
  `src/staff_routes.rs` holds the five staff route views. With `staff` they
  wrap the real pages in `ProtectedRoute`; without it they all resolve to
  `StaffShellHandoff`, so the linker drops the scanner, admin and organizer
  pages from the attendee wasm. The route list in `lib.rs` is unchanged.
- **`build.sh`** builds `--features staff` into `dist-staff/`, then the
  attendee shell into `dist/`, and merges the staff build in as
  `dist/staff-app.html`. Asset names are content-hashed, so both wasm files
  sit side by side; both get the q11 `.br` sibling. The SW cache version
  hashes both shells.
- **`_redirects`** (free, Workers static assets proxying) rewrites `/staff`,
  `/admin`, `/dashboard/live`, `/events/:id/summary` and
  `/events/:id/pr-pack` to `/staff-app` with status 200. The query string
  survives (checked `/admin?token=abc` locally). `tests/staff_shell_split.rs`
  pins `_redirects` to `STAFF_PATHS` and to the `lib.rs` routes.
- **Client-side navigation** from an attendee page to a staff path (a
  `<A href="/admin">`) makes a full page load of the router's target URL, so
  the edge hands over the staff shell. If the attendee shell *booted* on a
  staff path (no `_redirects`, or the SW offline fallback), it shows "The
  staff app could not be loaded" instead of looping.
- **Deploy/CI:** CI's e2e job builds with `build.sh` (a bare `trunk build`
  has no staff shell); CI also runs clippy with `--features staff`.
  `deploy.sh` fails the content-type check if `/admin` does not reference the
  staff shell's bundle. The PUT-API fallback cannot carry `_redirects` (same
  as `_headers`, #057); it now says so in its warnings, and staff pages then
  show the "could not be loaded" text.

### Measured (local build, br4 unless noted)

| | Before | After |
|---|---|---|
| Attendee first load (gate) | 1,994,214 | 1,226,416 |
| Attendee wasm, q11 as served | ≈ 1,459 K | 874,268 |
| Staff wasm, q11 as served | — | 1,459,276 |

The staff shell costs what the whole app cost before; only staff and
organizers download it.

### Checked

- Frontend: `cargo test` (incl. 3 new tests), wasm32 clippy `-D warnings`
  for both feature sets, `fmt --check`.
- Worker guards: `size_budget_guards`, `security_headers_parity`,
  `vendored_jsqr_integrity`, `precompressed_asset`. Fallback unit tests 40/40.
  ShellCheck gate clean.
- Local worker (`wrangler dev`): each staff path returns the staff shell,
  `/`, `/ticket/*` and `/events/:slug/recap` the attendee shell; both wasm
  files come back `application/wasm` + `br`.
- Playwright 51/51, including the admin and staff visual snapshots.
- Headless probe: `/privacy` → click `<a href="/admin?event_id=e2e#tab">`
  → the staff wasm loads, and the staff app's guard sends the signed-out
  probe to `/login?next=/admin`.

### Not checked

- `_redirects` on the real edge (staging). The deploy content-type check
  now covers `/admin`; run a staging deploy before prod.
- The "could not be loaded" path in a browser (needs a misrouted edge).

### Follow-ups (not done here)

- ~~The attendee `index.html` still links the staff stylesheets.~~ Done in
  the follow-up below.

### Follow-up: staff-only stylesheets (2026-09-29, session `event-checkin-1b`)

The attendee shell linked all 23 sheets, render-blocking. 839 rules in the
eight staff-flavoured sheets were sorted by one test: a rule moves when every
selector in its list names a class that the attendee wasm (plus `index.html`
and `js/`) never contains, because such a selector cannot match there. No
class in `src/` is assembled from fragments (`css_class_audit.rs` records that
invariant), so the byte search is sound.

- Moved rules sit in `styles/style-NN-*.staff.css` beside the sheet they came
  from; `style-06-admin` (quiz editor) and `style-23-admin-feedback` moved
  whole. `index.html` links only the attendee sheets.
- `staff_shell_html.py` writes `staff-shell.html` (index.html with every
  sheet, sorted) as the staff build's Trunk target. A `.staff.css` file sorts
  right after its parent, so the staff shell keeps the cascade position of
  every rule, except that staff rules now come after the attendee rules of
  the same file. A scan for such a pair that could hit one element (shared
  properties, subject classes that occur together in a `class` literal)
  found none.
- Rule multiset per file checked equal before/after (839 rules, 0 lost).
- Guards: `tests/staff_shell_split.rs` pins index.html's list to the sorted
  non-staff files; `scripts/verify/staff_css_fence.py` (build.sh + CI, with a
  self-test) fails when the attendee wasm uses a class only a `.staff.css`
  sheet styles.
- `bash serve.sh --staff` serves the staff shell in the fast edit loop.
- `externalize_inline_scripts.py` now processes every `*.html` in the
  staging dir, because Trunk names the staff build's page after its target.

| | Before | After |
|---|---|---|
| Attendee first load (gate, br4) | 1,226,416 | 1,211,646 (57.8%) |
| Stylesheets linked by the attendee shell | 23 | 21 |
| Stylesheets linked by the staff shell | 23 | 29 (same rules) |

Checked: `build.sh` exit 0 with the fence green (534 staff selectors); fence
A/B with `.claim-step.active` appended to a staff sheet → exit 1, reverted →
0; frontend `cargo test` 34 binaries green, `fmt --check`; ShellCheck gate;
local worker: `/` links no `.staff.css`, `/admin` `/staff` `/dashboard/live`
link them, served `text/css` + immutable; Playwright 51/51 including the
admin and staff pixel snapshots. Not pixel-checked: `/dashboard/live` and
the admin feedback tab (no snapshot exists); their rules kept their cascade
position (see above).
- `worker/src/lib.rs` `INDEX_HTML` fallback still embeds the attendee shell;
  it only answers when assets do not, so it was left alone.
