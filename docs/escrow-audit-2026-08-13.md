# USDC Escrow — Pre-Mainnet Audit & Go/No-Go (2026-08-13)

Read-only security audit of the on-chain `bethere-escrow` program + the worker's
escrow integration, ahead of custodying real attendee USDC on mainnet.

> **Do NOT modify `bethere-escrow/` casually.** It is audited, byte-matched, and
> deployed on devnet. Any source change to the program (even a rename or dead-code
> delete) changes the binary and invalidates the byte-match — batch the F5/F7/F8
> cleanups below with the **next intentional program build for mainnet**, not as
> standalone commits.

## Verdict

The on-chain program is **well-constructed and defensively coded** — exhaustive
PDA-address binding, checked arithmetic, vault/mint cross-checks on every money
instruction, the SEC-010 refund/close pairing, and the SEC-013 dust-grief guard
all hold. **No path found to drain the vault, double-refund, forfeit a checked-in
deposit, or under-pay a deposit.** The material gaps are off-chain (F1, fixed) and
operational (F2/F3, yours).

## Findings

| # | Sev | Area | Status |
|---|-----|------|--------|
| **F1** | Medium | Deposit verify didn't read back the on-chain deposit PDA → free verified tickets | ✅ **FIXED** (commit d6f990e) |
| **F2** | High (readiness) | Mainnet program undeployed; `ESCROW_PROGRAM_ID_MAINNET` empty (fails loudly — correct) | ⛔ yours |
| **F3** | High (governance) | Upgrade authority is a single EOA → rug vector over every vault | ⛔ yours (Squads multisig) |
| **F4** | Medium | The "99,104 vs 89,856" byte scare is a **stale doc note** — devnet binary *was* proven to match source (2026-08-08). Only the **mainnet** verified-build badge remains. | mostly resolved |
| F5 | Low | `refund` drops `require_distinct`/`validate_version` — safe today (all accounts PDA-address-bound; v1-only). Add a tripwire if a v2 layout is ever introduced. | batch w/ next build |
| F6 | Low/Info | `close_deposit` GC lets anyone reclaim ~0.001 SOL rent *after* event close — by design, cannot be spoofed while escrow is open. | accepted |
| F7 | Info | Duplicated introspection parser; `instructions/introspection.rs` compiled but never called (dead). Collapse/delete + shared test. | batch w/ next build |
| F8 | Info | `create_event` returns `RefundDeadlineNotPassed` for a construction-time check — mislabeled; `InvalidRefundDeadline` reads better. | batch w/ next build |

### F1 — fixed this session
The client-supplied confirm path marked a deposit `verified` when the tx was
confirmed and the attendee was fee-payer — never checking it was a *real* deposit.
So a confirmed-but-unrelated tx (e.g. a 0-value self-transfer) could earn a free
verified ticket. (Cannot steal funds — a refund builds against the real PDA and
fails if none exists.) **Fixed:** `verify_attendee_deposit_onchain()` reads back
the `AttendeeDeposit` PDA and requires owner == escrow program, `amount ==` the
event's deposit, `refunded == false`, before verifying. Fails closed on RPC error.
Pure decoder unit-tested. **Validate against a real devnet deposit before deploy.**

## Reviewed and found SOLID
- Deposit amount is **not client-controlled** — the program uses
  `event_escrow.deposit_amount()`; the worker's `deposit_amount` arg is display-only.
- The **wallet↔deposit refund cross-check** (this session's #2 fix) holds.
- **SEC-010 refund/close pairing**, **claim_forfeited** (checked-in unforfeitable),
  **close_event** dual accounting+balance guard, **rollover** conservation,
  reinit/replay safety, cluster routing (mint+program-id atomic flip).
- SVM adversarial suite covers a solid core (mint/vault mismatch, double-refund,
  refund-without-close, checked-in-bypass both directions, dust-grief, etc.).

**Test gaps to add before mainnet (Phase 2):** SEC-010 spoof (paired close targets
a *different* deposit; close *before* refund) must fail; F1 negative (non-escrow tx
by right fee-payer must NOT verify); multi-attendee accounting conservation;
exact-second clock boundaries; multi-wallet-adapter SEC-010 + SEC-014 cluster guard.

## USDC Mainnet Go/No-Go checklist (ordered)

**Blockers — must be GREEN:**
1. **F3 — upgrade authority → Squads multisig** (or documented immutability). Highest impact.
2. **F1 — validate the read-back against a real devnet deposit**, then deploy the fix.
3. **External audit** of the program + the pinned `quasar_lang` rev (zero-copy soundness is inherited + un-audited).
4. **Build from pinned source + byte-match** (`quasar build` from tracked `Cargo.lock`; keep `verify_devnet_binary.sh` green).

**Deploy sequence:**
5. Finish Phase 1 (all 9 instructions on-chain — done) + Phase 2 adversarial on the byte-matched binary; close the test gaps above.
6. Phase 3 off-chain: PDA read-back (F1 ✅), post-init echo-verify of `event_end`/`refund_deadline`/`deposit_amount`, blockhash timing, indexer + idempotency.
7. Generate + fund the mainnet program keypair (~1.5 SOL), deploy `bethere_escrow.so`, set `ESCROW_PROGRAM_ID_MAINNET` (`worker/src/solana_escrow/mod.rs`), re-run `cluster_selection_tests`. **Batch F5/F7/F8 cleanups into this build.**
8. `solana-verify build` → publish the mainnet verified-PDA badge (on-chain == source).
9. Register the mainnet Helius webhook; rehearse deploy + `wrangler rollback`; smoke-test served Content-Types (not just HTTP 200).

**Cutover (no native % canary — every deploy is 100%-at-once via PUT):**
10. Deploy mainnet-capable code first with `SOLANA_CLUSTER` still devnet, *then* flip `SOLANA_CLUSTER=mainnet-beta`.
11. Canary one low-value real event end-to-end (init → deposit → check-in → refund+close → deactivate → close) with a proven rollback; confirm F1 rejects a spoofed confirm.

See also: `docs/mainnet_readiness_runbook.md`, `docs/mainnet_canary_mitigation_runbook.md`.
