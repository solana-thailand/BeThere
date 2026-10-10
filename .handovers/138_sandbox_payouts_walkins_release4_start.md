# Handover 138 — Sandbox, credit payouts, walk-ins, staff shell, Release 4 start (2026-10-09 → 10)

Written by sessions `event-checkin-ac` → `event-checkin-d3` (one conversation,
renamed by the harness). Everything below was checked against GitHub,
`wrangler deployments list` or a probe on the day; re-check before building on
it (`issue_ledger.py`, the repro in each issue).

## 0. TL;DR

| Item | State |
|---|---|
| Prod | `07ea1885` at `main` `9c3ed7e4` (100 %): sandbox code (off), organizer credit payout, payout history + slip-link fix, walk-ins in-person, staff shell hand-off |
| Staging | `5598ef51` at develop `2b8614aa`: the `/sandbox` the owner practises on; **do not redeploy before the 12 Oct hackathon** (owner) |
| Prod freeze | owner, 2026-10-10: no prod (or staging) deploy until after 12 Oct |
| On develop, not deployed | R4.0 site routes (pull 184), R4.7 facts + per-event payers (pull 185), CI docs-only skip + pre-push hook (pull 186), phone nav (pull 187), sandbox own wallet (pull 188) |
| Draft, waiting | PR 172 (`.issues/163`, scoped credit release): after 12 Oct, with the admin-path rehearsal (§4) |
| Prod D1 backups | `~/bethere-backups/backup-prod-20261009-1427.sql`, `…-20261009-1736.sql`, `…-20261010-0005.sql` (600, PII, outside git) |

## 1. What shipped to prod (three owner-gated releases)

1. **Release pull 174 (`f23284cd`, 2026-10-09):** organizer pays held credit
   back to the deposit account without a request (`.issues/192`, pull 171 incl.
   the attendee's "Credit Paid Back" card); devnet `/sandbox` (plan 042 0.4,
   pull 173), which is **off on prod** by design (DEV_MODE 0, no keys).
2. **Release pull 178 (`8064041f`):** "Paid out" list on the Held as Credit
   tab (`GET /api/deposit/credit-payouts`), payout slip links fixed (they were
   `/api/credit-payouts/…`, a 404; now `/api/storage/credit-payouts/…`), payout
   amount prefilled, the button says what is missing (pull 177).
3. **Release pull 182 (`07ea1885`):** walk-ins are in-person everywhere
   (`.issues/162`, pull 180); the staff build hands every attendee page to the
   attendee shell, staff first load 97 % → 63.5 % of 2 MiB (pull 181).

Each: staging first, parity gate passed with no `--force`, D1 backup, smoke.
**Prod write smoke is always "untested":** `post_deploy_smoke.sh` falls back to
`dev-token`, which prod refuses (401) — `SMOKE_TOKEN` is the owner's item.

## 2. The sandbox (`/sandbox`, staging only)

- Owner decisions: Worker-held devnet organizer key, Circle-USDC faucet
  wallet, no Turnstile. Secrets `SANDBOX_ORGANIZER_KEY` (pubkey `AULAURJ…qzxH`)
  and `SANDBOX_FAUCET_KEY` (`Fh9m2…KLar`) are set on staging only; the value is
  the whole `solana-keygen` JSON file (`cat file | wrangler secret put …`).
- Off unless DEV_MODE + devnet + both keys parse; rolling 24 h caps on
  `advisory_locks` (one grant per wallet, 50 grants, 200 events); empty faucet
  → 429 with a plain message.
- Recheck: `node scripts/e2e/sandbox_devnet.mjs <staging-url>` (~2.5 min,
  returns the USDC). Passed on staging 2026-10-09 with the owner's keys.
- Pull 188 (develop only): "Which wallet?" — the test wallet or the visitor's
  own devnet wallet, plus faucet.solana.com / faucet.circle.com links.
  **Real-wallet signing has not been tried** (headless has no Phantom): try it
  on staging after 12 Oct with Phantom set to Solana Devnet.

## 3. Release 4 (plan 045) — started early, owner-confirmed 2026-10-10

- R4.0 (pull 184): `/events` (Discover list in the site frame until R4.3),
  `/discover` → `/events`, `/organizers`, `/sponsors`, shared `SiteFrame` and
  doors. Phone nav and the underline current page: pull 187.
- R4.7 (pull 185): ladder + room in `domain::models::facts`;
  `PublicStats.payers_by_event` (public events only). Prod D1 read-only check:
  RTM #4–#6 equal the facts' System rows.
- Next in the plan: R4.2 / R4.1 (landing hall and lit room), then R4.3.
- **Check site pages in light and dark.** CI baselines are light; a dark-only
  local check missed a dark-on-dark heading and an axe contrast failure.

## 4. For the next session — follow-ups

1. After 12 Oct: deploy staging from develop (R4.0, R4.7, pulls 187, 188),
   look at `/`, `/events`, `/organizers`, `/sponsors`, `/sandbox` in both
   themes and on a phone; try the sandbox with a real Phantom on devnet; then
   prod with an owner go and a D1 backup.
2. PR 172 (`.issues/163`): mark ready, merge, deploy staging, rehearse through
   admin paths — walk-in X + `/deposit/thb/admin-upload` (auto-verify, bank
   fields) + `/refund/hold/{id}` on event A; walk-in X on event B (ends in
   ~150 s) + `POST /api/deposit/apply-credit/{id}`; after B ends, a per-person
   read (re-POST apply-credit, payout candidates) and read X's ledger rows on
   staging D1. `dev-token` and wallet sessions cannot spend credit at
   registration by design (`signup.rs` `credit_identity_ok`). On develop
   `.issues/163` still reads "open"; the newer status is on the PR branch.
3. `SMOKE_TOKEN` (owner) so prod writes are smoke-tested.
4. Plan 042 0.6: paste a link into X, Facebook and LINE (needs a signed-in
   person).
5. SG3 left: standing daily event by cron, Turnstile, per-IP faucet cap, live
   tally, `/try` route + `TryBand`, organizer-rent reclaim.
6. Docs that still name `/discover` (it redirects, so nothing breaks):
   `docs/business_flows_event_page.md`, `docs/escrow_contract_surface.md`,
   `docs/claude_tool_calling_brief.md`.

## 5. How to dev / test what changed

- Workspace: `cargo clippy --workspace --locked --all-targets -- -D warnings`,
  `cargo test --workspace --locked`, then the floors
  (`test_count_floor.py --suite workspace <log>`). Python security tests:
  `cd worker/tests/security && python3 -m unittest discover -p 'test_*.py'`.
- Frontend: from `frontend-leptos/`, clippy for wasm32 with and without
  `--features staff`, `cargo test --locked`, `bash build.sh`,
  `scripts/verify/frontend_size_budget.sh --shell attendee|staff`.
- Local worker from a `/tmp` worktree: symlink `worker/node_modules` to the
  main checkout's, use `--env-file` (no real credentials), own
  `CARGO_TARGET_DIR`, apply migrations to the `--persist-to` dir. The public
  devnet RPC blocks workerd (403); use Helius devnet for anything on-chain.
- Visual baselines: delete the affected `-linux.png`, push, let CI write them
  (`--update-snapshots=missing`), download the `visual-baselines` artifact,
  check by eye, commit. Linux baselines only from CI.
- New: `git config core.hooksPath scripts/hooks` (pre-push `cargo fmt
  --check`); docs-only PRs skip the heavy CI jobs (pull 186).

## 6. Gotchas hit

- `gh pr merge` refuses a draft PR; a script that keeps going after that
  deploys develop without the PR (happened: staging `5598ef51`).
- In this shell `grep` is a function wrapping `rg` and zsh does not split an
  unquoted variable: both made a correct CI filter look wrong in a local test.
  Use `/usr/bin/grep` and real newlines.
- `git reset --hard HEAD~N` after a throwaway test commit also drops the work
  committed with it; recover from the reflog (`git checkout <sha> -- paths`).
- Wrangler parses a SQL file's leading `--` comment as a flag: strip comments
  before `d1 execute --command`.
- `sips -c` crops from the centre; it is no tool for "the top of a screenshot".

## 7. Addendum (same day, later): freeze lifted, released

The owner lifted the freeze. Merged PR 172 and the sandbox 429 retry (pull
190); staging `406921ba` (develop `4db13660`): smoke, 20/20 page checks, the
sandbox e2e, and the `.issues/163` rehearsal on staging D1 (hold +500, apply
-500, return 500) all passed. A first sandbox run hit a public-devnet-RPC 429
at the deposit (one test USDC stranded; the faucet holds 19); pull 190 makes
the browser retry with backoff. Release pull 191 → prod `05db3a46` (main
`14365976`), D1 backup `~/bethere-backups/backup-prod-20261010-1118.sql`,
parity passed; prod: `payers_by_event` live (RTM #2 14/14, #4 16/14, #5
14/13, #6 21/20), sandbox off, 20/20 page checks. §4 items 1 and 2 are done;
real-wallet signing in the sandbox is still untested.

## 8. Addendum (same day, later): R4.2 and R4.1 released

- **R4.2, the payers' hall** (pull 193): the hall replaced the hero's ticket
  loop (owner's pick, as in the prototype). Release pull 194 → prod
  `ff8a1a24` (main `ddcced44`), backup `backup-prod-20261010-1246.sql`.
- **R4.1, the lit room** (pull 195): `room/room.js` + `room/rtm6-room.webp`
  (the prototype's data URI decoded, 14.6 KB) are copy-dir, off the first
  load; `landing/room.rs` is the mount bridge and `utils/lazy_script.rs` the
  shared loader (the globe uses it too). The hero is the night room in both
  themes; the hall sits on a card. First load +1,797 B. The prototype's
  lamps (one per watched episode) wait for courses (R4.4). Release pull 197
  → prod `4a74bbc2` (main `bb986e18`), backup
  `backup-prod-20261010-1425.sql`.
- **A stale Worker reached staging** (fixed, gated): the develop and main
  worktrees shared `~/.cargo/target`. After the R4.2 prod build, cargo reused
  that wasm for the R4.1 staging deploy. The Worker compiles
  `dist/index.html` in (`lib.rs` `INDEX_HTML`) for every non-file route, so
  `/` booted while `/events`, `/organizers` and `/sponsors` loaded prod's JS
  hash and 404'd. Every deploy check was green; a browser probe of `/events`
  caught it. Pull 196: `deploy.sh` `check_worker_embeds_dist` refuses the
  upload when the hashed JS `dist/index.html` names is not inside the bundled
  wasm. **Deploy each worktree with its own `CARGO_TARGET_DIR`**, and probe a
  non-`/` route after every deploy. Prod was never affected.
- **First-load budget:** 98,282 B over the 6 Oct baseline; the fail line is
  102,400. The next page (R4.3) will cross it. Reset the baseline at a
  release (precedent: `1488706e` before release 3) or find savings first.
- Probes used (scratch, not in git): `staging_site_probe.mjs` (20 page ×
  theme × viewport checks), `events_probe.mjs` (boot time per route, 4xx),
  `room_probe.mjs` (lit / quiet / reduced motion), `hall_probe*.mjs`.

## 9. Addendum (same day, evening): the site as in the prototype, and subscribe

- **Why:** the owner said the pages looked unfinished. A page-by-page
  comparison with the prototype (`bethere-ux/site/`) confirmed it: the home
  was still the old long landing, `/organizers` only the swimlane, `/events` a
  bare list, no page heads. Plan 045 had underscoped R4.2/R4.5.
- **pull 199:** first-load baselines reset to the R4.1 prod build (+99 KB of
  deliberate growth listed in the commit).
- **pull 200 → prod `755a3750`** (release pull 201): page heads in the still
  room (`pages/site/head.rs`), `/organizers` as in the prototype (story, how,
  the planning tile `pages/site/plan.rs` on `facts::expected_came`, the
  organizer card), the home trimmed to hero → doors → so far + goal, and
  R4.3 `/events` (open events in the head, else the empty state from
  `domain::models::catalogue::cadence`; learn from past events; the deposit
  strip; 90/94). Also: copy markup links may be same-site paths; in-app
  `#id` links scroll (the router used to land at the top); the side dot index
  went.
- **pull 202 (R4.12), release pull 203:** "Email me when it opens".
  Migration 0060 applied on staging and prod (backups
  `backup-staging-20261010-1817.sql`, `backup-prod-20261010-1829.sql`).
  Gmail secrets set on staging, then prod (owner's go). The staging test
  send to bethere.sol@gmail.com went at 12:17 UTC (Gmail accepted it); the
  test event `r4-12-mail-test-staging-delete-after` is archived. The fix
  (pull 204) shipped in release 205, prod `4f43669a`, with the single
  `17 * * * *` trigger.
- **Gotchas:** a stale `workerd` keeps serving an old build after
  `pkill wrangler` (kill the port's listener); the catalogue of past events is
  a hand-kept snapshot (`domain/src/models/catalogue.rs`) until R4.4 gives
  events a series and recording; Turnstile is off on staging.
- **Prod incident, fixed (2026-10-10 ~11:48 UTC, minutes):** the R4.12
  release's `wrangler deploy` uploaded the full version (`ab7717a4`, git
  `79269e71`), then the schedules API answered 400 to the second cron on
  prod. `deploy.sh` read that as the 10013 bug and ran the PUT fallback,
  which replaced the live version with one lacking `_redirects` (staff pages
  got the attendee shell), `_headers`, the rate limiters and placement.
  Rolled back to `ab7717a4` (`wrangler rollback`); verified: staff shell on
  `/admin`, `/staff`, 20/20 page checks. Prod's cron stayed the old daily one.
  Fix PR: one hourly trigger per worker (`src/schedule.rs`, daily jobs in the
  03:xx tick) and `deploy.sh` never falls back after a live upload.
- **Setting a worker secret makes a new deployment** with no `git:` message:
  after `wrangler secret put --env staging`, the staging parity gate refuses
  prod until staging is redeployed with `deploy.sh staging`.
- **Next:** R4.4 (course pages
  and progress), R4.8, R4.9. Owner items unchanged: `SMOKE_TOKEN`, sandbox
  real-wallet signing.

## 10. Addendum (same day, night): R4.13, wrangler, R4.9

- **R4.13** (pull 207, release 208, prod `4baf1431`): held THB credit in the
  `/events` empty state. First real prod write smoke with the owner's token
  (`~/.bethere-smoke-token`, 24 h; read with `$(cat …)`, never printed).
- **wrangler 4.149** (pull 209, release 210, prod `a862d80d`). Each deploying
  worktree now has its own `worker/node_modules` (`pnpm install
  --frozen-lockfile`), not the symlink to the main checkout.
- **R4.9 static home** (pull 211, release 213, prod `b6c59edc`): `/` is served
  by the Worker (`run_worker_first` has `"/"`), `worker/src/home.rs` splices the
  promise, 90/94 and the open events (or the cadence line) into
  `#boot-summary`; 17,288 bytes; staging cpuTime median 1 ms
  (`.benchmarks/008`). Trade-off: every home view is a Worker request.
- **Deploy checks** (pull 212): the Content-Type, staff-shell and
  security-header checks wait about a minute for the edge (20 s gave two false
  reds after the wrangler upgrade).
- R4.8 was already done by `.issues/183`. The owner tested sandbox signing with
  their own wallet (works).
- **Next in plan 045:** R4.4 (course pages and progress), R4.10 (click
  counters), R4.11 (meals not ordered), then SG1/SG4.

