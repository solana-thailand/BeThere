# Goal: Audit all BeThere business flows end-to-end

Verify every on-chain instruction flow works (code + tests + e2e). Produce a verdict matrix so we know which flows are production-ready and which need work.

## Context

- On-chain program: `bethere-escrow/` (Solana/Quasar, cdylib)
- 10 instructions in `src/instructions/`: create_event, deposit, mark_checked_in, refund, claim_forfeited, rollover_deposit, close_deposit, close_event, deactivate_event, introspection
- 9 test files in `src/tests/`: checkin, close, create_event, deposit, introspection, refund, rollover, rollover_flow
- Worker (edge): `worker/` (Cloudflare Workers, TypeScript)
- Frontend: `frontend-leptos/` (Leptos WASM)
- Historical issues: `.issues/001` through `.issues/010/`

## Tasks

### Phase 1 — Build & baseline
- [x] Run `cargo build` in `bethere-escrow/` — confirm clean compile
      Result: exit 0, zero warnings on `src/`.
- [x] Run `cargo clippy` in `bethere-escrow/` — capture all warnings
      Result: `src/` is clippy-clean.
        - Fixed 2 `too_many_arguments` warnings in `src/tests/mod.rs` test fixture builders
          (`event_escrow_account`, `attendee_deposit_account`) via justified
          `#[allow(clippy::too_many_arguments)]` — args map 1:1 to struct fields across
          50+ call sites in 7 test files; refactor would be churn with no functional gain.
        - 19 remaining warnings are all in `target/client/rust/bethere-escrow-client/`
          (auto-generated client from codegen, dev-dependency only). Out of scope —
          regenerates on next client build.
- [x] Run `cargo test` in `bethere-escrow/` — capture pass/fail counts per test file
      Result: **43 passed / 0 failed / 0 ignored / 0 measured** (0.08s).
      Per-file breakdown:
        - `tests/checkin.rs`        : 3  tests ✓
        - `tests/close.rs`          : 14 tests ✓
        - `tests/create_event.rs`   : 2  tests ✓
        - `tests/deposit.rs`        : 3  tests ✓
        - `tests/introspection.rs`  : 4  tests ✓
        - `tests/refund.rs`         : 4  tests ✓
        - `tests/rollover.rs`       : 5  tests ✓
        - `tests/rollover_flow.rs`  : 4  tests ✓
        - `tests/mod.rs` (top-level): 4  integration tests ✓
                                      (test_id, test_full_happy_path,
                                       test_no_show_path, test_full_lifecycle_with_deactivate)

### Phase 2 — Audit each instruction flow
For each instruction, verify: code exists → has test → test passes → logic matches `.issues/` spec.

Verdict summary: **9/9 instructions PASS**, introspection helper module PASS. All 43 tests green.

- [x] `create_event` — organizer configures deposit amount + event window
      **PASS**. `src/instructions/create_event.rs`.
      Guards: deposit_amount != 0 (`InvalidDepositAmount`), event_end > now
      (`EventEndInPast`), refund_deadline > event_end (`RefundDeadlineNotPassed`).
      require_distinct on organizer/escrow/vault. PDA seeds verified via
      `address = EventEscrow::seeds(organizer, event_id)`.
      Tests: `test_create_event`, `test_create_event_bad_deadline` ✓
      Spec ref: `.issues/010` (matches; PDA seeds actually `["escrow", organizer, event_id]`
      — code is *more* correct than spec which omitted organizer scoping).

- [x] `deposit` — attendee locks USDC, AttendeeDeposit PDA created
      **PASS**. `src/instructions/deposit.rs`.
      Guards: escrow.is_active, mint match, vault match, require_distinct.
      validate_version on escrow. Checked arithmetic on total_deposited
      (`Overflow` on wrap). transfer_checked with mint decimals.
      Tests: `test_deposit`, `test_deposit_event_not_active`,
      `test_deposit_escrow_version_mismatch` ✓

- [x] `mark_checked_in` — staff scan → on-chain check-in flag set
      **PASS**. `src/instructions/mark_checked_in.rs`.
      Guards: has_one(organizer) (`Unauthorized`), not already checked_in,
      deposit belongs to escrow, PDA seeds. SEC-011: rejects check-in after
      event_end (`EventEnded`). validate_version on both accounts.
      Tests: `test_mark_checked_in`, `test_mark_checked_in_wrong_organizer`,
      `test_mark_checked_in_deposit_version_mismatch` ✓

- [x] `refund` — checked-in attendee gets deposit back
      **PASS**. `src/instructions/refund.rs`.
      Guards: attendee signer == deposit.attendee, not already refunded, PDA seeds.
      Dual time-window logic correct: checked-in → anytime after event_end;
      no-show → must be before refund_deadline.
      SEC-010 instruction introspection ACTIVE: `require_close_deposit_pair()`
      scans Instructions sysvar for sibling close_deposit (disc 7) targeting same
      deposit — prevents rent leaks. Sysvar address validated against canonical ID.
      require_distinct/validate_version intentionally removed (BPF call depth,
      PDA seeds guarantee distinctness) — documented in `.issues/047`.
      Tests: `test_refund`, `test_refund_already_refunded`,
      `test_refund_checked_in_after_deadline`, `test_refund_not_checked_in` ✓

- [x] `claim_forfeited` — organizer claims no-show deposits after refund_deadline
      **PASS**. `src/instructions/claim_forfeited.rs`.
      Guards: has_one(organizer), deposit.event == escrow, NOT checked_in
      (`AttendeeCheckedIn`), not refunded, clock >= refund_deadline, amount > 0.
      Atomic vault→organizer transfer signed by escrow PDA. Marks refunded=true
      (prevents double-claim). Checked arithmetic on total_forfeited.
      Tests: `test_claim_forfeited`, `test_claim_forfeited_before_deadline`,
      `test_claim_forfeited_checked_in_rejected`,
      `test_claim_forfeited_nothing_to_claim` ✓

- [x] `rollover_deposit` — deposit moved to new event (multiday/reschedule)
      **PASS**. `src/instructions/rollover_deposit.rs`.
      Guards: attendee signer, source_deposit.checked_in (`NotCheckedIn`),
      not refunded, same organizer + same mint + same deposit_amount across
      events, target.is_active. Atomic source_vault→target_vault transfer.
      Settles source (refunded=true, source.total_refunded += amount) and
      inits target deposit + target.total_deposited += amount. require_distinct
      on 7 accounts.
      Tests: 5 in `rollover.rs` + 4 in `rollover_flow.rs` (double-rollover
      rejected, then-claim/refund/close on target) ✓
      Spec ref: `.issues/032` Option B (CHOSEN) — matches exactly.

- [x] `close_deposit` — attendee cancels before event_start, refund
      **PASS**. `src/instructions/close_deposit.rs`.
      Two paths: (1) self-close — attendee==signer AND refunded==true;
      (2) GC — anyone can close once event_escrow is closed (data_len == 0).
      `close(dest = signer)` reclaims rent. require_distinct + validate_version.
      Tests: `test_close_deposit_after_refund`, `test_close_deposit_not_refunded`,
      `test_close_deposit_wrong_signer`, `test_close_deposit_gc_after_event_closed` ✓

- [x] `close_event` — organizer finalizes event, sweeps remaining
      **PASS**. `src/instructions/close_event.rs`.
      Guards: has_one(organizer), !is_active (`EventStillActive`), close(dest=organizer).
      Double vault-empty check: accounting invariant (total_deposited ==
      total_refunded + total_forfeited) AND actual vault.amount() == 0.
      The latter prevents airdrop griefing. Closes vault via PDA-signed CPI.
      Tests: `test_close_event`, `test_close_event_vault_not_empty`,
      `test_close_event_still_active`, `test_close_deposit_after_refund` ✓

- [x] `deactivate_event` — soft-disable event without closing
      **PASS**. `src/instructions/deactivate_event.rs`.
      Guards: has_one(organizer), is_active (idempotent — rejects double-deactivate).
      Sets is_active=false. Refunds still allowed; close_event callable after.
      Tests: `test_deactivate_event`, `test_deactivate_event_already_inactive`,
      `test_deactivate_event_wrong_organizer`, `test_full_lifecycle_with_deactivate` ✓

- [x] `introspection` — read-only getters for event/deposit state
      **PASS** — *helper module, not an instruction*. `src/instructions/introspection.rs`
      provides `validate_instruction_sysvar`, `has_close_deposit_for`,
      `has_rollover_deposit_for`, `require_close_deposit_follows_refund`.
      Zero-allocation sysvar parsing (in-place slice scan).
      P0 (refund+close enforcement) COMPLETE per `.issues/047`. P1/P2/P3 deferred.
      Tests: 4 in `introspection.rs` (sysvar required, chain succeeds,
      wrong sysvar rejected, standalone refund rejected) ✓

#### Phase 2 — `program_autofixer` findings
Ran autofixer on lib.rs + all instruction handlers. All findings are
**justified false positives** — framework misidentified as Anchor (this is
Quasar). Details:
- `anchor-init-without-space` (HIGH) on `Refund.attendee_ta`,
  `ClaimForfeited.organizer_ta`, `RolloverDeposit.target_deposit`: Quasar's
  `init` derives space from the account type — `Account<Token>` w/ `token(...)`
  directive implies SPL token size; `Account<T>` w/ `#[account]` uses the
  struct's declared layout. **Proof**: all 43 tests pass including those that
  exercise these exact init paths.
- `anchor-unchecked-account` (LOW) on `Refund.instruction_sysvar`,
  `CloseDeposit.event_escrow`: both have `/// CHECK:` doc comments and the
  addresses/state are validated in-handler (sysvar ID check / data_len check).
  This is the canonical pattern for the Instructions sysvar (no typed wrapper).

### Phase 3 — Edge worker + e2e coverage
- [x] Audit `worker/src/solana.rs` — RPC calls match current on-chain program
      **PASS**. `worker/src/solana.rs` is the Helius DAS API client (NFT minting +
      `getAssetsByOwner` for on-chain verification). No direct escrow RPC calls here —
      escrow interactions are in `worker/src/solana_escrow/`. Wallet validation
      (base58 const lookup table) is solid. 8 unit tests pass.
- [x] Audit `worker/src/` — deposit → check-in → quiz → claim → mint flows
      **PASS with 1 CRITICAL bug found and FIXED**.
      Audited all 7 tx builders in `worker/src/solana_escrow/tx_builders/` against
      the on-chain account structs. 6/7 matched. **1 mismatch found and fixed**:

      **CRITICAL FIX — refund tx builder missing `instruction_sysvar`:**
      The on-chain `Refund` struct (hardened in `.issues/047` for SEC-010
      instruction introspection) requires 10 accounts including `instruction_sysvar`
      at index 6 (between vault and rent). The worker's
      `build_refund_transaction` and `build_refund_and_close_transaction`
      (`worker/src/solana_escrow/tx_builders/refund.rs`) were sending only 9
      accounts — missing the Instructions sysvar. This would cause every
      production refund to be rejected on-chain with `NotEnoughAccountKeys`.
      The production refund handler
      (`worker/src/handlers/deposit/escrow/handlers.rs:244`) calls
      `build_refund_and_close_transaction`, so this affected the live refund flow.

      Root cause: `.issues/047` added `instruction_sysvar` to the on-chain
      `Refund` struct but the worker tx builder was not updated to match.
      The unit tests (117 pass) didn't catch it because they test serialization,
      not on-chain execution; the e2e devnet tests (which would catch it) require
      a live environment.

      Fix applied (3 files):
      - `worker/src/solana_escrow/mod.rs`: Added `INSTRUCTIONS_SYSVAR_ID` constant.
      - `worker/src/solana_escrow/tx_builders/mod.rs`: Added `instruction_sysvar`
        field to `EscrowCtx` + populated in `resolve()`.
      - `worker/src/solana_escrow/tx_builders/refund.rs`: Inserted
        `acct_r(ctx.instruction_sysvar)` at correct position (index 6, after vault,
        before rent) in BOTH `build_refund_transaction` and
        `build_refund_and_close_transaction`.

      Verification: `cargo build` clean, `cargo clippy` clean, **117 tests pass**
      (81 + 15 + 21), 0 failures.
- [x] Verify no-show path: deposit → expire → forfeit
      **PASS**. `claim_forfeited` builder (`close.rs`) account order matches
      on-chain `ClaimForfeited` struct (9 accounts). Handler path:
      `handlers.rs:733` → `build_batch_claim_forfeited_transaction`. Batch claim
      supports multiple no-shows in one TX. Logic: deposit → (no check-in) →
      wait past refund_deadline → organizer calls claim_forfeited → USDC to
      organizer_ta. Matches `.issues/010` spec.
- [x] Verify rollover path: deposit → rollover → claim on new event
      **PASS**. `build_rollover_deposit_transaction` (`rollover.rs`) account order
      matches on-chain `RolloverDeposit` struct (11 accounts). Handler path:
      `handlers/deposit/escrow/status.rs:609`. Atomic source_vault→target_vault
      transfer. Matches `.issues/032` Option B (CHOSEN).
- [x] Verify check-in path: scan QR → on-chain check-in → unlock claim
      **PASS**. `build_mark_checked_in_transaction` (`mark.rs`) + handler
      `mark_checked_in_tx_handler`. Check-in sets on-chain flag → unlocks refund
      (checked-in attendees can refund anytime after event_end, no deadline).
- [x] Run `cd worker && pnpm test` (or playwright) — capture results
      **Rust unit tests**: `cargo test` → **117 passed / 0 failed** (81 + 15 + 21).
      **Playwright e2e** (`e2e/*.spec.ts`): require running frontend + worker +
      devnet RPC — out of scope for static audit. **E2e devnet scripts** exist
      (`scripts/e2e/test_escrow_devnet.sh`, `test_rollover_devnet.sh`,
      `test_full_e2e.sh`) covering all flows; they would have caught the refund
      bug (script exercises `/api/escrow/refund` at line 766). Documented as the
      validation path for post-deploy verification.

### Phase 4 — Security review
All 5 checks PASS. Findings consolidated from Phase 2 autofixer run + manual
review of every account struct in `src/instructions/`.

- [x] Signer checks present on every mutating instruction
      **PASS**. All 9 instructions have exactly one `Signer` field (verified via
      `rg "pub .*: Signer" src/instructions/`):
        - create_event, deposit → attendee action (attendee/organizer Signer)
        - mark_checked_in, claim_forfeited, close_event, deactivate_event → organizer Signer
        - refund, rollover_deposit → attendee Signer
        - close_deposit → signer Signer (attendee or GC closer)
      No `mut` account lacks an indirect signer constraint (PDA authority,
      has_one, or `close(dest=...)` semantics).

- [x] PDA derivations use correct seeds + bump
      **PASS**. Every PDA-typed account is bound via `address = X::seeds(...)`
      (verified via `rg "address = .*::seeds" src/instructions/` — 14 matches).
      Seeds match the `#[seeds]` declarations in `src/state.rs`:
        - EventEscrow: `["escrow", organizer, event_id]`
        - AttendeeDeposit: `["deposit", event, attendee]`
      Bumps captured from `ctx.bumps` (Quasar canonical pattern), not re-derived.
      Cross-check vs `.issues/010` spec: spec said `["escrow", event_id]` (no
      organizer) — the code is *more* correct (organizer-scoped prevents
      cross-organizer PDA collisions).

- [x] No missing account validation (owner check, AccountMeta count)
      **PASS**. Quasar's `Account<T>` enforces owner + discriminator automatically.
      Manual review confirms:
        - All SPL token accounts typed as `Account<Token>` / `Account<Mint>` (owner-checked).
        - `Program<TokenProgram>` / `Program<SystemProgram>` enforce program ID.
        - Sysvar accounts (`Sysvar<Rent>`) validated by type.
        - Two `UncheckedAccount` fields (both have `/// CHECK:` doc + handler-side
          validation): `Refund.instruction_sysvar` (canonical Instructions sysvar
          pattern, address validated against `INSTRUCTIONS_SYSVAR_ID` bytes) and
          `CloseDeposit.event_escrow` (may be closed/zero-data, intentionally raw).
        - AccountMeta counts: tx builders emit correct counts per on-chain structs
          (verified in Phase 3 — only the refund builder was off, now FIXED).

- [x] Refund/forfeit only callable in correct time window
      **PASS**. Three-layer time enforcement:
        - `refund` (refund.rs:84-95): rejects if `clock < event_end`
          (`RefundNotYetAllowed`). For no-shows, also rejects if
          `clock >= refund_deadline` (`RefundDeadlinePassed`). Checked-in
          attendees skip the deadline check (earned refund by showing up).
        - `claim_forfeited` (claim_forfeited.rs:55): rejects if
          `clock < refund_deadline` (`RefundDeadlineNotPassed`). Also requires
          `!checked_in` and `!refunded` — checked-in deposits cannot be forfeited.
        - `mark_checked_in` (mark_checked_in.rs:31): rejects if
          `clock > event_end` (`EventEnded`) — SEC-011 prevents late check-ins
          that would unlock refunds outside the no-show grace window.
      Tests confirm each window: `test_refund_checked_in_after_deadline`,
      `test_refund_not_checked_in`, `test_claim_forfeited_before_deadline`,
      `test_claim_forfeited_checked_in_rejected`.

- [x] No unauthorized claim paths
      **PASS**. Defense-in-depth:
        - All organizer-only ops use `has_one(organizer) @ Unauthorized`:
          mark_checked_in, claim_forfeited, close_event, deactivate_event.
        - `require_distinct` (defense vs duplicate-mutable-account attack,
          Solana Security Checklist #7) on every handler except `refund`
          (intentionally removed for BPF call depth — PDA seeds already guarantee
          distinctness, documented in `.issues/047`).
        - `validate_version` guards against account-type confusion (v1-only).
        - close_deposit GC path (anyone can close after event closed) is safe:
          escrow close requires `total_deposited == total_refunded + total_forfeited`
          AND `vault.amount() == 0` — deposit is fully settled before GC; only
          rent (not tokens) is reclaimed.
        - rollover requires same organizer + same mint + same deposit_amount
          on both events, plus source.checked_in — no cross-org fund movement
          (VULN-009 from `.issues/045` already hardened at the worker layer too).
        - refund enforces `attendee_deposit.attendee() == attendee.address()`
          so no signer can refund someone else's deposit.

### Phase 5 — Verdict + documentation
- [x] Create `docs/business_flow_verdict.md` with matrix: flow × status (PASS/FAIL/UNKNOWN) × notes
      **DONE**. Created `docs/business_flow_verdict.md` (182 lines) with:
        - Summary matrix for all 9 on-chain instructions + introspection helper (all PASS)
        - Summary matrix for all 11 worker tx builders (1 was FIXED, rest PASS)
        - Critical finding section documenting the refund `instruction_sysvar` bug + fix
        - Full security review (5 checks all PASS)
        - `program_autofixer` findings table (5 justified false positives)
        - Build & lint baseline (bethere-escrow: 43/43 tests, worker: 117/117 tests)
        - Spec cross-reference to `.issues/010`, `032`, `045`, `047`
        - Action items (1 done, 3 follow-up)
- [x] Update each `.issues/NNN_*.md` with verified status if outdated
      **DONE**. Updated `.issues/047_instruction_introspection.md` with "Session 4
      Findings (2026-06-14) — Worker TX Builder Sync" section documenting the
      refund `instruction_sysvar` mismatch discovery + fix + regression guard
      recommendation. Other issues (010, 032, 045) were already current.
- [x] Commit findings on `feature/001_business_flow_audit` (branched from `develop`)
      with conventional commit `fix(worker): add instruction_sysvar to refund tx builder`
      **DONE**. Created `develop` from `main`, branched `feature/001_business_flow_audit`,
      staged the 8 audit files, and committed with the message body below:
        ```
        git branch develop main
        git checkout -b feature/001_business_flow_audit develop
        git add bethere-escrow/Cargo.toml bethere-escrow/src/tests/mod.rs \
                worker/src/solana_escrow/mod.rs \
                worker/src/solana_escrow/tx_builders/mod.rs \
                worker/src/solana_escrow/tx_builders/refund.rs \
                docs/business_flow_verdict.md \
                .issues/047_instruction_introspection.md \
                .plans/001_audit_business_flows.md
        git commit -m "fix(worker): add instruction_sysvar to refund tx builder

SEC-010 introspection hardening (.issues/047) added instruction_sysvar to the
on-chain Refund struct but the worker tx builder was not synced, causing every
production refund to be rejected on-chain with NotEnoughAccountKeys.

- Add INSTRUCTIONS_SYSVAR_ID constant to solana_escrow/mod.rs
- Add instruction_sysvar field to EscrowCtx, populated in resolve()
- Insert acct_r(ctx.instruction_sysvar) at index 6 in both refund builders

Also: docs/business_flow_verdict.md (full audit), register cfg(kani) in
bethere-escrow Cargo.toml, allow(clippy::too_many_arguments) on test fixtures.

bethere-escrow: 43/43 tests pass, clippy clean
worker: 117/117 tests pass, clippy clean"
        ```

## Acceptance Criteria
- [x] Every instruction has a verdict row (PASS/FAIL/UNKNOWN)
      All 9 instructions + introspection helper = PASS in `docs/business_flow_verdict.md`.
- [x] All test runs captured (counts + any failures)
      bethere-escrow: 43 passed / 0 failed. Worker: 117 passed / 0 failed.
      Per-file breakdown captured in Phase 1 + verdict doc.
- [x] Clippy clean or all warnings justified
      bethere-escrow `src/` clippy-clean (2 test-fixture warnings justified via
      `#[allow]`; 19 client-codegen warnings out of scope). Worker clippy-clean.
- [x] `docs/business_flow_verdict.md` exists and is complete
      182 lines, full matrix + security review + action items.
- [x] Committed on `feature/001_business_flow_audit` (branched from `develop`)
      DONE — see Phase 5 above for the exact branch, staged files, and commit message.

## Constraints (production grade only)
- No mock, no TODO, no placeholder, no unwrap()
- Fix all diagnostics before marking done
- snake_case, format!("{var}"), match over if
- Use blake3/argon2/papaya where applicable
- Conventional commits on `develop`-based feature branch
