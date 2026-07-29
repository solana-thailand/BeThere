# Mainnet Readiness Runbook — Devnet Validation → Mainnet Confidence

> **Purpose.** A single, ordered path to sufficient confidence before deploying the
> `bethere-escrow` program to **mainnet-beta** with real funds. It consolidates the
> 2026-07-28 readiness assessment and ties together the detailed runbooks
> (`devnet_testing_guide.md`, `devnet_e2e_walkthrough.md`, `staging_deploy_runbook.md`,
> `mainnet_canary_mitigation_runbook.md`, `gradual_deploy_runbook.md`).
>
> **Golden rule:** treat NO devnet result as evidence until the deployed program byte-matches
> the current pinned source (Phase 0). The devnet program has historically drifted from source.

Key facts (verify before trusting):
- Program ID (devnet): `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`
- Upgrade authority (single key today): `9Bz7p4RWdX7eaR4hFUeCc7aSZjDHsie8q1u8imwavkBN`
- USDC mint — devnet `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`, mainnet `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m`
- `ESCROW_PROGRAM_ID_MAINNET` is empty (`worker/src/solana_escrow/mod.rs`) — mainnet program not deployed.

---

## Already done (code side — no keys needed)

- ✅ **Reproducible build** — `bethere-escrow/Cargo.lock` tracked, quasar deps rev-pinned (PR #46).
- ✅ **Adversarial SVM tests** — 9 money-path reject tests (mint/vault mismatch, refund-before-end,
  no-show-after-deadline, checked-in bypass, rollover integrity, event-ended check-in, vault dust-grief)
  run the real program (PR #47).
- ✅ **Kani model reconciled** with the real refund handler (checked-in bypass) (PR #48).
- ✅ **CI enforces the SVM suite** — `quasar test` runs on every change (PR #49). *Kani in CI is still
  pending (heavy toolchain).*

These make the following phases *trustworthy*; they do not replace them.

---

## Phase 0 — Make devnet trustworthy (foundational, BLOCKER)

The on-chain programdata was 99,104 bytes vs a local build of 89,856 bytes — the deployed binary is
**not** the current source, so all prior devnet testing may have exercised an old build.

- [ ] **0.1 Build from pinned source:** `cd bethere-escrow && quasar build`
- [ ] **0.2 Redeploy to devnet** (needs the program keypair + a funded devnet deployer):
      `quasar deploy --url devnet --program-keypair <program-keypair.json> -k <payer.json>`
- [ ] **0.3 Prove `binary == source`:** `bash scripts/verify_devnet_binary.sh` (read-only, no keypair).
      It dumps the deployed program and compares correctly — the first `len(local)` bytes must be byte-identical
      **and** everything after must be zero padding. (A naive length/`sha256sum` compare *falsely fails*: `solana
      program dump` returns the full allocated programdata zero-padded to `max_len`, which is why the on-chain
      99,104 B ≠ local 89,856 B above is padding, not a mismatch.) Add `--build` to `quasar build` from pinned
      source first. On success it prints the source sha256 to record in `docs/audit_submission.md`.
- [ ] **0.4 Cluster-selection defect (worker):** `usdc_mint()` / `escrow_program_id()` read
      `std::env::var("SOLANA_CLUSTER")`, which is empty in the wasm Workers runtime → always devnet.
      Fix to read the `Env` binding + add `SOLANA_CLUSTER` to `wrangler.toml` and the `deploy.sh` PUT
      metadata, then assert (via `/api/health` or a log) that the mint + program ID used at tx-build time
      are the devnet values. *(Tracked as code piece #4 — assign to the code stream, not this runbook.)*

**Gate:** do not record any Phase 1+ result until 0.3 passes.

---

## Phase 1 — Full functional lifecycle on devnet

Exercise **all 9 instructions** against the freshly-matched program — the automated script only covers
create→deposit→checkin→refund today; 5 instructions have never run on-chain from CI/scripts.

- [ ] **1.1** Full escrow lifecycle: `bash scripts/e2e/test_escrow_devnet.sh`
      (create_event → deposit → mark_checked_in → refund+close_deposit → claim_forfeited →
      deactivate_event → close_event). Capture every tx signature + Solscan link.
- [ ] **1.2** Rollover (highest-risk multi-vault path): `bash scripts/e2e/test_rollover_devnet.sh`
      and `bash scripts/e2e/test_rollover_full_lifecycle.sh`.
- [ ] **1.3** Confirm rent is reclaimed on close, and re-derive a known escrow PDA to check it matches
      the worker's `derive_escrow_address`.
- [ ] **1.4** Make the **flow-harness** actually run (replace the `// TODO(staging-live)` stubs) so
      `cargo run -p flow-harness` against a devnet staging worker produces `flow-harness/results/.last-green`.
      *(Code piece #5.)* See `docs/staging_deploy_runbook.md` to stand up the DEV_MODE=1 staging worker
      first (the e2e script currently defaults to the prod URL, which is `dev_mode:false`).

**Gate:** every instruction confirmed on-chain with a captured signature.

---

## Phase 2 — Adversarial / edge validation on devnet (the "sufficiently sure" core)

These are proven in the SVM suite now, but must be confirmed against the **deployed bytecode** — that is
what "sure on devnet" means. Assert the exact on-chain error each time.

- [ ] **2.1** Deposit: wrong mint → `MintMismatch`; wrong/foreign vault ATA → `VaultMismatch`.
- [ ] **2.2** Refund: before `event_end` → `RefundNotYetAllowed`; **no-show** after `refund_deadline` →
      `RefundDeadlinePassed`; **checked-in** after `refund_deadline` → **succeeds** (the bypass).
- [ ] **2.3** Double-refund → `AlreadyRefunded`; refund WITHOUT the paired `close_deposit` → `RefundRequiresClose`.
- [ ] **2.4** `claim_forfeited` of a **checked-in** attendee → `AttendeeCheckedIn` (never forfeitable).
- [ ] **2.5** Rollover value-integrity: target event different mint → `MintMismatch`; different amount →
      `IncorrectDepositAmount`.
- [ ] **2.6** **Vault dust-griefing:** airdrop 1 micro-USDC to a settled vault, then `close_event` →
      `VaultNotEmpty`; confirm the documented recovery path still lets the organizer close.
- [ ] **2.7** Clock boundaries with a **short-lived** real event (event_end / refund_deadline minutes out):
      refund/checkin at the exact boundaries (SVM only tests warped ±1).
- [ ] **2.8** Multi-attendee accounting conservation: N attendees mixing refund / forfeit / rollover, then
      read the escrow and assert `total_deposited == total_refunded + total_forfeited` and vault == 0 before close.
- [ ] **2.9** Load / CU headroom: 50–100 concurrent deposits to one event on a **private (Helius) devnet RPC**
      (public `api.devnet.solana.com` is rate-limited); watch for blockhash-expiry, PDA-init races,
      and refund's inlined sysvar-scan CU cost under many sibling instructions.
- [ ] **2.10** Multi-wallet-adapter: run the refund+close pairing (SEC-010) via Phantom, Solflare, and
      Backpack — some reorder/strip instructions; confirm none trips `RefundRequiresClose` (#22).
- [ ] **2.11** SEC-014 cluster-mismatch guard: app on devnet, wallet on mainnet → signing blocked with a
      clear error.

**Gate:** every reject returns the *expected* error code (not a wrong-reason failure).

---

## Phase 3 — Off-chain integration validation on devnet

- [ ] **3.1** Deposit verification reads back the **on-chain `AttendeeDeposit` PDA** (amount + mint + state),
      not just "a confirmed tx signed by the right wallet exists".
- [ ] **3.2** After `confirm-init`, read back the `EventEscrow` and assert `event_end` / `refund_deadline` /
      `deposit_amount` match what the worker intended (catches the silent 7-day-default horizon in
      `init_escrow_tx_handler`).
- [ ] **3.3** Blockhash timing: measure build→land elapsed with a real 10–40s wallet-sign delay; confirm the
      30s-cached, `finalized`-commitment blockhash never causes `BlockhashNotFound`.
- [ ] **3.4** Helius indexer: register the devnet enhanced webhook against the program, drive a deposit, and
      confirm `escrow_indexer` populates D1 for MarkCheckedIn / Refund / ClaimForfeited; test the indexer-lag path.
- [ ] **3.5** Idempotency / drift recovery: kill the worker between deposit and confirm; verify
      `recover_and_verify_deposit` self-heals via `getSignaturesForAddress`; call confirm-init twice → same result.
- [ ] **3.6** Negative/tamper: deposit signed by a different wallet → `signer_matched=false`, `verified` stays
      false; replay another attendee's signature → rejected; wrong USDC amount → not verified.

---

## Phase 4 — Mainnet gates (devnet cannot cover — governance / ops)

These are the true long-poles. **None of them are validated by devnet**, and each is required for real funds.

- [ ] **4.1 External security audit** — submit the prepared `docs/audit_submission.md` package (Audit Arena).
      This is the long pole; devnet validation does **not** substitute for it. Note the program is built on the
      pre-release **Quasar** framework (unaudited codegen) — flag that to the auditor.
  - [ ] **4.1a Framework soundness is in audit scope.** Our program contains **no `unsafe`** — the entire
        zero-copy unsafe-soundness burden lives in `quasar_lang`. Per the [zero-copy safety model]
        (https://quasar-lang.com/docs/zero-copy/safety-model), Quasar **v0.1.0 is explicitly un-audited** and its
        soundness rests on **Tree Borrows** aliasing semantics (an "accepted risk", validated only under
        `-Zmiri-tree-borrows`, which cannot cover SBF syscalls/FFI). Because we **pin** the framework
        (`Cargo.lock` + rev-pinned deps, PR #46), we inherit exactly that commit's soundness. The auditor must
        therefore review **the pinned `quasar_lang` version**, not just our handlers — record the exact rev in the
        submission. On our side the safety-model do's are met and re-checkable: non-zero discriminators
        (`EventEscrow=1`/`AttendeeDeposit=2`), `close(dest=…)` zeroes discriminators (revival protection),
        explicit `_padding` (stable layout), `require_distinct` (10 handlers, dup/aliasing guard),
        `validate_version` (8 handlers).
- [ ] **4.2 Upgrade-authority hardening** — move the upgrade authority to a **Squads multisig** (or publish an
      immutability timeline). A single EOA upgrade key on mainnet is a live rug vector for every event vault.
      Document custody + backup of the program-id and upgrade keys.
- [ ] **4.3 Mainnet program** — generate + fund the mainnet program keypair (~1.5 SOL rent), deploy
      `bethere_escrow.so`, set `ESCROW_PROGRAM_ID_MAINNET` in `worker/src/solana_escrow/mod.rs`.
- [ ] **4.4 Reproducible verified build** — install a container runtime (colima/docker), run `solana-verify build`
      and publish the explorer verify-PDA badge, proving on-chain == source publicly.
- [ ] **4.5 Mainnet indexer** — register a mainnet Helius webhook with its own signing secret.
- [ ] **4.6 Deploy/rollback rehearsal** — on devnet/staging, exercise `worker/deploy.sh` including the
      Wrangler-10013 PUT-API fallback and a `wrangler rollback`; verify served asset Content-Types
      (the 2026-07-26 octet-stream incident — see `handover 132`). See `mainnet_canary_mitigation_runbook.md`:
      because of CF bug 10013 there is **no native % canary** — every deploy is 100%-at-once via PUT, so
      Path A (decouple code-deploy from the `SOLANA_CLUSTER` cutover) + a proven rollback are the only controls.

---

## Sign-off gate (all must be true before mainnet)

- [ ] Phase 0 — devnet binary proven == pinned source; cluster selection fixed + verified.
- [ ] Phase 1 — all 9 instructions confirmed on-chain; flow-harness green (`.last-green` exists).
- [ ] Phase 2 — every adversarial/edge case returns the expected on-chain error; load + boundary + multi-adapter clean.
- [ ] Phase 3 — deposit verification reads the PDA; horizons echo-verified; indexer + idempotency proven.
- [ ] Phase 4 — external audit complete; upgrade authority = multisig/immutable; mainnet program deployed +
      verified-build badge; deploy+rollback rehearsed.

> Reference detail lives in: `devnet_testing_guide.md` (Flows A–D), `devnet_e2e_walkthrough.md` (Flows 1–10),
> `staging_deploy_runbook.md`, `mainnet_canary_mitigation_runbook.md`, `gradual_deploy_runbook.md`,
> `audit_submission.md`. This runbook is the index/gate over them.
