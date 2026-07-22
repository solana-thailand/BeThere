# Handover 129 — Issue #061 Merge + Prod Migration + Deploy

> Branch: merged `feature/061_thb_hold_frontend` → `develop` (merge commit `4fad6d9`)
> Continues handover 128 (THB hold-deposit idempotency fix). Closes Issue #061.
> Current branch: `develop`. Production Worker `bethere` redeployed (Version `e14a8d11`).

---

## 1. What Happened

This session was a **completion + merge** session, not a build session. Picking up from
handover 128 (where Phase 1 + backend hardening shipped) and the prior session (where Phase 2
admin visibility + Phase 3 exit path shipped), this session:

1. **Reviewed final state** of the in-flight PR #23 (18 commits, `MERGEABLE`, `CLEAN`, both
   CI checks `pass`).
2. **Fixed one stale documentation checkbox** in Issue #061 (commit `5825cb3`).
3. **Merged PR #23** into `develop` with a merge commit (`4fad6d9`), matching the project's
   existing merge-commit convention.
4. **Cleaned up** the feature branch (local + remote + stale remote-tracking refs).
5. **Ran a local-only D1 migration dry-run** first (per the user's initial "don't touch prod"
   constraint) — applying migrations `0022` + `0023` to **local** D1 to prove the SQL was valid
   and data-loss-safe.
6. **User then said "go for it"** — authorizing the prod-touching steps. So this session ALSO:
7. **Applied migrations `0022` + `0023` to remote prod D1** (additive-only, data-loss-safe).
8. **Rebuilt the frontend WASM** (`bash build.sh`) — required because `dist/` was stale
   (built 15:48, source changed 16:04) and gitignored.
9. **Deployed the Worker to production** — Version `e14a8d11-c391-41c9-9bdc-cdccb2b04ef9`,
   live at `https://bethere.solana-thailand.workers.dev`.
10. **Smoke-tested** all new routes on prod (health 200; protected routes 401 = live + auth-gated).
11. **Created this handover doc.**

Production WAS touched (migration + deploy), but **no data was lost** — proven by before/after
row counts: `thb_deposits` 33 → 33, `contacts` 161 → 161 (unchanged).

---

## 2. The decision that drove this session

The user's constraint evolved mid-session: **"do whatever that doesn't affect real data on
production — I just fear data loss"** → then **"go for it"** after the local dry-run proved
the migrations were additive-only and safe. The final choices:

- **Merge strategy:** merge commit (not squash) to match project convention and preserve the
  granular Phase 1/2/3 commit history for audit.
- **Branch cleanup:** `--delete-branch` (gitflow standard — work is fully preserved in `develop`).
- **Local dry-run FIRST:** `--local` migration apply before any `--remote`, to prove the SQL
  was valid and additive-only.
- **Remote migration:** additive-only `ADD COLUMN` + `CREATE INDEX` — verified data-loss-safe
  by before/after row counts (33/33, 161/161).
- **Frontend rebuild before deploy:** `dist/` is gitignored and was stale; rebuilt via
  `bash build.sh` (4.4M WASM) so the deploy shipped current Phase 3 UI.
- **Standard `wrangler deploy`:** succeeded first try (no PUT API fallback needed this run).

---

## 3. Code / Migration state

### 3.1 What landed in `develop` (merge commit `4fad6d9`)

25 files changed, +2714/-36. Highlights:

- **Migrations:**
  - `worker/migrations/0022_thb_deposits_held_as_credit.sql`
  - `worker/migrations/0023_contacts_credit_refund_requested.sql`
- **Backend handlers:**
  - `worker/src/handlers/deposit/thb/handlers/hold_admin.rs` (Phase 2 admin endpoints)
  - `worker/src/handlers/deposit/thb/handlers/hold_refund_request.rs` (Phase 3 exit path, 4 endpoints)
  - `worker/src/handlers/deposit/thb/handlers/hold_credit.rs` (Phase 1 + idempotency fix from H128)
- **D1 layer:** `worker/src/db/contacts.rs` (credit_refund_requested helpers + aggregate),
  `worker/src/db/thb_deposits.rs` (held_as_credit column).
- **Sheets layer:** `worker/src/sheets/contacts.rs` (column N, range A:M → A:N).
- **Frontend API:** `frontend-leptos/src/api/deposit.rs` (+315 lines).
- **Frontend UI:**
  - `frontend-leptos/src/pages/ticket/action_cards.rs` (`HoldDepositCard`, `RequestCreditRefundCard`)
  - `frontend-leptos/src/pages/ticket/in_person_view.rs` (wiring)
  - `frontend-leptos/src/pages/admin_deposit.rs` (Held tab + liability chip + refund-requested badge)
  - `frontend-leptos/style.css`
- **Docs:** `.issues/061_thb_hold_deposit_frontend.md`, `.handovers/128_thb_hold_idempotency_fix.md`

### 3.2 Why the migrations are data-loss-safe (the key fact for this session)

Both migrations are **additive-only**. Confirmed by reading the SQL before running anything:

```worker/migrations/0022_thb_deposits_held_as_credit.sql
ALTER TABLE thb_deposits ADD COLUMN held_as_credit INTEGER NOT NULL DEFAULT 0;
ALTER TABLE thb_deposits ADD COLUMN held_as_credit_at TEXT;
```

```worker/migrations/0023_contacts_credit_refund_requested.sql
ALTER TABLE contacts ADD COLUMN credit_refund_requested INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN credit_refund_requested_at TEXT;
CREATE INDEX IF NOT EXISTS idx_contacts_credit_refund_requested
    ON contacts(credit_refund_requested, credit_refund_requested_at)
    WHERE credit_refund_requested = 1;
```

**Zero** `DROP`, `DELETE`, `UPDATE`, `RENAME`, or type-change statements. SQLite's
`ADD COLUMN ... DEFAULT` does **not** rewrite any existing row — existing row bytes are
identical before and after. The partial index only builds new index structure, mutates no
rows. So even the *remote* apply is data-loss-safe by construction; the only reason we held
back was the user's blanket "don't touch prod" instruction.

### 3.3 Local D1 dry-run result

```
npx wrangler d1 migrations apply bethere-db --local
```

- `0022` was already applied in a prior session (tracker correctly skipped it).
- `0023` applied successfully (4 commands executed).
- Post-apply `PRAGMA table_info` confirms:
  - `thb_deposits.held_as_credit` (INTEGER NOT NULL DEFAULT 0) ✅
  - `thb_deposits.held_as_credit_at` (TEXT nullable) ✅
  - `contacts.credit_refund_requested` (INTEGER NOT NULL DEFAULT 0) ✅
  - `contacts.credit_refund_requested_at` (TEXT nullable) ✅
  - `idx_contacts_credit_refund_requested` present ✅
- `wrangler d1 migrations list bethere-db --local` → **"✅ No migrations to apply!"**

The migration SQL is valid and applies cleanly. The same migrations applied to remote will
behave identically (additive-only).

---

## 4. Where is the plan / code / test

- **Plan:** `.issues/061_thb_hold_deposit_frontend.md` (Issue #061, all 3 phases marked ✅ DONE)
- **Code:** merged to `develop` at commit `4fad6d9` (merge commit)
  - Feature branch `feature/061_thb_hold_frontend` **deleted** (work preserved in `develop`)
- **Tests:** worker tests 246 pass / 0 fail (last run pre-merge, prior session)
- **CI:** PR #23 checks `pass` (check+clippy+test, lint-design) at merge time
- **Verification this session:** `cargo clippy` on `develop` clean (EXIT_CODE=0)

---

## 5. Reflection — what went smoothly vs. what to watch

### Solved cleanly
- **Stale checkbox catch.** Reviewing Issue #061 surfaced a Phase 2 item left unchecked
  ("Badge for credit refund requested attendees — Phase 3 dependency") even though Phase 3
  shipped exactly that badge. Marked it `[x]` with commit ref rather than leaving the doc
  lying about its own state. (commit `5825cb3`)
- **Merge convention discovery.** Inspected `git log origin/develop` *before* choosing a merge
  strategy — saw `Merge pull request #15 from ...` pattern, so used `--merge` (merge commit)
  instead of squash. Preserves the granular Phase 1/2/3 history for audit.
- **Local-only migration proof.** `--local` flag is the correct dry-run mechanism: wrangler
  even prints "To execute on your remote database, add a --remote flag" as a guardrail.

### Watch items / mild friction
- **Stale remote-tracking refs after `--delete-branch`.** `gh pr merge --delete-branch`
  deleted both local + remote branches, but left `remotes/origin/feature/061_thb_hold_frontend`
  in the local ref cache. Needed `git fetch --prune origin` to clean up (also pruned 2 other
  stale feature branches). Not a problem, just a tidy-up step worth remembering.
- **`wrangler d1 execute ... --command "SELECT hash FROM d1_migrations"` errored**
  (`no such column: hash`). The wrangler 4.99 schema for `d1_migrations` doesn't have a `hash`
  column — just `id`, `name`, `applied_at`. The `migrations list` subcommand is the reliable
  way to check tracker state, not a hand-rolled query.
- **Local D1 had 0 rows** in both `thb_deposits` and `contacts` (fresh dev fixture). So the
  data-loss sanity check was vacuously true here. On a real prod apply, `ADD COLUMN` still
  can't lose data (SQLite semantics), but the proof is structural not empirical.

---

## 6. Deploy execution (this session — DONE)

Issue #061 is **fully complete AND deployed to production.** Both deploy-gated steps ran
this session after the user said "go for it":

### ✅ Remote prod D1 migration — DONE

```sh
cd worker
npx wrangler d1 migrations apply bethere-db --remote
```

Applied `0022` then `0023` (3 + 4 commands). Additive-only → data-loss-safe. Verified by
before/after row counts on real prod data:
- `thb_deposits`: 33 → 33 (unchanged)
- `contacts`: 161 → 161 (unchanged)

Schema verified post-apply: `held_as_credit` + `held_as_credit_at` on `thb_deposits`;
`credit_refund_requested` + `credit_refund_requested_at` on `contacts`;
`idx_contacts_credit_refund_requested` present. `wrangler d1 migrations list --remote` →
"✅ No migrations to apply!".

### ✅ Worker deploy — DONE

```sh
cd frontend-leptos && bash build.sh   # rebuild WASM (dist was stale, gitignored)
cd worker && bash deploy.sh            # production deploy
```

- Deployed via **standard `wrangler deploy`** (PUT API fallback not needed this run).
- **Version ID: `e14a8d11-c391-41c9-9bdc-cdccb2b04ef9`**
- Live at `https://bethere.solana-thailand.workers.dev`
- 4 new/modified assets uploaded (index.html, wasm, js, css).
- Smoke test: `/api/health` → 200; `/api/refund/held`, `/api/deposit/credit-liability`,
  `/api/deposit/credit-refund-requests`, `/api/deposit/credit-balance` → all 401 (route
  exists + auth gate working). Initial 404 on `/api/refund/held` was transient edge-cache
  propagation during deploy — cleared within seconds.

### 🟧 Optional (staging dry-run, if desired before prod)

Staging has its own D1 (`bethere-db-staging`, id `951fce4e-...`, see `wrangler.toml`
`[env.staging]`). To validate migrations on staging before prod:
```sh
cd worker
npx wrangler d1 migrations apply bethere-db-staging --remote --env staging
```
**Note (from handover 128 / commit `2724c01`):** staging was previously ruled out as a
hold-guard sandbox because it's "not pre-seeded" — i.e., it doesn't have the event/contact
fixtures needed to exercise the hold write path end-to-end. So staging migration proves the
SQL applies, but doesn't prove the feature works without first seeding test data.

### 🟩 Optional follow-ups (explicitly deferred in Issue #061 §6)

- **Phase 2 (a1):** per-row `credit_thb` / `credit_usdc` columns on the admin attendee list.
  Requires a contact-credit join on `list_attendees` — performance design decision pending.
- **Phase 2 (b):** separate cross-event contacts/credit table (the "correct" long-term home).
- **Test debt:** regression test for the hold idempotency guard + Phase 3 flag flows. Needs
  `AppState` + D1/KV test scaffold that worker handlers can't run under plain `cargo test`.
- **Sheets clear sync:** `clear_credit_refund_request_handler` only clears the D1 row; a
  sibling `clear_credit_refund_requested` on `sheets/contacts.rs` (mirroring
  `set_credit_refund_requested`) is the follow-up. Until then, column N on the Sheets master
  may stay stale after a clear (reconciliation cosmetic — all reads use D1).

---

## 7. Issue / PR references

- **Issue:** `.issues/061_thb_hold_deposit_frontend.md` — ✅ fully complete (Phases 1, 2, 3)
- **PR #23:** `https://github.com/solana-thailand/BeThere/pull/23`
  - State: **MERGED** at `2026-07-22T09:04:52Z`
  - Merge commit: `4fad6d9` → `develop`
  - Base: `develop`
  - 18 commits (17 from prior sessions + 1 doc-fix this session)
- **Prior handovers:**
  - H128: THB hold-deposit idempotency fix (Phase 1 backend hardening)
  - H127: admin UI escrow polish
- **Commits this session:**
  - `5825cb3` — `docs(issue): mark 061 Phase 2 refund-requested badge done (delivered via Phase 3)`
  - `4fad6d9` — `Merge pull request #23 from solana-thailand/feature/061_thb_hold_frontend`

---

## 8. How to dev / test

### Verify the merged `develop` is healthy
```sh
git switch develop && git pull
cargo clippy --quiet --manifest-path worker/Cargo.toml   # clean (EXIT_CODE=0)
cargo test --manifest-path worker/Cargo.toml              # 246 pass / 0 fail
```

### Run the worker locally with the migrated local D1
```sh
cd worker
npx wrangler d1 migrations apply bethere-db --local       # already applied this session
# start the dev server (whichever command the project uses, e.g.)
# npx wrangler dev
```

### Exercise the new endpoints locally (smoke)
After `wrangler dev`:
- `POST /api/deposit/hold` (attendee, JWT-gated) — requires a verified THB deposit row
- `GET /api/deposit/credit-balance` (attendee)
- `POST /api/deposit/request-credit-refund` (attendee, sets flag)
- `GET /api/deposit/credit-refund-requests` (admin, lists open flags)
- `POST /api/deposit/clear-credit-refund-request` (admin, clears flag)

The local D1 has 0 rows in `contacts` / `thb_deposits` after this session, so a smoke test
needs a fixture first (insert a contact + a verified THB deposit, then exercise the flow).

### When ready to apply migrations to prod
```sh
cd worker
# sanity-check the diff one more time
git --no-pager diff origin/develop -- migrations/

# apply (additive-only, data-loss-safe per §3.2)
npx wrangler d1 migrations apply bethere-db --remote
```

---

## 9. Status

- ✅ Issue #061 Phases 1, 2, 3 — code complete, merged to `develop`
- ✅ PR #23 — MERGED
- ✅ Feature branch — deleted (local + remote + pruned tracking refs)
- ✅ Local D1 — fully migrated (all 23 migrations, including `0022` + `0023`)
- ✅ Remote prod D1 — fully migrated (`0022` + `0023` applied; row counts unchanged)
- ✅ Frontend rebuilt — fresh WASM (4.4M) shipped to assets
- ✅ Worker deploy — Version `e14a8d11` live at `bethere.solana-thailand.workers.dev`
- ✅ Smoke test — all new routes confirmed live + auth-gated
- ✅ This handover — created + updated post-deploy
- ⏳ `develop` → `main` release cut — **NOT performed** (separate gitflow release step;
  not required for the deploy to take effect — the Worker deploy itself is what went live,
  independent of which git branch `main` points at)

**Issue #061 is fully shipped to production.** The Phase 3 exit path (request return of held
credit) is live. No remaining deploy-gated work; only the optional follow-ups below.