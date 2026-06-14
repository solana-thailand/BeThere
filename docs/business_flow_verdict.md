# Business Flow Audit Verdict

> **Audit date**: 2026-06-14
> **Scope**: BeThere escrow program (`bethere-escrow/`) + worker tx builders (`worker/src/solana_escrow/`)
> **Method**: Static review of code vs `.issues/` spec + `cargo test` + `cargo clippy` + `program_autofixer`
> **Overall verdict**: ✅ **9/9 on-chain instructions PASS** · 1 critical worker bug found & FIXED

---

## Summary Matrix — On-Chain Instructions

| # | Instruction | Code | Test | Logic vs Spec | Verdict |
|---|-------------|------|------|---------------|---------|
| 0 | `create_event` | ✓ | 2 tests | matches `.issues/010` (PDA seeds *more* correct than spec) | ✅ PASS |
| 1 | `deposit` | ✓ | 3 tests | matches `.issues/010` | ✅ PASS |
| 2 | `mark_checked_in` | ✓ | 3 tests | matches `.issues/010` + SEC-011 (post-event-end reject) | ✅ PASS |
| 3 | `refund` | ✓ | 4 tests | matches `.issues/010` + SEC-010 introspection (`.issues/047`) | ✅ PASS |
| 4 | `claim_forfeited` | ✓ | 4 tests | matches `.issues/010` | ✅ PASS |
| 5 | `close_event` | ✓ | 3 tests | double vault-empty guard (accounting + actual balance) | ✅ PASS |
| 6 | `deactivate_event` | ✓ | 3 tests | matches `.issues/010` (soft-disable, refunds still allowed) | ✅ PASS |
| 7 | `close_deposit` | ✓ | 4 tests | dual-path: self-close + GC (after escrow closed) | ✅ PASS |
| 8 | `rollover_deposit` | ✓ | 9 tests | matches `.issues/032` Option B (CHOSEN) | ✅ PASS |
| — | `introspection` (helper module) | ✓ | 4 tests | P0 complete per `.issues/047`; P1/P2/P3 deferred | ✅ PASS |

**Test totals**: 43 passed / 0 failed / 0 ignored (0.08s, quasar-svm).

---

## Summary Matrix — Worker TX Builders

| Builder | File | Account Order vs On-Chain | Verdict |
|---------|------|---------------------------|---------|
| `build_deposit_transaction` | `deposit.rs` | ✓ 9/9 accounts match | ✅ PASS |
| `build_init_escrow_transaction` | `init.rs` | ✓ matches | ✅ PASS |
| `build_mark_checked_in_transaction` | `mark.rs` | ✓ matches | ✅ PASS |
| `build_refund_transaction` | `refund.rs` | ✅ FIXED — was missing `instruction_sysvar` | ✅ PASS (after fix) |
| `build_refund_and_close_transaction` | `refund.rs` | ✅ FIXED — was missing `instruction_sysvar` | ✅ PASS (after fix) |
| `build_claim_forfeited_transaction` | `close.rs` | ✓ 9/9 accounts match | ✅ PASS |
| `build_batch_claim_forfeited_transaction` | `refund.rs` | ✓ matches | ✅ PASS |
| `build_close_deposit_transaction` | `close.rs` | ✓ 4/4 accounts match | ✅ PASS |
| `build_close_event_transaction` | `close.rs` | ✓ 4/4 accounts match | ✅ PASS |
| `build_deactivate_event_transaction` | `close.rs` | ✓ 2/2 accounts match | ✅ PASS |
| `build_rollover_deposit_transaction` | `rollover.rs` | ✓ 11/11 accounts match | ✅ PASS |

**Worker tests**: 117 passed / 0 failed (81 + 15 + 21).

---

## Critical Finding — FIXED

### Worker refund tx builder missing `instruction_sysvar`

**Severity**: 🔴 CRITICAL (production refund flow broken)
**Status**: ✅ Fixed in this audit

**Root cause**: The on-chain `Refund` struct was hardened in `.issues/047` (SEC-010 instruction introspection) to require an `instruction_sysvar: UncheckedAccount` field at account index 6 (between `vault` and `rent`). This enforces that `refund` is always paired with `close_deposit` in the same transaction to prevent rent leaks.

The worker's `build_refund_transaction` and `build_refund_and_close_transaction` in `worker/src/solana_escrow/tx_builders/refund.rs` were **not updated** to include this account — they sent 9 accounts instead of the required 10, causing every production refund to be rejected on-chain with `NotEnoughAccountKeys`.

The production refund handler at `worker/src/handlers/deposit/escrow/handlers.rs:244` calls `build_refund_and_close_transaction`, so this affected the live `/api/escrow/refund` endpoint.

**Why unit tests didn't catch it**: Worker Rust unit tests (117 pass) validate transaction serialization, not on-chain execution. The quasar-svm tests in `bethere-escrow/` (43 pass) use the generated client which includes the sysvar. The e2e devnet scripts (`scripts/e2e/test_escrow_devnet.sh:766`) would catch it but require a live environment.

**Fix applied** (3 files):
- `worker/src/solana_escrow/mod.rs` — added `INSTRUCTIONS_SYSVAR_ID` constant (`Sysvar1nstructions1111111111111111111111111`)
- `worker/src/solana_escrow/tx_builders/mod.rs` — added `instruction_sysvar` field to `EscrowCtx` + populated in `resolve()`
- `worker/src/solana_escrow/tx_builders/refund.rs` — inserted `acct_r(ctx.instruction_sysvar)` at index 6 (after `vault`, before `rent`) in both `build_refund_transaction` and `build_refund_and_close_transaction`

**Verification**: `cargo build` clean · `cargo clippy` clean · 117 worker tests pass · account order now matches on-chain `Refund` struct exactly.

---

## Security Review

All 5 checks PASS.

### 1. Signer checks — PASS
Every mutating instruction has exactly one `Signer` field:
- create_event, deposit → attendee/organizer Signer
- mark_checked_in, claim_forfeited, close_event, deactivate_event → organizer Signer
- refund, rollover_deposit → attendee Signer
- close_deposit → signer Signer (attendee or GC closer)

### 2. PDA derivations — PASS
Every PDA-typed account is bound via `address = X::seeds(...)`. Seeds match `#[seeds]` in `src/state.rs`:
- `EventEscrow`: `["escrow", organizer, event_id]`
- `AttendeeDeposit`: `["deposit", event, attendee]`

Bumps captured from `ctx.bumps` (Quasar canonical pattern). Cross-check vs `.issues/010` spec: spec said `["escrow", event_id]` (no organizer) — the code is *more* correct (organizer-scoped prevents cross-organizer PDA collisions).

### 3. Account validation — PASS
- All SPL token accounts typed as `Account<Token>` / `Account<Mint>` (owner-checked)
- `Program<TokenProgram>` / `Program<SystemProgram>` enforce program ID
- Two `UncheckedAccount` fields have `/// CHECK:` doc + handler-side validation:
  - `Refund.instruction_sysvar` — canonical Instructions sysvar pattern, address validated against `INSTRUCTIONS_SYSVAR_ID`
  - `CloseDeposit.event_escrow` — may be closed/zero-data, intentionally raw

### 4. Time windows — PASS
Three-layer time enforcement:
- `refund`: rejects if `clock < event_end`. No-shows also rejected if `clock >= refund_deadline`. Checked-in attendees skip the deadline.
- `claim_forfeited`: rejects if `clock < refund_deadline`. Also requires `!checked_in` and `!refunded`.
- `mark_checked_in`: rejects if `clock > event_end` (SEC-011 — prevents late check-ins)

### 5. No unauthorized claim paths — PASS
- All organizer-only ops use `has_one(organizer) @ Unauthorized`
- `require_distinct` (Solana Security Checklist #7) on every handler except `refund` (intentionally removed for BPF call depth — PDA seeds guarantee distinctness, per `.issues/047`)
- `validate_version` guards against account-type confusion
- `close_deposit` GC path safe: escrow close requires `total_deposited == total_refunded + total_forfeited` AND `vault.amount() == 0` — deposit fully settled before GC; only rent reclaimed
- `rollover_deposit` requires same organizer + same mint + same deposit_amount + source.checked_in — no cross-org fund movement
- `refund` enforces `attendee_deposit.attendee() == attendee.address()` — no signer can refund someone else's deposit

---

## `program_autofixer` Findings

Ran autofixer on `lib.rs` + all 9 instruction handlers. All findings are **justified false positives** — framework misidentified as Anchor (this is Quasar):

| Rule | Severity | Field | Verdict |
|------|----------|-------|---------|
| `anchor-init-without-space` | HIGH | `Refund.attendee_ta` | False positive — Quasar `token(...)` directive implies SPL token size |
| `anchor-init-without-space` | HIGH | `ClaimForfeited.organizer_ta` | False positive — same as above |
| `anchor-init-without-space` | HIGH | `RolloverDeposit.target_deposit` | False positive — Quasar derives space from `#[account]` struct layout |
| `anchor-unchecked-account` | LOW | `Refund.instruction_sysvar` | Intentional — `/// CHECK:` doc + sysvar ID validated in handler |
| `anchor-unchecked-account` | LOW | `CloseDeposit.event_escrow` | Intentional — `/// CHECK:` doc + `data_len == 0` check in handler |

**Proof**: All 43 tests pass including those that exercise the exact `init` paths flagged. If space were truly missing, the init would fail.

---

## Build & Lint Baseline

### `bethere-escrow/` (on-chain program)
- `cargo build` — ✅ exit 0, zero warnings
- `cargo clippy --all-targets` — ✅ `src/` clean. 19 warnings in `target/client/rust/bethere-escrow-client/` (auto-generated client, dev-dependency only) — out of scope, regenerates on next client build.
- `cargo test` — ✅ 43 passed / 0 failed / 0 ignored (0.08s)

Per-file test breakdown:
| File | Tests | Status |
|------|-------|--------|
| `tests/checkin.rs` | 3 | ✓ |
| `tests/close.rs` | 14 | ✓ |
| `tests/create_event.rs` | 2 | ✓ |
| `tests/deposit.rs` | 3 | ✓ |
| `tests/introspection.rs` | 4 | ✓ |
| `tests/refund.rs` | 4 | ✓ |
| `tests/rollover.rs` | 5 | ✓ |
| `tests/rollover_flow.rs` | 4 | ✓ |
| `tests/mod.rs` (top-level) | 4 | ✓ |

### `worker/` (Cloudflare Worker)
- `cargo build` — ✅ exit 0
- `cargo clippy` — ✅ clean (1 benign profile warning)
- `cargo test` — ✅ 117 passed / 0 failed (81 + 15 + 21)

---

## Known Limitations / Out of Scope

1. **Playwright e2e** (`e2e/*.spec.ts`): require running frontend + worker + devnet RPC — not run in static audit. E2e devnet scripts exist at `scripts/e2e/test_escrow_devnet.sh`, `test_rollover_devnet.sh`, `test_full_e2e.sh` covering all flows; they are the validation path for post-deploy verification.
2. **Auto-generated client warnings** (19 in `bethere-escrow-client/`): out of scope — regenerates from IDL via Codama/codegen.
3. **Introspection P1/P2/P3** (`.issues/047`): multi-deposit prevention, CPI detection, atomic deposit+check-in — deferred, not regressions.
4. **THB deposit path**: off-chain (Google Sheets + PromptPay slips) — not part of on-chain escrow audit.

---

## Spec Cross-Reference

| Issue | Title | On-Chain Status |
|-------|-------|-----------------|
| `.issues/010` | Deposit/Refund with PDA Escrow | ✅ Phases 1–5a complete, 43/43 tests |
| `.issues/032` | Rolling Deposit Credit (rollover) | ✅ Option B implemented (on-chain rollover) |
| `.issues/045` | Security Audit Remediation | ✅ 14/14 fixed (worker-side); on-chain reviewed PASS |
| `.issues/047` | Instruction Introspection | ✅ P0 complete; worker tx builder synced in this audit |

---

## Action Items

- [x] Fix worker refund tx builder to include `instruction_sysvar` (CRITICAL — done in this audit)
- [ ] Run `scripts/e2e/test_escrow_devnet.sh` post-deploy to confirm end-to-end refund flow on devnet
- [ ] Run `scripts/e2e/test_rollover_devnet.sh` post-deploy to confirm rollover flow on devnet
- [ ] Consider adding a worker-side serialization test that asserts refund account count == 10 (regression guard for future introspection changes)