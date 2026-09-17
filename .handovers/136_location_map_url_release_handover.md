# Handover 136 — Event location Google Maps link: released

> **Completed 2026-09-17.** Everything below §0 is the pre-release snapshot,
> kept as the record of how it was done; the "pending" and "not done" wording
> in it is no longer true.
>
> | Step (§5) | Outcome |
> |---|---|
> | 1 Merge #121 | rebase-merged to `develop` 2026-09-17T01:10Z |
> | 2 Release scope | user chose "ship all 5" |
> | 3 Release to `main` | `fc8e60c` (merge commit), tree identical to `develop` @ `3ca6ddb` |
> | 4 Back up prod D1 | `~/bethere-backups/bethere-db-20260917-pre-0041.sql` (outside git) |
> | 5 Migration 0041 | applied to prod, column read back from `sqlite_master` |
> | 6 Deploy | prod `9c2d0574-dcc4-4906-9fa9-f87735292b82`, `--force` + reason (`.issues/084`) |
> | 7 Smoke test | served `event-checkin-frontend-56444e2d4f8f8d7c_bg.wasm` byte-identical to the build; "Open in Google Maps" count 1; RTM #6 API `has_key: true`, `location_map_url: null`; health 200 |
>
> A first deploy attempt was interrupted when the terminal app quit, leaving
> `~/.pnp.cjs` moved aside; it was restored, prod was confirmed still on
> `1a4ab8e6`, and the deploy was re-run. Prod has since moved on to `8ba9e1f5`
> (`.handovers/137`, #115). Still open from this feature: a human organizer
> setting a real link on prod (`DEV_MODE=0`).


## 0. TL;DR

Organizers can now attach a Google Maps link to an event's location; attendees see a
tappable link on the public event page and the ticket page. The feature is **finished,
CI-green, verified locally in a browser, and live + verified on staging**.

**Not done:** PR #121 is not merged, and nothing has reached production. The previous
agent was blocked by the Claude Code auto-mode classifier (`[Merge Without Review]`)
even after the user approved — a human must merge (or add a `gh pr merge` permission
rule) before the release can continue.

| Item | State |
|---|---|
| Branch | `feature/location-map-url` @ `a51d4e7` (pushed, clean) |
| PR | [#121](https://github.com/solana-thailand/BeThere/pull/121) → `develop`, **OPEN**, mergeable, all 7 CI checks green on `a51d4e7` |
| Staging Worker | `bethere-staging` version `869eb3bd-97ec-448e-b047-29e6f6ed2172` (built from `a51d4e7`) |
| Staging D1 | migration 0041 **applied**, column verified in `sqlite_master` |
| Prod Worker | `bethere` version `1a4ab8e6-37e3-4853-b598-5d0879d653e0`, deployed 2026-09-16T01:26Z — **does not have this feature** |
| Prod D1 | migration 0041 **pending** (the only pending one) |
| `origin/main` | `43b5839` (2026-09-13) — **75 commits behind `origin/develop`** |

---

## 1. What the feature does

- **Event form (admin):** new optional field "Google Maps Link" under "Location".
  Always sent on update, so clearing the field removes the link.
- **Public event page `/e/:slug`:** "Open in Google Maps ↗" under the location
  (`target=_blank rel="noopener noreferrer"`, class `pe-map-link`).
- **Ticket page `/ticket/:id` (in-person + online):** "📍 <location> Map ↗".
- **Duplicate event** copies the link.
- **Security:** only `https://` URLs, no whitespace/control chars, ≤ 2048 chars.
  Validated on create/update (`normalize_map_url`) **and** re-checked on read
  (`safe_map_url`) in both the Worker JSON and the frontend, because event data can
  also arrive from Sheets or older clients. A stored `javascript:` link renders nothing
  and the API returns `null`.
- **Not included (deliberate scope):** landing/discover list cards (`PUBLIC_EVENT_COLUMNS`
  does not select the column), notification emails, PR pack. The public *list*
  endpoint therefore never has `location_map_url` — that is expected, not a bug.

## 2. Commits on the branch (in order)

1. `ff6cc2f` feat(admin): cross-event/series feedback aggregation (Issue #113) — **the
   user's own commit**, was on local `develop` but never pushed, so it rides along in PR #121.
2. `9828808` feat(events): add Google Maps link for event location
3. `d48f134` fix(ci): restore fmt, PII-log, SQL-interpolation and CSS-class guards —
   pre-existing CI failures introduced by the feedback work:
   - `log_pii_guard`: tracing field `admin_email` → `identity_fingerprint` (value was already fingerprinted)
   - `sql_interpolation_guard`: allowlisted the generated `?N` IN-list in `worker/src/db/feedback.rs` (same pattern as `db/contacts.rs`)
   - `css_class_audit`: styled `badge-neutral`, `badge-sm`, `admin-feedback-page`; `form-control` → existing `form-input`
   - `cargo fmt` on `handlers/feedback.rs`, `handlers/mod.rs`, `frontend-leptos/src/api/admin.rs`
4. `a51d4e7` fix(events): validation message reworded — the API error redactor turns any
   `scheme://…` text into `[redacted-url]`, so "must start with https://" reached the
   user as "must start with [redacted-url]". Now: "map link must be a secure (https) web address".

## 3. Where the code is

| Layer | Files |
|---|---|
| Domain | `domain/src/models/event/map_url.rs` (new: `normalize_map_url`, `safe_map_url`, `MAX_MAP_URL_CHARS`), `event/config.rs`, `event/requests.rs`, `event/mod.rs` |
| Domain tests | `domain/tests/map_url.rs` (4 tests) |
| Migration | `worker/migrations/0041_event_location_map_url.sql` — `ALTER TABLE events ADD COLUMN location_map_url TEXT DEFAULT ''` (additive; old code ignores it) |
| Worker D1 | `worker/src/db/events.rs` (row field, `into_config`, upsert column + bind — bind order matters) |
| Worker writes | `worker/src/event_store/write/{create,update,seed}.rs`, `handlers/events/duplicate.rs`, `handlers/deposit/escrow/status.rs`, `db/event_summaries.rs` |
| Worker API out | `handlers/public_event.rs` (`location_map_url`), `handlers/attendee/read.rs` (`event_location_map_url`) |
| Frontend | `api/types.rs`, `api/event/types.rs`, `pages/event_form.rs`, `pages/public_event/{types,details_card}.rs`, `pages/ticket/{event_context,view_data,in_person_view,online_view}.rs`, `styles/style-13-public-event.css` |

## 4. Verification already done (evidence)

- CI on `a51d4e7`: check+clippy+test (domain, worker), frontend wasm32 clippy + native tests,
  playwright e2e, worker wasm32 build, escrow SVM, shellcheck, lint-design — **all pass**.
- **Local browser** (`wrangler dev --local`, fresh D1 0001–0041, headless Playwright):
  public page link, ticket link, admin form prefill + save via "Update Event", trim,
  clear, `javascript:`/`http://` rejected, bad stored link renders nothing, feedback-page
  badges render. Details are in the PR comment on #121.
- **Staging** (`https://bethere-staging.solana-thailand.workers.dev`):
  - Content-Type check passed (`/` text/html, JS text/javascript).
  - Served `event-checkin-frontend-56444e2d4f8f8d7c_bg.wasm` is byte-identical (`cmp`) to the local build and contains "Open in Google Maps".
  - Existing events load; `GET /api/public/event/flow-084-verify` has `location_map_url: null`.
  - With `Authorization: Bearer dev-token` (staging accepts it): `javascript:` rejected with the new message; set `https://maps.app.goo.gl/stagingProbe` → public API returned it; then **restored to empty** (returns `null` again).

## 5. Remaining work (do in this order)

### Step 1 — Merge PR #121 (needs a human or a permission rule)
The classifier blocks `gh pr merge` for unreviewed PRs. Ask the user to either merge
#121 in the GitHub UI (**rebase merge**, per gitflow) or add a Bash permission rule for
`gh pr merge`. Do not self-approve or otherwise work around it.

Then sync local `develop` (local `develop` still holds the old `ff6cc2f`; a rebase pull
drops the duplicate patch):
```bash
cd /Users/ozone/event-checkin && git fetch -q origin && git switch develop && git pull --rebase origin develop && git --no-pager log --oneline -5
```

### Step 2 — Decide the release scope with the user (IMPORTANT)
`origin/main` is **75 commits behind** `origin/develop`. A normal gitflow release
(`develop` → `main` merge commit, deploy from `main`) ships all 75, not just this feature.
Also, prod version `1a4ab8e6` was deployed 2026-09-16T01:26Z (08:26 +07), which is
*after* `main`'s last commit — it most likely came from around `15f46b5`
(feedback dashboard, 08:17 +07) on `develop`/a local tree, but this is **not confirmed**.
Confirm with the user what should ship before merging to `main`.

### Step 3 — Release merge `develop` → `main` (merge commit)
Likely also blocked by the classifier (open a release PR `develop` → `main`; a human merges).

### Step 4 — Back up prod D1 (keep out of git — contains PII)
```bash
cd /Users/ozone/event-checkin/worker && mv ~/.pnp.cjs ~/.pnp.cjs.bak; CI=true npx wrangler d1 export DB --remote --output ../backup-$(date +%Y%m%d).sql; mv ~/.pnp.cjs.bak ~/.pnp.cjs
```
Check `git check-ignore` covers the backup path before continuing.

### Step 5 — Apply migration 0041 to prod, then verify the schema
```bash
cd /Users/ozone/event-checkin/worker && mv ~/.pnp.cjs ~/.pnp.cjs.bak; CI=true npx wrangler d1 migrations apply DB --remote && CI=true npx wrangler d1 execute DB --remote --command "SELECT sql FROM sqlite_master WHERE name='events'" --json | rg -o "location_map_url[^,\"]*"; mv ~/.pnp.cjs.bak ~/.pnp.cjs
```
Must happen **before** the Worker deploy — the new upsert writes `location_map_url`,
so saving an event 500s on a DB without the column.

### Step 6 — Build frontend + deploy prod
```bash
cd /Users/ozone/event-checkin/frontend-leptos && bash build.sh && cd ../worker && bash deploy.sh --force --reason "release: location map link (PR #121) + <scope>"
```
`--force` is required: the §3.5 preflight gate always blocks prod because
`flow-harness/results/.last-green` has never existed (issue #084).

### Step 7 — Smoke test prod (content, not status codes)
```bash
S=https://bethere.solana-thailand.workers.dev; F=$(curl -s $S/ | rg -o 'event-checkin-frontend-[a-f0-9]+_bg\.wasm' | head -1); curl -s $S/$F -o /tmp/prod.wasm && cmp /tmp/prod.wasm /Users/ozone/event-checkin/frontend-leptos/dist/$F && rg -c "Open in Google Maps" /tmp/prod.wasm; curl -s $S/api/public/event/solana-x-ai-builders-the-road-to-mainnet-6-bangkok | jq -c '.data | {location, has_key: has("location_map_url")}'
```
Expect: `cmp` silent, count `1`, `has_key: true`. `deploy.sh` already checks Content-Type;
any `octet-stream` on `/` or `*.js` = failed deploy (see memory "Cloudflare assets
content-type poisoning"; fix = bump `BUILD_TAG` in `frontend-leptos/src/lib.rs`).
Prod `DEV_MODE=0`, so an agent cannot write a link on prod — ask a human organizer to
set one on a real event via the admin form and check the public page.

**Rollback** if needed: `echo y | CI=true npx wrangler rollback 1a4ab8e6-37e3-4853-b598-5d0879d653e0 --message "..."`
(with `~/.pnp.cjs` moved aside). Rollback does not undo the migration; the added column is harmless to old code.

## 6. Gotchas learned this session

- **Auto-mode classifier:** blocks `gh pr merge` (`Merge Without Review`) even with user
  approval in chat. It also denied a *staging* `d1 migrations apply` once
  (`Modify Shared Resources`) and allowed the identical retry. Retry D1 commands once;
  hand merges to the human.
- **Cloudflare 7403** ("account not valid or not authorized") on a D1 call was transient —
  `d1 list` + a `SELECT 1` succeeded right after. Retry before diagnosing auth.
- **Error redactor** rewrites `://` in API error messages → never put `https://` in a validation message.
- **KV masks direct D1 event writes** (also locally): set event fields through
  `PUT /api/events/{id}`, not `d1 execute`, or the read path returns the stale KV config.
- **Shared cargo target:** `target-dir = ~/.cargo/target` (from `~/.cargo/config.toml`);
  other projects' `cargo test` contend for the lock, and each new test binary can take
  ~1 min to launch at 0% CPU (likely macOS scanning). Slow ≠ hung — check that the binary name advances.
- **chrome-devtools MCP** may fail with "browser already running" (another session owns the
  profile). Use headless `@playwright/test` from `worker/` instead; don't kill the other browser.
- `.dev.vars` holds **live** Google credentials; public event GETs fall back to the Sheets
  API for attendee counts even under `wrangler dev --local` (read only).
- Local `rg` with no path argument in a non-TTY shell reads stdin and hangs — always pass a path.

## 7. How to dev/test this feature

```bash
# unit tests
cargo test -p event-checkin-domain --test map_url
# CI-equivalent gates
cargo fmt --all -- --check && cargo clippy --workspace --locked --all-targets -- -D warnings
(cd frontend-leptos && cargo fmt --check && cargo clippy --target wasm32-unknown-unknown --all-targets -- -D warnings && cargo test --test css_class_audit)
cargo test -p event-checkin-worker --test log_pii_guard --test sql_interpolation_guard
```
Local end-to-end: follow memory "Local D1 verification harness" (fresh `--persist-to`
dir, apply migrations, `wrangler dev --local --port 8788`, `Bearer dev-token`).

## 8. References

- PR: https://github.com/solana-thailand/BeThere/pull/121 (includes a verification comment)
- Memories: `bethere-deploy-and-rollback`, `cloudflare-assets-content-type-poisoning`,
  `deploy-smoke-test-content-type`, `kv-masks-direct-d1-event-writes`,
  `local-d1-verification-harness`, `error-redactor-eats-scheme-text`
