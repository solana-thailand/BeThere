# Handover 128 — THB Hold-Deposit Idempotency Fix (Issue #061 §8 resolved)

> Branch: `feature/061_thb_hold_frontend` (gitflow, branched off `develop` at `0e31032`)
> Continues handover 127 (admin UI escrow polish) and the in-flight Phase 1 work on Issue #061.

---

## 1. What Happened

Phase 1 (attendee-side Hold-Deposit UI) was already complete from the prior session (3 commits,
building clean). While wiring it, the prior session surfaced a **backend idempotency gap** that
this session **resolved before shipping**: `hold_deposit_handler` incremented rolling credit
without settling the source deposit, so a re-call (reload + click, or direct API) would
**double-increment credit** — an unacceptable financial bug on a deposit-touching path.

This session's single deliverable is the **backend hardening** that closes that gap, plus the
**frontend surfacing** required so the attendee ticket page behaves correctly once the backend
became strict. Phase 2 (admin credit visibility) and Phase 3 (exit path) were scoped but
deliberately deferred — see §5.

### The decision that drove the design

The prior session recommended reusing `ThbDeposit.refunded` (= Design A: "set refunded=true on
hold, mirror mark_refund_handler"). This session **rejected that** after auditing the data flow,
because `get_public_ticket` populates the frontend `DepositInfo.refunded` directly from
`ThbDeposit.refunded` (`worker/src/handlers/attendee.rs:372`):

```text
"refunded": thb_deposit.as_ref().is_some_and(|t| t.refunded),
```

Reusing `refunded` would have made a held-as-credit attendee render the **`RefundCard`** on
reload (the `in_person_view` shows `<RefundCard>` whenever `dep.refunded` is true), and would
have mislabeled held deposits as "refunded" in the admin refunded-list — a reconciliation hazard.
A **distinct `held_as_credit` flag** keeps `refunded=false` (no RefundCard, no reconciliation
pollution) and gives the frontend a clean signal to mount the card in an `AlreadyHeld`
confirmation. Cost was one D1 migration + three mechanical D1-layer edits — justified on
financial code.

---

## 2. Changes (1 fix commit + 1 docs commit on top of Phase 1)

### `b5ee048` — `fix(deposit): make hold-as-credit idempotent (resolve double-credit gap)`

**Domain:**
- `domain/src/models/deposit.rs` — `ThbDeposit` gained `held_as_credit: bool`
  (`#[serde(default)]`) + `held_as_credit_at: Option<String>`. Field doc explains why it is
  distinct from `refunded` (retained liability vs released funds).

**Worker — data layer:**
- `worker/migrations/0022_thb_deposits_held_as_credit.sql` — `ALTER TABLE thb_deposits ADD
  COLUMN held_as_credit INTEGER NOT NULL DEFAULT 0` + `held_as_credit_at TEXT`. Conventions
  mirror `refunded`/`refunded_at` from migration `0013`.
- `worker/src/db/thb_deposits.rs` — `insert_thb_deposit`, `update_thb_deposit`, and
  `row_to_thb_deposit` all carry the two new columns. **Careful: the first edit pass
  accidentally dropped the `event_id` bind from `insert_thb_deposit`** (the column order is
  `attendee_id, event_id, amount_thb, …` so `?2` must be `event_id`); restored immediately and
  re-verified by reading the full function. Any future edit to this file must re-check the
  placeholder↔bind alignment — there is no compile-time guard.
- `worker/src/handlers/deposit/thb/handlers/slip_upload.rs` — `ThbDeposit { … }` literal now
  initializes `held_as_credit: false, held_as_credit_at: None`.

**Worker — the fix itself:**
- `worker/src/handlers/deposit/thb/handlers/hold_credit.rs` — `hold_deposit_handler` rewritten:
  1. **Rejects USDC** (defense-in-depth). USDC has no settleable off-chain record; the atomic
     on-chain `rollover_deposit` (`POST /api/escrow/rollover-deposit`) is the correct USDC path.
     Previously the handler accepted USDC and credited it with no settle record — a latent
     double-credit vector reachable via direct API even though the frontend never calls it.
  2. **Loads `ThbDeposit`** (the settleable record; `DepositStatus` has no `refunded` field).
  3. **Guards** on `refunded` ("already refunded") and `held_as_credit` ("already held as
     credit") — the idempotency backstop.
  4. **Settles BEFORE incrementing credit** (`held_as_credit=true`, `held_as_credit_at=now`,
     `save_thb_deposit`). Failure ordering is load-bearing: if settle succeeds but the credit
     increment fails, no money is created and an admin can reconcile; the reverse order would
     permit infinite credit via retry. Mirrors `mark_refund_handler`'s settle-first pattern.
  5. **Audits** via the new `DepositHeldAsCredit` action (non-fatal, sibling of `RefundMarked`).
- `worker/src/audit_store.rs` — added `AuditAction::DepositHeldAsCredit` variant (serde-derived,
  no Display impl to update).
- `worker/src/handlers/deposit/thb/handlers/slip_list.rs` — `refund_queue_handler` filter is now
  `d.verified && !d.refunded && !d.held_as_credit` so the admin refund queue excludes
  held-as-credit deposits (no double-process).

**Worker — surfacing to the ticket page:**
- `worker/src/handlers/attendee.rs` — `get_public_ticket`'s `deposit_info` JSON now includes
  `"held_as_credit": thb_deposit.as_ref().is_some_and(|t| t.held_as_credit)`.

**Frontend — surfacing:**
- `frontend-leptos/src/api/types.rs` — `DepositInfo` gained `held_as_credit: bool`
  (`#[serde(default)]`).
- `frontend-leptos/src/pages/ticket/action_cards.rs` — `HoldDepositState` gained an `AlreadyHeld`
  variant; `HoldDepositCard` gained an `already_held: bool` prop (default false) and initializes
  its signal to `AlreadyHeld` when true. New view arm renders the held-confirmation (no CTA).
- `frontend-leptos/src/pages/ticket/in_person_view.rs` — passes `already_held=dep.held_as_credit`
  to `<HoldDepositCard>`, and the stale "backend not idempotent" NOTE comment is replaced with
  the resolved explanation. Gating unchanged: `verified && !refunded && checked_in && method==Thb`.

### `c5ad563` — `docs(issue): mark 061 Phase 1 done, scope Phase 2/3 after backend hardening`
- `.issues/061_thb_hold_deposit_frontend.md` — Phase 1 checkboxes checked; §8 item 0 added &
  marked **RESOLVED**; §5 Phase 2 annotated with a scoping note (the admin page has a per-event
  attendee list, not a cross-event contacts table — credit lives on the contact, so "credit
  columns" needs a join or a new table before implementing).

---

## 3. Plan / Code / Test

- **Branch:** `feature/061_thb_hold_frontend`
- **Commits this session:** `b5ee048` (fix) → `c5ad563` (docs) on top of Phase 1's `3a05a37`,
  `41f0501`, `3c785f4`.
- **Key files:**
  - `worker/src/handlers/deposit/thb/handlers/hold_credit.rs` — the rewritten handler (the fix)
  - `domain/src/models/deposit.rs` — `ThbDeposit.held_as_credit` field
  - `worker/migrations/0022_thb_deposits_held_as_credit.sql` — schema
  - `worker/src/db/thb_deposits.rs` — D1 insert/update/read (re-verify bind alignment on any edit)
  - `frontend-leptos/src/pages/ticket/action_cards.rs` — `HoldDepositCard` + `AlreadyHeld` state

### Verification
- `cargo check --workspace` → clean
- `cargo clippy --workspace` → clean (no warnings)
- `cargo test` (worker) → **157 + 23 + 15 + 12 + 39 pass, 0 fail**
- `cargo test` (domain) → **104 pass, 0 fail**
- `cargo check --target wasm32-unknown-unknown` (frontend) → clean
- `bash build.sh` → clean (WASM 4.3M, JS 73K, CSS 310K emitted)
- `diagnostics` tool → no errors or warnings project-wide

### Live verification (local sandbox — added after the initial commit)
Ran the worker in `--local` mode (empty local SQLite D1 + empty local KV; no prod data touched):

- **Migration applied cleanly:** `wrangler d1 migrations apply bethere-db --local` ran all 22
  migrations; `0022_thb_deposits_held_as_credit` ✅. `PRAGMA table_info` confirms
  `held_as_credit INTEGER NOT NULL DEFAULT 0` + `held_as_credit_at TEXT` landed correctly.
- **D1 round-trip:** inserted a row with `held_as_credit=1`, read it back → correct; the
  `row_to_thb_deposit` `get_bool`/`get_opt_str` paths deserialize the new columns. (Test row
  deleted after.)
- **Worker boots:** `deploy.sh dev --local` compiled the new WASM and reached
  `Ready on http://localhost:8787` with no panic.
- **Routes respond + auth gates:** `GET /api/deposit/credit-balance` and `POST /api/deposit/hold`
  with no token → **HTTP 401 "missing authentication token"** (route wired, not 404; handler
  loads, not a 500).
- **Handler body executes:** minted a real HS256 JWT using `JWT_SECRET` from `.dev.vars`
  (`Claims { email, sub, iat, exp }`) and POSTed it → **HTTP 404 "event 'nonexistent-evt' not
  found"**, and the log shows my `hold_credit.rs:37` tracing line firing
  (`hold deposit initiated attendee_id=… email=hold-test@example.com`). Confirms the rewritten
  handler runs end-to-end through auth + the first guard; no panic, 19ms.

### Still not verified (residual honest gap)
- **The `held_as_credit` guard branch itself is not exercised against live data.** Reaching it
  needs a real event config in KV + a Google-Sheets attendee + a D1 deposit row + a checked-in
  state — the full Sheets+KV+D1+auth setup, which the empty `--local` sandbox does not provide
  and which `--remote` would write real credit. The guard logic is simple (two boolean checks)
  and the settle-first ordering argument covers the safety property, but it has not been
  observed firing on the second `/hold` call. Manual browser verification against a throwaway
  attendee (or a seeded `--local` harness) remains the open item.
- **No new automated test for the idempotency guard was added.** The existing `worker/tests/`
  suite has no `hold_credit` harness (confirmed: the 4 test files are pure serialization/logic),
  and standing one up requires the full `AppState` + D1/KV + Sheets mock scaffold. Tracked in §5.

---

## 4. Reflection / Struggles / Solved

### Solved — the design-A trap
The prior session's "mirror mark_refund, reuse `refunded`" recommendation was reasonable in
isolation but would have introduced a **real UX + reconciliation bug**: the frontend RefundCard
keys off `refunded`, and the admin refunded-list filters on `refunded`. Overloading it would
show a refund card to held-as-credit attendees and pollute refund reconciliation with
non-refund rows. The fix was tracing the `ThbDeposit.refunded` value through
`get_public_ticket` → frontend `DepositInfo` → `in_person_view` branching. Lesson re-learned:
**before reusing a boolean flag on financial code, trace every consumer of that flag.**

### Solved — USDC arm was a latent vector
The handler's `match deposit.method` accepted USDC and credited it with **no settleable record**
(USDC has no off-chain `UsdDeposit`; its state is on-chain, settled atomically by
`rollover_deposit`). The frontend never calls `/deposit/hold` for USDC (uses the rollover card),
but the endpoint was reachable by direct API. Rejecting USDC closes that vector at the source.

### Solved — bind-alignment regression caught
The first edit to `insert_thb_deposit` silently dropped the `event_id` bind, which would have
shifted every column by one and corrupted inserts. Caught by reading the full function after the
edit, not by the compiler (D1 binds are `&[D1Type]` — no compile-time arity check). The
handover rule "don't guess, adapt from existing code" + post-edit re-read is what caught it.

### Struggle — scoping Phase 2 honestly
Phase 2's issue text assumes an "admin contacts table" exists. It does not — the admin page has
a **per-event attendee list** (`AttendeeListItem`); cross-event contacts are CSV-export-only via
`GET /api/contacts/audience`. Credit lives on the **contact** (D1 cols K–M), not the attendee
row. So Phase 2 needs a product decision (enrich attendee rows vs. build a contacts table)
before coding. Flagged in the issue rather than guessing.

---

## 5. Remain Work

### Phase 2 — Admin credit visibility (deferred — needs a product decision)
- [ ] Decide home for credit data: enrich `AttendeeListItem` with a contact-credit join, **or**
      build a new in-app cross-event contacts table. Do NOT assume the per-event attendee list
      is the right surface.
- [ ] `credit_thb` / `credit_usdc` columns on the chosen surface (data already in D1 cols K–M)
- [ ] Liability header chip — "Total credit held: X THB across N attendees" (needs a sum query;
      `get_credit_balance` is per-email, a new aggregate endpoint is required)
- [ ] Badge for "credit refund requested" attendees (Phase 3 dependency)

### Phase 3 — Exit path (deferred)
- [ ] Backend: `credit_refund_requested` flag on contact (one column or KV flag)
- [ ] Attendee: "Request Return" button on ticket page (sets flag, no payout)
- [ ] Admin: badge/queue for "credit refund requested" → processed via existing refund tooling

### Test debt (this session)
- [ ] Add a `worker/tests/hold_credit_idempotency.rs` regression test asserting: (a) a second
      `/deposit/hold` call returns `Validation("already held as credit")` and does not increment
      credit a second time; (b) a USDC deposit is rejected; (c) a `refunded` deposit is rejected.
      Requires the `AppState` + D1/KV test scaffold.
- [ ] Manual browser verification once a sandbox worker is running (see §3 "Not verified").

### Merge / deploy
- [ ] Before merge: rebase `feature/061_thb_hold_frontend` onto latest `develop`.
- [ ] **Migration `0022` must be applied** (`wrangler d1 migrations apply`) on any environment
      running this branch, or D1 writes carrying `held_as_credit` will fail. The read path is
      safe pre-migration (`row_to_thb_deposit` defaults missing columns to false).

---

## 6. Issues Ref

- **#061** — THB Hold-Deposit Frontend (this branch; §8 item 0 resolved, Phase 1 done, Phase 2/3 scoped)
- **#032** — Rolling Deposit Credit (parent design; backend shipped prior)
- **#045** — Security Audit Remediation (VULN-012 ownership check already guards `hold_deposit_handler`; this session's idempotency fix is a sibling hardening)

---

## 7. How to Dev / Test

```bash
# Backend checks (fast)
cd worker && cargo check --quiet && cargo test --quiet
cd ../domain && cargo test --quiet

# Frontend WASM build
cd ../frontend-leptos && cargo check --target wasm32-unknown-unknown --quiet && bash build.sh

# Workspace-wide lint
cd .. && cargo clippy --workspace --quiet

# Local UI test (sandbox — does NOT touch prod data)
# 1. Ensure wrangler.toml is pointed at --local (or use deploy.sh local if it exists)
# 2. bash deploy.sh dev   # restarts the :8787 worker with fresh assets
# 3. Browser: log in as a verified, checked-in THB-deposit attendee → ticket page
#    - HoldDepositCard shows "Hold Deposit for Next Event" CTA
#    - Click → Confirm → "Confirm & Hold" → Confirmed (shows balance)
#    - Reload page → card now shows "Deposit Held as Credit ✓" (AlreadyHeld), NO RefundCard
#    - Direct second POST /api/deposit/hold → 400 "deposit already held as credit"

# Apply the migration to a D1 environment before running this branch there:
# wrangler d1 migrations apply <DB_NAME> --remote   # or --local
```

### Key invariants to preserve on future edits
1. **`hold_deposit_handler` settles BEFORE incrementing credit.** Reversing this reopens the
   double-credit window. The comment at the settle step explains why.
2. **`held_as_credit` and `refunded` are distinct terminal states.** Do not collapse them — the
   frontend RefundCard and the admin refund reconciliation both depend on `refunded` meaning
   "funds released back to attendee."
3. **`insert_thb_deposit` / `update_thb_deposit` bind arrays must align 1:1 with the SQL
   placeholder order.** D1 binds are untyped `&[D1Type]`; the compiler will not catch a shift.
``````
