# CLAUDE.md — agent rules for this repo

An index, not a narrative. Each rule is one line plus a pointer to where the
reasoning lives. If a rule needs a story, put the story in `.issues/` or
`docs/` and link it here.

## Layout

| Path | What | Workspace? |
|---|---|---|
| `domain/` | Shared types + logic (native + wasm32) | yes |
| `worker/` | Cloudflare Worker (`bethere`), D1/KV/R2, migrations | yes |
| `frontend-leptos/` | Leptos WASM SPA, served by the worker | **no** (`exclude`d) |
| `bethere-escrow/` | On-chain escrow program (Quasar) | no |
| `flow-harness/` | Business-flow harness | no |
| `scripts/verify/` | Gates and probes run by CI and by hand | — |
| `.issues/` `.plans/` `.handovers/` `.benchmarks/` | Numbered working docs | — |

Architecture: `docs/architecture.md`. Security posture: `SECURITY.md`,
`docs/iso27001_gap_assessment.md`.

## Before you touch the tree

- Other agent sessions often share this working tree. Check for peers first
  (`ListAgents`); if one is busy here, agree on disjoint file sets before
  editing.
- Work on `develop` or a `feature/*` branch. Pushing, tags and prod deploys
  are the owner's call. Do not do them unless you were told to in this session.

## Commit hygiene

- **Stage named files only.** Use `git add path/a path/b`, never `git add -A`
  or `git add .`. A peer's half-finished edit would ride along in your commit.
- **When a file becomes a directory, stage the deletion too:**
  `git add worker/src/x.rs`. A directory pathspec misses its sibling `.rs`
  file, and CI then fails with E0761.
- **Commit messages:** a conventional subject (`feat|fix|refactor|docs|chore|ci(scope): …`).
  Write the message to a temp file and commit with `git commit -F <file>`;
  never put newlines inside `-m`.
- **Add a trailer to every agent commit:**
  `Session: <session-name>, <unix-epoch>`. The name is the one `ListAgents`
  shows for you. The epoch is `date +%s` at your first commit; reuse it for
  every later commit in the same session, because names get reused. That lets
  a commit be traced back to its session when several run at once.
- **Read-only git:** `git --no-pager log|diff|show`. Commands that may open an
  editor: prefix them with `GIT_EDITOR=true`.
- **Before recording a negative result** ("X does not exist", "no one uses
  Y", "the fix did not work"), run `git fetch` and check the peer's latest
  commits and fixtures. A negative result built on a stale tree is the most
  expensive kind of wrong.
- Never commit `worker/scripts/.preflight-bypass.log`, D1 exports or PII
  dumps. The repo is public.

## Numbered docs (`.issues`, `.plans`, `.handovers`)

- **Allocate numbers, don't pick them:**
  `python3 scripts/verify/numbering_gate.py --next .issues`. It hands out the
  next number under a lock. CI fails on duplicates and on a `.highwater`
  below the highest number on disk.
- **Issues numbered above 144 must open `Status:` with one of these words:**
  open · in progress · fixed on develop · deployed · closed · closed negative
  · parked (name the reopen trigger) · declined. The checker is
  `scripts/verify/issue_ledger.py --vocab`.
- **Before building on an issue's claim,** re-run its repro. Status prose goes
  stale; `issue_ledger.py` is the ledger check.

## Build and test

Heavy gates (full clippy, wasm builds) belong to CI. Run the cheap checks
locally. Two cargo runs on the shared `~/.cargo/target` block each other, so
run one at a time.

```sh
# workspace (domain + worker)
cargo fmt --all -- --check
cargo clippy --workspace --locked --all-targets -- -D warnings
cargo test --workspace --locked

# frontend: outside the workspace, so gate it separately
cd frontend-leptos && cargo fmt --check \
  && cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings \
  && cargo test --locked

# zero-alloc audit (feature-gated, one thread)
cargo test -p event-checkin-domain --locked --features wire,alloc_count \
  --test alloc_count -- --test-threads=1
```

- Tests go in each crate's `tests/` folder.
- **Test-count floors:** CI checks every test binary against
  `scripts/verify/test_floors.json` (`test_count_floor.py`). A binary that
  drops to 0 turns CI red.
  - After adding tests, ratchet the floors up:
    `python3 scripts/verify/test_count_floor.py --suite workspace <log> --update`.
  - Lowering a floor needs `--allow-decrease` and a commit that says why.
- **After splitting a module,** `rg` for the old path as a string literal. The
  path guards in `frontend-leptos/tests/` pin file paths.
- **A frontend change is verified by opening the page,** not by check + clippy
  + a curl 200 (`docs/web-verification-runbook.md`).

## Gates in `scripts/verify/`

Each gate has a `--self-test` (or a CI step) that proves it can fail. A gate
that cannot fail is not a gate.

| Script | Guards |
|---|---|
| `numbering_gate.py` | duplicate doc numbers, `.highwater` drift |
| `issue_ledger.py` | issue Status vocabulary and ledger |
| `test_count_floor.py` | green-zero test binaries |
| `worker_size_budget.sh` / `frontend_size_budget.sh` | bundle size vs. budget |
| `shellcheck_gate.sh` | shell scripts (CI pins ShellCheck v0.11.0) |
| `event_invariants_audit.py` | contradictory D1 columns (`--db` for real data) |
| `pii_log_probe.sh` | PII in logs: leak → 1, missing log → 2 |
| `post_deploy_smoke.sh` | status **and** Content-Type after a deploy |
| `golden_vectors_check.sh` | escrow PDA/ATA fixture vs. the Solana CLI |
| `wasm_leak_scan.sh` | build-host paths (report-only) and secrets in shipped wasm/js |
| `domain_import_fence.py` | `domain`'s wasm32 graph reaching app/platform crates; JS bridge pinned both ways |
| `third_party_licenses.sh` | `THIRD_PARTY_LICENSES.md` drift vs. the lockfiles (licence gate itself: `cargo deny check licenses`) |
| `bench_records.py` | `.benchmarks/` record headers (green gate, interleaved lanes, retractions) and citations that resolve |

## Deploy (owner-gated)

- Runbooks: `docs/staging_deploy_runbook.md`, `docs/gradual_deploy_runbook.md`.
- Build the frontend first (`frontend-leptos/build.sh`); `dist/` is not in git.
- `worker/deploy.sh staging` for staging. `worker/deploy.sh` for prod needs
  an explicit owner go.
- Back up D1 before any prod deploy:
  `npx wrangler d1 export <db> --remote --output backup-$(date +%Y%m%d).sql`.
  Keep the backup out of git.
- `deploy.sh` does **not** apply migrations. Apply them before the code
  deploy, then read the schema back to confirm.
- "Pushed to main" ≠ "deployed". The truth is `npx wrangler deployments list`.
- After a deploy, check write volume on every table a new constraint touches.
  A migration can break code that already writes to the table.

## Data rules

- Never `wrangler d1 execute` an `events` row in prod. The read path checks
  KV first, so the write is invisible. Use `PUT /api/events/{id}` or
  `reseed-kv`.
- `/api/public/ticket/{id}` without `?event_id=` serves the **active** event.
  Per-event probes must pass the id.
- Handlers that take `Extension<Claims>` must sit in the authed router. In
  the public router they return 500, not 401.
- When you add a guard to one state transition, find every other writer of
  the same transition and guard it too.
- Keep `://` out of API error text; the redactor turns it into `[redacted-url]`.

## Code style

- snake_case; real types, not strings; `format!("{var}")`.
- Prefer `match` and early returns. Keep `.rs` files under 1024 lines, and
  use `mod.rs` as an index only.
- No `unsafe`: `frontend-leptos` has `#![forbid(unsafe_code)]`.
- Fix warnings before you commit. Remove code that is truly unused.
