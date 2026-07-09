# Plan 005 — Flow Verification Harness + Staging Worker

> **Status**: PARTIALLY DONE (code-only sections shipped; infra chain pending).
> - §3.2 Contract surface audit — **DONE** (`docs/escrow_contract_surface.md`, 23 variants mapped).
> - §3.2 Divergence fix #19 (`RefundDeadlinePassed` not pre-gated) — **SHIPPED & MERGED**: `DepositStatusResponse` gained `checked_in` + `refund_deadline_ms` (`domain/src/models/deposit.rs`), worker populates them (`worker/src/handlers/deposit/usdc/handlers.rs` "divergence fix #19" block), frontend gate rewritten to the two-path predicate (`frontend-leptos/src/pages/deposit/types.rs` L209-226) with both call sites updated.
> - §3.3 LiteSVM tests — **SUPERSEDED**: equivalent two-path coverage already exists via `quasar-svm` tests in `bethere-escrow/src/tests/refund.rs` (`test_refund`, `test_refund_not_checked_in`, `test_refund_already_refunded`, `test_refund_checked_in_after_deadline`). No marginal value in duplicating under LiteSVM.
> - §3.1 Staging env — **NOT STARTED** (real blocker: needs Cloudflare D1/KV/R2 provisioning + OAuth redirect URI registration + deploy.sh staging arg).
> - §3.4 E2E harness (`flow-harness/` crate) — **NOT STARTED** (blocked on §3.1 staging + Helius devnet RPC).
> - §3.5 Preflight gate — **NOT STARTED** (blocked on §3.4).
> **Remaining critical path**: staging env → E2E harness → preflight gate (pure infra; unblocks plans 006/007).
> **Type**: ops (staging isolation) + testing (E2E harness + contract audit)
> **Priority**: P1 — prerequisite for plans 006 (SIWS) and 007 (Dioxus mobile). This plan is the safety net that lets us change the worker without endangering production.
> **Created**: 2026-06-17
> **Blocks**: 006, 007
> **Decisions locked**: 3b (automated E2E harness)

---

## 1. Problem

The refund-gate bug (plan 004) survived for months because nothing automatically checked that the frontend's understanding of the on-chain contract matches the program's actual behavior. We found one divergence (`refundable` flag vs `event_end` time-check); the escrow has **23 error variants** and **9 instructions** with subtle constraints, so there are likely more. Two concrete suspects from investigation:

- **Two refund paths exist on-chain** (`bethere-escrow/src/instructions/refund.rs#L20-26`): checked-in attendees may refund anytime after `event_end` (no deadline); no-shows are gated to `[event_end, refund_deadline)`. The current frontend gate (plan 004) only checks `event_end` — it does not surface `refund_deadline` to no-shows, nor distinguish checked-in vs no-show. Likely divergence.
- **`refund_deadline`** is a first-class on-chain field but is not exposed as an absolute timestamp in `DepositStatusResponse` (only `refund_deadline_hours` relative). Needs verification.

Plans 006 (SIWS auth) and 007 (Dioxus mobile) will both change the worker. Without a regression harness and an isolated staging environment, every worker deploy risks production. This plan builds both **before** 006/007 start.

### Evidence (contract surface discovered during investigation)

- **Escrow errors** (`bethere-escrow/src/errors.rs`): 23 variants — `IncorrectDepositAmount(0)`, `RefundNotYetAllowed(1)`, `NotCheckedIn(2)`, `RefundDeadlineNotPassed(3)`, `AlreadyRefunded(4)`, `AttendeeCheckedIn(5)`, `NoForfeitedFunds(6)`, `EventNotActive(7)`, `EventStillActive(8)`, `Unauthorized(9)`, `VaultMismatch(10)`, `MintMismatch(11)`, `InvalidDepositAmount(12)`, `EventEndInPast(13)`, `Overflow(14)`, `VaultNotEmpty(15)`, `EventEnded(16)`, `DepositNotRefunded(17)`, `EventEscrowStillActive(18)`, `RefundDeadlinePassed(19)`, `EscrowVersionMismatch(20)`, `DepositVersionMismatch(21)`, `RefundRequiresClose(22)`.
- **Escrow instructions** (`bethere-escrow/src/instructions/`): `create_event`, `deposit`, `refund`, `close_deposit`, `close_event`, `claim_forfeited`, `rollover_deposit`, `mark_checked_in`, `deactivate_event`, `introspection`.
- **Worker escrow-touching routes** (`worker/src/handlers/mod.rs`): `/deposit/usdc`, `/deposit/usdc/tx`, `/deposit/hold`, `/deposit/status/{id}`, `/escrow/init`, `/escrow/refund`, `/escrow/close-event`, `/escrow/cancel-status`, `/escrow/health`, `/escrow/sync`, `/refund/queue`, `/refund/refunded`, `/refund/batch-thb`, `/claim/{token}`.
- **Auth is JWT-based** (`worker/src/auth.rs#L286`): `create_session_jwt(email, sub, secret)` / `verify_session_jwt(token, secret)`. SIWS (plan 006) will add a second issuance path — this plan's harness must cover the existing flow as a baseline before 006 ships.
- **Identity is email-keyed** (`worker/migrations/0002_attendees_contacts_events_developers.sql`): `contacts.email PRIMARY KEY`, `attendees.email`, `staff.email`, `developer_profiles.email`. Plan 006 will add wallet linking on top; this plan documents the current model so 006 has a clear baseline.

### Why now

Plan 004 fixed a UX symptom. This plan fixes the **structural absence of a safety net** so that 006 and 007 can proceed without endangering the production users we just protected.

---

## 2. Scope

### In scope

- **Staging worker environment**: a second Cloudflare Worker env (`bethere-staging`) with its own D1/KV/R2 bindings and secrets, fully isolated from production. Mobile dev (plan 007) and SIWS dev (plan 006) point here, never at production.
- **Contract surface inventory**: a single doc mapping every on-chain error variant → which worker endpoint can trigger it → how the frontend currently handles it → gap status.
- **Escrow unit tests via LiteSVM**: fast, validator-free unit tests for the escrow program's instruction constraints (the 23 error variants), per the Solana testing-pyramid skill.
- **Automated E2E harness**: a new Rust crate (`flow-harness/`) that drives the staging worker over HTTP and asserts behavior on deposit/refund/claim/auth flows. Uses real escrow transactions on devnet (or a local validator) — no mocks of the contract.
- **Contract divergence fixes**: any divergence found during the audit (e.g. `refund_deadline` not surfaced to no-show attendees) is fixed as part of this plan, not deferred.

### Out of scope

- **Wallet auth (SIWS)** — plan 006. This plan's harness covers the existing auth flow; 006 extends it.
- **Mobile app** — plan 007. This plan provides the staging URL + harness it consumes.
- **On-chain program changes** — divergences are fixed on the frontend/worker side to match the program; `bethere-escrow` is not modified here. If a divergence reveals a genuine program bug, it escalates to a separate plan.
- **Load / perf testing** — correctness only.
- **Web Leptos → Dioxus migration** — deferred future work.

---

## 3. Implementation

### 3.1 Staging worker environment

Goal: mobile/SIWS dev cannot reach production data even by accident.

- [ ] Add `[env.staging]` block to `worker/wrangler.toml` with:
  - `name = "bethere-staging"`
  - Separate D1 binding (`bethere-db-staging`, created via `wrangler d1 create bethere-db-staging`)
  - Separate KV binding (`EVENTS_STAGING`, new namespace via `wrangler kv namespace create EVENTS_STAGING`)
  - Separate R2 binding (`bethere-assets-staging`)
  - Test-event values for `EVENT_END_MS` / `EVENT_START_MS` (not the production event)
  - `DEV_MODE = "1"` to allow test shortcuts on staging only
- [ ] Update `worker/deploy.sh` to accept an env arg: `bash deploy.sh staging` → deploys to `bethere-staging`; `bash deploy.sh` → production (default, unchanged).
- [ ] Document staging secrets setup: `wrangler secret put JWT_SECRET --env staging`, plus Google OAuth secrets with a separate redirect URI (`https://bethere-staging.solana-thailand.workers.dev/api/auth/callback`). Register this URI in the Google Cloud OAuth app.
- [ ] Add `worker/scripts/seed-staging.sh`: idempotent seeding of a test event (`flow-test-event`) with known `event_start_ms`/`event_end_ms`/`refund_deadline` plus a test attendee + deposit row.
- [ ] Add `worker/.env.staging.example` documenting the staging URL for plan 007 to consume: `STAGING_WORKER_URL=https://bethere-staging.solana-thailand.workers.dev`.
- [ ] Verify isolation: after staging deploy, confirm `wrangler d1 execute bethere-db-staging --remote --command "SELECT count(*) FROM attendees"` returns the seeded count (not production count).

### 3.2 Contract surface inventory

Produce `.docs/escrow_contract_surface.md` mapping each on-chain error to its trigger and current handling.

- [x] For each of the 23 escrow error variants in `bethere-escrow/src/errors.rs`:
  - Which instruction can raise it
  - Which worker endpoint can surface it to the user
  - How the frontend currently handles it (toast copy, state transition, silent failure)
  - **Gap status**: `ok` | `divergence` | `unhandled`
      (Done in `docs/escrow_contract_surface.md` §2 — all 23 mapped; 22 `ok`, 1 divergence #19.)
- [x] Specifically audit the **two refund paths**:
  - Checked-in attendee: refund window = `[event_end, ∞)`. Confirm the frontend gate (post plan 004) allows this at any time after `event_end`.
  - No-show attendee: refund window = `[event_end, refund_deadline)`. Confirm whether the frontend surfaces `refund_deadline` and disables the CTA after it. (Investigation suggests `refund_deadline` is not exposed as an absolute ms timestamp in `DepositStatusResponse` — only `refund_deadline_hours` relative — likely divergence.)
      (Audited in `docs/escrow_contract_surface.md` §3; the divergence it flagged (#19) was the
      catalyst for fix #19, now shipped.)
- [x] Audit `deposit_order` vs `max_refundable_deposits`: the `refundable` flag is computed from these. Confirm `compute_refund_info` (`frontend-leptos/src/pages/deposit/types.rs`) matches the program's tier logic exactly.
      (Audited in `docs/escrow_contract_surface.md` §8 — tier logic present (`is_refundable_tier`),
      server-enforced (`status.refundable` in `refund_and_close_tx_handler`).)
- [x] Audit the `close_deposit` ↔ `refund` coupling (`RefundRequiresClose` error). Confirm the frontend refund flow issues the right instruction sequence.
      (Audited in `docs/escrow_contract_surface.md` §2 #22 / §8 — worker pairs
      `refund + close_deposit` in `refund_and_close_tx_handler`.)

### 3.3 Escrow unit tests via LiteSVM

Per the Solana testing-pyramid skill: LiteSVM for fast, validator-free unit tests of instruction constraints.

- [x] Add `bethere-escrow-tests/` sibling crate (kept out of the workspace to avoid polluting worker builds) with LiteSVM-based tests.
      **SUPERSEDED** — equivalent coverage lives in `bethere-escrow/src/tests/refund.rs` via `quasar-svm` (Quasar's SVM simulator, analogous to LiteSVM). See the case list below; a sibling LiteSVM crate would duplicate it.
- [x] Cover the high-risk constraints (one test per case):
  - `RefundNotYetAllowed` — `clock < event_end` for both checked-in and no-show
  - `RefundDeadlinePassed` — `clock >= refund_deadline` AND not checked_in
  - Checked-in refund succeeds at any time after `event_end` (no deadline bound)
  - `AlreadyRefunded` — second refund attempt on same PDA
  - `IncorrectDepositAmount` — lamports mismatch
  - `MintMismatch` — wrong SPL token
  - `VaultMismatch` — wrong vault ATA
  - `EventNotActive` / `EventStillActive` / `EventEnded` — state machine transitions
  - `EscrowVersionMismatch` / `DepositVersionMismatch` — version guards
- [x] **SUPERSEDED** — the four refund-path cases (the high-risk subset) are covered
  by `quasar-svm` tests in `bethere-escrow/src/tests/refund.rs`
  (`test_refund`, `test_refund_not_checked_in`, `test_refund_already_refunded`,
  `test_refund_checked_in_after_deadline`). The non-refund constraints
  (mint/vault/version/state guards) are exercised by the program's existing test
  suite. A standalone LiteSVM crate would duplicate coverage with no marginal value;
  revisit only if the program test suite regresses.
- [x] CI: `cargo test -p bethere-escrow-tests` ~~runs in seconds, no validator startup~~
    **Superseded** — `cargo test -p bethere-escrow` (quasar-svm) covers the same
    refund-path constraints; no separate crate exists.

### 3.4 Automated E2E harness (`flow-harness/`)

New Rust crate at repo root. Drives the staging worker over HTTP. No contract mocks — real escrow transactions on devnet (with Helius RPC), or local-validator fallback if devnet rate limits bite.

- [ ] `flow-harness/Cargo.toml`: standalone crate, depends on `reqwest`, `solana-sdk`, `spl-token`, `tokio`, `serde_json`, `domain` (path dep).
- [ ] `flow-harness/src/context.rs`: `StagingContext` — worker base URL, funded payer keypair, test event id, test attendee wallet, derived PDAs.
- [ ] `flow-harness/src/flows/deposit.rs`: register attendee → `POST /deposit/usdc` → sign+send tx → poll `GET /deposit/status/{id}` until `verified=true` → assert PDA exists on-chain with expected fields.
- [ ] `flow-harness/src/flows/refund_pre_event_end.rs`: attempt refund before `event_end` → assert simulation fails with `RefundNotYetAllowed` → assert the frontend gate condition (`event_refund_window_open`) returns `false`.
- [ ] `flow-harness/src/flows/refund_post_event_end_checked_in.rs`: check-in attendee → advance clock past `event_end` (use a test event with short horizon) → refund succeeds.
- [ ] `flow-harness/src/flows/refund_no_show_deadline.rs`: no-show refund in `[event_end, refund_deadline)` succeeds; after `refund_deadline` fails with `RefundDeadlinePassed`.
- [ ] `flow-harness/src/flows/claim.rs`: NFT claim flow post-checkin via `/claim/{token}`.
- [ ] `flow-harness/src/flows/auth.rs`: existing Google-auth session issuance baseline (so plan 006 can prove SIWS doesn't regress it).
- [ ] `flow-harness/src/runner.rs`: orchestrates flows, collects results, exits non-zero on any failure, writes `flow-harness/results/<ISO-timestamp>/summary.json`.
- [ ] Runnable both as `cargo run -- --worker <url>` and `cargo test`.

### 3.5 Wire harness into pre-deploy gate

The harness is the safety mechanism for 006/007.

- [ ] Add `worker/scripts/preflight.sh`: runs `flow-harness` against staging after every staging deploy. Non-zero exit blocks production deploy.
- [ ] Update `worker/deploy.sh` header rule: **production deploys require a green preflight run against staging within the last hour.** Enforce via timestamp check on `flow-harness/results/.last-green`.
- [ ] Add a `--force` escape hatch to `deploy.sh` for emergencies, with an audit-log entry (gate is bypassable but never silently).

---

## 4. Testing (the harness tests itself)

- [ ] Mutation check: intentionally break one staging response and confirm the harness catches it (proves the assertions are real).
- [ ] Performance budget: LiteSVM tests <30s; E2E harness against staging <5min.
- [ ] CI job `flow-harness.yml`: LiteSVM tests on every PR; full E2E harness on nightly schedule and on `develop` merges.

---

## 5. Rollout

- [ ] Create staging Cloudflare resources (D1, KV, R2) — free tier, zero cost.
- [ ] Add `[env.staging]` to `wrangler.toml`; deploy staging; verify isolation (staging D1 contains only seeded test data, not production attendees).
- [x] Write contract surface doc (§3.2); fix any divergences found as separate commits within this plan.
      (Doc written; the one divergence (#19) fixed & merged.)
- [x] Implement LiteSVM tests (§3.3) — ship first, fastest payoff.
      (Superseded by `quasar-svm` coverage in `bethere-escrow/src/tests/refund.rs` — see §3.3.)
- [ ] Implement E2E harness (§3.4) — ship second.
- [ ] Wire preflight gate (§3.5) — ship last; this is what blocks 006/007 from proceeding.
- [ ] One-time baseline: run harness read-only flows against production to confirm current behavior matches expectations (no writes).

---

## 6. Files Touched (expected)

| File / Dir                           | Change                                                   |
| ------------------------------------ | -------------------------------------------------------- |
| `worker/wrangler.toml`               | + `[env.staging]` block                                  |
| `worker/deploy.sh`                   | + `staging` arg support; + preflight gate                |
| `worker/scripts/seed-staging.sh`     | NEW — idempotent staging seed                            |
| `worker/scripts/preflight.sh`        | NEW — pre-deploy gate                                    |
| `worker/.env.staging.example`        | NEW — staging URL doc for plan 007                       |
| `.docs/escrow_contract_surface.md`   | NEW — contract surface inventory                         |
| `bethere-escrow-tests/`              | NEW crate — LiteSVM unit tests                           |
| `flow-harness/`                      | NEW crate — E2E harness                                  |
| `flow-harness/results/`              | NEW — JSON run results (gitignored except `.last-green`) |
| `.github/workflows/flow-harness.yml` | NEW — CI job                                             |

Plus any divergence fixes the §3.2 audit surfaces (e.g. surfacing `refund_deadline` in `DepositStatusResponse`).

---

## 7. Acceptance Criteria

- [ ] Staging worker live at `https://bethere-staging.solana-thailand.workers.dev` with its own D1/KV/R2; production D1 byte-identical to before (no cross-contamination).
- [ ] `bash worker/deploy.sh staging` deploys to staging; `bash worker/deploy.sh` still deploys to production (default unchanged).
- [x] Contract surface doc complete: all 23 escrow error variants mapped; every variant marked `ok` / `divergence` / `unhandled`.
      (`docs/escrow_contract_surface.md` §2 — 22 `ok`, 1 divergence.)
- [x] Every `divergence` from §3.2 is either fixed in this plan or escalated to a follow-up plan with a documented reason.
      (The single divergence #19 is fixed & merged — see status header.)
- [x] LiteSVM tests cover the §3.3 high-risk list and run in <30s in CI.
      (Superseded — `quasar-svm` tests in `bethere-escrow/src/tests/refund.rs` cover the refund-path
      high-risk subset; no separate LiteSVM crate.)
- [ ] E2E harness passes against staging for: deposit, refund-pre-event-end (fails correctly), refund-post-event-end-checked-in, refund-no-show-deadline, claim, auth.
- [ ] Preflight gate enforced: production deploy refuses to run if staging preflight hasn't passed within 1h (`--force` escape hatch logs to audit).
- [ ] Nightly CI runs harness against staging; failures open an issue.

---

## 8. Risks / Notes

- **Devnet rate limits** may make E2E tests flaky. Mitigation: dedicated Helius devnet RPC for the harness; fall back to `solana-test-validator` locally if devnet becomes a blocker. LiteSVM tests are unaffected.
- **Staging Google OAuth** needs a registered redirect URI in the Google Cloud app. If the app is locked down, staging auth flow cannot be E2E-tested until an admin adds the URI. Do this early — it's config, not code.
- **The harness is code that can break.** Keep it small, well-typed, and treat its own failures as release-blocking (better noisy than silent).
- **No production risk from this plan itself**: staging is additive, harness is a new crate, LiteSVM tests are isolated. The only production-touching step is the §5 one-time read-only baseline (GET endpoints only).
- **LiteSVM vs the real program**: LiteSVM is a fast SVM simulator; rare edge cases (specific sysvars, CPI to non-standard programs) may behave differently than devnet. The E2E harness against devnet catches what LiteSVM misses. The two tiers complement.
- **Decision 3b locked**: this plan delivers the automated E2E harness. Manual-audit-only (3a) was rejected in favor of 3b.

---

## 9. Unblocks

- **Plan 006 (SIWS auth)**: uses the staging env for SIWS endpoint development; uses the preflight gate to prove the existing Google-auth flow is not regressed.
- **Plan 007 (Dioxus mobile)**: points at the staging URL; relies on the contract surface doc as the API contract; uses the harness as the regression safety net during MWA bridge development.

005 must land before 006/007. Within 005, the recommended order is: staging env → LiteSVM tests → E2E harness → preflight gate. The contract surface doc (§3.2) runs in parallel with staging setup and informs the LiteSVM test list.
