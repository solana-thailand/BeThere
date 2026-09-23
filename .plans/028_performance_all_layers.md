# 028 — Performance, all layers (2026-09-23 → )

**Ask (owner, 2026-09-23):** improve system performance in every respect.
**Constraint that ranks everything:** the Cloudflare **free plan** — 10 ms CPU
per request ([[free-plan-cpu-cap-is-binding]]), **1,000 KV writes/day**,
100k KV reads/day, 50 subrequests/request. RTM#6 is 2026-09-27; prod deploys
are owner-gated, so everything here lands on `develop` + staging first.

Sources: two read-only audits (worker hot paths, frontend runtime) run this
session, each finding re-checked against the code before it was acted on.
Two audit recommendations were **rejected after measurement** — see §4.

## 1. Done (on `develop`, verified on staging where observable)

| # | Layer | Change | Evidence |
|---|---|---|---|
| D1 | transfer | wasm served pre-compressed at brotli 11 via Worker negotiation (`7ed1c07`) | staging: 1,675,993 → **1,291,073 bytes (−23 %)**; br/gzip/identity byte-identical; 304 works; app boots in Chrome. `.issues/135` §9 |
| D2 | deploy safety | `deploy.sh` smoke test now checks `/api/health` → JSON and the wasm → `application/wasm` (+ warn if not `br`) | caught class: `run_worker_first` array without `/api/*` → SPA swallowed the API on staging for ~6 min |
| D3 | KV writes | audit log: D1 insert is primary; the KV array is written only if D1 is unbound or the insert fails (`audit_store.rs::append_d1`) — covers all ~40 callers | removes 1 KV read-modify-write of a ≤500-entry array per check-in/undo/deposit action. Readers were already D1-first |
| D4 | KV writes + bug | ticket QR cache: value now `"{url}\n{image}"`, validated on read; TTL 1 h → 7 d | was 1 KV write per polling attendee per hour; also served a stale QR for up to 1 h after the lazy `qr_url` backfill |
| D5 | attendee phones | ticket polling keyed on a `Memo<Option<PollingTier>>`, not on `state`; ticks skipped while the tab is hidden | before: every poll re-ran the effect, restarting the 5-min tier-1 cap, so it never fired (10 s polling forever) and leaked 2 closures per poll. Staging check in §3 |
| D6 | door reliability | scanner: camera prompt no longer waits on the CDN; jsQR loaded only without `BarcodeDetector`; dead QRious load removed | a slow/blocked jsdelivr used to fail the whole scanner, even on Android where jsQR is never used |
| D7 | door CPU (iOS) | jsQR loop: canvas resized only on size change, `willReadFrequently`, `inversionAttempts: "dontInvert"` (both QR generators draw dark-on-light) | removes per-frame canvas realloc + GPU readback + the second inverted decode pass |
| D9 | door latency | check-in/undo: the Sheets column mapping is resolved inside the `wait_until` task, not before the response (`sheets::column_mapping_or_hardcoded`) | −1 KV read (a Sheets header fetch on a miss) on every scan's critical path |
| D10 | attendee phones | ticket poll skips `set_state` when the payload is byte-identical | staging: the child `event-series` fetch went from once per poll to once per page |
| D11 | security trade-off | jsQR now loaded as `dist/jsQR.js` with SRI, not the on-the-fly `jsQR.min.js` (`.plans/029`) | **+11 KB brotli** (42.0 → 53.3 KB), paid only without `BarcodeDetector` (iOS) and loaded in parallel with the camera prompt. F10 removes it |
| D8 | stability | `try_get()` in the scanner poll + undo loops; adventure timer cleared on unmount and reads via `try_with` | `.get()` on a disposed signal panics → wasm trap under `panic = "abort"` after client-side navigation |

## 2. Queued — ranked by (impact × frequency) / risk

### Worker
- [ ] **W1 Staff cache TTL 60 s → 600 s + in-isolate cache** (`sheets/mod.rs:38`). ~60 Sheets fetches + KV writes/hour during an event. Trade: revocation latency = TTL. Low risk, but it is an access-control latency choice — note it in the commit.
- [ ] **W2 Per-event staff N+1** (`auth.rs:241-275`): N sequential `get_event_config` KV reads on every protected request for volunteers only in `staff_emails`. Add `staff_emails` to `EventMeta` or `join_all`.
- [ ] **W3 Counting via full attendee list** (6 sites; `public_event.rs:172` on every landing view, `signup.rs` twice per registration). Replace with one aggregate. (`ParticipationType::parse` no longer allocates per attendee, 2026-09-24, plan 030 §3; the list fetch is still the cost.) **Medium risk:** the SQL predicate must equal `ParticipationType::parse` (note `db/dashboard.rs:149` `IN_PERSON_PREDICATE` is narrower). Also: D1 `Ok(empty)` falls back to Sheets for new events → a Google subrequest per public view.
- [ ] **W4 Public GET edge cache** (Cache API, 30–60 s) for `/public/events` and `/public/event/{slug}`; push the `/public/events` filter into SQL (no WHERE, no LIMIT today). **Also a correctness bug:** private-event responses get `public, max-age=120` (`handlers/mod.rs:55-72`). **Correctness half fixed on develop 2026-09-24:** the public layers (`middleware/cache.rs::with_public_cache`) now cache only 2xx responses without a handler-set `Cache-Control`, set `no-store` on errors, and repeat the public policy on a 304; `get_public_event` sets `private, no-store` for private events; `worker/tests/public_cache_policy.rs` (mutant: the old unconditional insert → 2 red). Found alongside it: `.issues/149` (the event-series endpoint lists private and draft events). The perf half (Cache API, SQL filter) is still open.
- [ ] **W5 Credit-ledger release** runs an `INSERT…SELECT` before every balance read (twice per registration). Once per request + indexes `credit_ledger(reason, event_id, email)`, `credit_ledger(organization_id, currency)`. Migration → after RTM#6 (numbering: 0048 is reserved, `.issues/127`).
- [ ] **W6 Claim lookup** reads the same attendee row 3–4× (`claim/mint/lookup.rs`); quiz/lock/profile reads are independent → `join!`.
- [ ] **W7 Unknown ticket id → full Sheets fetch, unauthenticated, no rate limit** (`sheets/mod.rs:596`). 404 on D1 `Ok(None)` + rate-limit `/api/public/ticket`. Low risk *if* D1 is complete — [[d1-first-not-sheets-first]] says it is for attendee reads; verify first.
- [ ] W9 Admin roster: 3 independent awaits → `join!`; `recent_check_ins` unbounded.
- [ ] W10 Dashboard (2.5 s poll) resolves the event twice, D1 `SELECT *` incl. `form_config`.
- [ ] W11 `resolve_event_by_slug` parses the whole KV index per request.
- [ ] W12 HMAC `importKey` per request → cache the key.

### Frontend
- [x] **F1 Service worker asset cache never prunes** — **closed negative (2026-09-24).** The premise is false: `build.sh::bump_sw_version` sets `CACHE_VERSION` to a hash of `index.html` on every build (prod and staging both serve a hashed version), and `activate` deletes every cache not named for it, so a deploy's first activation purges the previous build. The real gap was that the purge rested on an unchecked `sed` (exit 0 on no match); `bump_sw_version` now fails the build if the bump didn't land (mutant `var`→`let` → exit 1).
- [x] **F2 Snippets fetched `no-store` every visit** — done (2026-09-24): `networkFirst(req, cacheMode)`; snippets pass `no-cache`, navigations keep `no-store`. Headless Chrome A/B over 3 loads: old SW 33× 200, new SW 11× 200 then 22× 304; app boots, 0 SRI errors. Staging edge sends an ETag and answers `If-None-Match` with 304.
- [ ] F3 Scan-to-result latency: two stacked 300 ms polls (JS detect + Rust poll). Callback instead of polling: −150–400 ms per scan.
- [ ] F4 Scanner mount fetches `/api/events/{id}` twice and pages through every event.
- [ ] F5 Public event countdown interval never cleared (`on_cleanup` after `.await` has no owner).
- [ ] F6 Admin roster rebuilds every row per keystroke/checkbox; no debounce.
- [ ] F7 Adventure grid re-created per move; ~20 `GameState` clones.
- [ ] F8 Leaked global keydown listeners (`admin.rs:563`, `events_page.rs:78`); `SessionTimer` loops stack.
- [ ] F10 Self-host jsQR (same origin, precompressed like the wasm): wins back D11's 11 KB, removes the venue-Wi-Fi CDN dependency, and lets the CSP drop `cdn.jsdelivr.net`.
- [ ] F9 CSS: 22 unminified render-blocking sheets + 5 Inter weights. ~0 effect on first paint while the wasm dominates ([[measure-at-the-compression-served]]); hygiene.

### Measurement
- [ ] **M1 `.plans/010` P0.2 — CPU per hot path** (check-in, claim, deposit confirm, refund). Now possible unattended on staging: seed via `/escrow/init` + `confirm-init` ([[devnet-e2e-run-recipe]]) and read `cpuTime` from `wrangler tail --env staging`. Needed before W3/W4 claim any CPU number. Record each rung under `.benchmarks/` (rules: `.benchmarks/README.md`, checked by `scripts/verify/bench_records.py`).

## 3. Verification log

- D1/D2: see `.issues/135` §9.
- D3–D8: `cargo clippy -D warnings` (worker host, frontend wasm32) exit 0; worker 586 tests, frontend 226 tests green; deployed to staging.
- D5 on staging: fixture `perf-poll-fixture-1` on `e2e-test-event-1790173781`
  (tier 1, no deposit), page left open. Console: `tier 1 polling expired after
  300s` after 30 polls, **no further requests** a minute later (before: no cap
  ever fired). D10: after redeploy, 1 request per poll (was 2).
- Tier change still propagates: fixture marked checked-in in staging D1 → next
  poll rendered it. That exposed **`.issues/142`** (unrelated, live in prod): the
  checked-in view's credit chip 401s and bounces signed-out attendees to /login.
- Fixture deleted afterwards.
- Not exercised live: scanner (needs a staff session), adventure timer (needs
  a claim token), hidden-tab skip (same pattern as `dashboard_live.rs:152`).

## 4. Rejected after measurement

- **"Drop the QR KV cache, generation is cheap."** Measured `generate_qr_base64` in `--release`: **0.511 ms native per call** → ~1–1.5 ms in wasm at the edge, on every ticket poll, against a 10 ms cap. That spends the binding budget to save a non-binding one. D4 fixes the actual defects (stale key, 1 h TTL) instead.
- **Stripping `console_log` (−19 KB) / `wasm-opt -O2` (−8.9 KB)** — unchanged from `.issues/135` §6; small next to D1's 385 KB.

## 5. Not doing before RTM#6

W3, W4, W5 (schema/semantics changes on paths RTM#6 exercises) — after 2026-09-27, each with a staging rehearsal.
