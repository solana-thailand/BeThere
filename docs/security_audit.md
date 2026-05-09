# Security Audit: BeThere Escrow System

## Audit Info

- **Date**: 2026-05-07
- **Auditor**: Internal review
- **Scope**: On-chain escrow program, backend TX builders, frontend wallet integration, KV store
- **Codebase**: `bethere-escrow/`, `worker/src/solana_escrow.rs`, `worker/src/handlers/deposit.rs`, `frontend-leptos/`
- **Status**: Pre-mainnet audit

---

## Findings Summary

| ID | Severity | Title | Status |
|----|----------|-------|--------|
| SEC-001 | 🔴 Critical | Check-in Gate Enables Complete Fund Theft | ✅ Fixed (Phase 3) |
| SEC-002 | 🟠 High | Escrow-Critical Fields Mutable After On-Chain Init | ✅ Fixed (Phase 1) |
| SEC-003 | 🟡 Medium | No Maximum Deposit Cap | ✅ Fixed (Phase 1) |
| SEC-004 | 🟡 Medium | Archive Doesn't Deactivate On-Chain Escrow | ✅ Fixed (Phase 1) |
| SEC-005 | 🟡 Medium | Explorer Links Hardcoded to Devnet | ✅ Fixed (Phase 2) |
| SEC-006 | 🟢 Low | Duplicate Merkle Tree Field in Form | ✅ Fixed (Phase 2) |
| SEC-007 | 🟢 Info | Worker Cannot Manipulate Funds | Confirmed Safe |
| SEC-008 | 🟢 Info | On-Chain Escrow Fields Immutable After Creation | Confirmed Safe |
| SEC-009 | 🟡 Medium | Token Transfers Use `transfer()` Not `transfer_checked()` | ✅ Fixed (Phase 3) |
| SEC-010 | 🟡 Medium | AttendeeDeposit PDAs Never Closed (Rent Leak) | ✅ Fixed (Phase 4) |
| SEC-011 | 🟡 Medium | No `event_end` Guard on `mark_checked_in` | ✅ Fixed (Phase 3) |
| SEC-012 | 🟠 High | Refund No refund_deadline Upper Bound (Race with claim_forfeited) | ✅ Fixed (Phase 5) |
| SEC-013 | 🟡 Medium | Vault Griefing via External USDC Airdrop Blocks close_event | ✅ Fixed (Phase 5) |
| SEC-014 | 🟡 Medium | No Wallet Network Detection (Wrong Cluster TX Signing) | ✅ Fixed (Phase 6) |

---

## Finding Details

### SEC-001: Check-in Gate Enables Complete Fund Theft

**Severity**: 🔴 Critical
**Status**: ✅ Fixed (Phase 3) — `checked_in` constraint removed from `refund.rs`

**Description**:
The `refund` instruction requires `checked_in == true` on the `AttendeeDeposit` account, which only the organizer can set via `mark_checked_in`. If the organizer refuses to check in attendees, nobody can refund. After the `refund_deadline` passes, the organizer calls `claim_forfeited` and takes all deposits.

**Attack Scenario**:
1. Organizer creates event with $15 USDC deposit
2. 100 attendees deposit → $1,500 USDC locked in vault
3. Event day arrives: organizer doesn't scan anyone (or selectively scans only friends)
4. After `event_end`: attendees attempt refund → `NotCheckedIn` error rejected at the account validation level
5. After `refund_deadline`: organizer calls `claim_forfeited` → takes all $1,500

**Impact**: Total loss of all attendee deposits. Classic rug pull vector.

**Affected Code**:
- `bethere-escrow/src/instructions/refund.rs` L40 — `constraints(attendee_deposit.checked_in()) @ EscrowError::NotCheckedIn` gates refund behind organizer-controlled flag
- `bethere-escrow/src/instructions/mark_checked_in.rs` L15-24 — `has_one(organizer)` ensures only organizer can check people in
- `bethere-escrow/src/instructions/claim_forfeited.rs` L46-55 — organizer claims all un-refunded deposits after deadline

**Recommendation**:
Allow refunds after `event_end` regardless of check-in status. The `checked_in` field still serves a purpose for analytics and NFT eligibility, but should **not** gate refunds. Two approaches:

1. **Preferred**: Remove the `checked_in` constraint from `Refund` accounts entirely. Attendees can always refund after `event_end`. Organizer only forfeits deposits from attendees who didn't even show up (no deposit record) or who attended and didn't refund.
2. **Alternative**: Add a time-based bypass — if `clock.unix_timestamp > event_escrow.event_end()`, skip the `checked_in` check. This requires restructuring the account validation into the instruction handler.

**Effort**: Medium (requires on-chain program change + redeploy)

---

### SEC-002: Escrow-Critical Fields Mutable After On-Chain Init

**Severity**: 🟠 High
**Status**: Open

**Description**:
The backend `update_event` function accepts changes to `organizer_wallet`, `on_chain_event_id`, `deposit_amount_usdc`, and `refund_deadline_hours` even after the escrow has been initialized on-chain. While the on-chain values are immutable (SEC-008), changing the KV values causes TX builders to derive incorrect PDAs, resulting in failed transactions.

**Attack Scenario**:
1. Organizer initializes escrow on-chain with wallet A
2. Organizer changes `organizer_wallet` in KV store to wallet B
3. All subsequent TX builders (deactivate, claim, close) derive PDAs using wallet B
4. Transactions fail on-chain because PDAs don't match wallet A's escrow
5. Result: operational DoS — cannot deactivate, cannot claim, cannot close

**Impact**: Not direct fund theft, but makes the escrow inoperable. Funds may be permanently locked if the organizer cannot resolve the PDA mismatch.

**Affected Code**:
- `worker/src/event_store.rs` L220-352 — `update_event` accepts all fields without escrow-state guards; specifically L332-343 apply `organizer_wallet`, `on_chain_event_id`, and `refund_deadline_hours` unconditionally
- `domain/src/models/event.rs` — `UpdateEventRequest` has no immutability constraints for escrow-bound fields

**Recommendation**:
In `update_event`, before applying changes to escrow-critical fields, check if `escrow_address` is non-empty. If it is, reject changes to:
- `organizer_wallet`
- `on_chain_event_id`
- `deposit_amount_usdc`
- `refund_deadline_hours`

Example guard:

```rust
if !config.escrow_address.trim().is_empty() {
    if req.organizer_wallet.is_some()
        || req.on_chain_event_id.is_some()
        || req.deposit_amount_usdc.is_some()
        || req.refund_deadline_hours.is_some()
    {
        return Err(
            "Cannot change escrow-critical fields after on-chain initialization".to_string()
        );
    }
}
```

**Effort**: Small (backend validation only, no on-chain changes)

---

### SEC-003: No Maximum Deposit Cap

**Severity**: 🟡 Medium
**Status**: Open

**Description**:
There is no upper bound on `deposit_amount_usdc`. An organizer could set it to an extremely high value (e.g., $10,000 USDC). While attendees must sign the transaction and can see the amount, social engineering or trust dynamics could lead to significant individual losses. This finding magnifies SEC-001.

**Attack Scenario**:
1. Organizer sets deposit to $5,000 USDC for a "premium" event
2. Markets the event aggressively with social proof and credibility signals
3. Attendees sign the deposit TX trusting the organizer
4. Organizer rug pulls via SEC-001 (doesn't check in anyone)
5. Organizer claims all deposits via `claim_forfeited`

**Impact**: Magnifies SEC-001 by allowing larger individual losses per attendee.

**Affected Code**:
- `worker/src/handlers/deposit.rs` — no max deposit validation in the backend deposit handler
- `bethere-escrow/src/instructions/create_event.rs` L33-35 — only checks `deposit_amount == 0`, no upper bound

**Recommendation**:
Add a maximum cap (e.g., $1,000 USDC) in both:
1. Backend validation — reject `deposit_amount_usdc > 1000` in `create_event` and `update_event`
2. On-chain `create_event` instruction — add `if deposit_amount > MAX_DEPOSIT_USDC` check

The exact cap value should be a governance decision.

**Effort**: Small (backend validation + single on-chain check)

---

### SEC-004: Archive Doesn't Deactivate On-Chain Escrow

**Severity**: 🟡 Medium
**Status**: Open

**Description**:
The domain `EventStatus::Archived` and the on-chain `is_active` flag are completely independent. Archiving an event in the admin panel does **not** deactivate the escrow on-chain. This means:
- An archived event can still receive deposits
- The frontend may hide archived events from public listing, but the escrow remains live
- Attendees may not realize their deposits are going to an "archived" event

**Attack Scenario**:
1. Organizer creates event, initializes escrow, collects deposits
2. Organizer archives the event in the admin panel
3. Event disappears from public listing
4. But escrow is still active on-chain
5. Organizer can still deactivate, claim forfeited, close — attendees have no visibility into the event status

**Impact**: Information asymmetry. Attendees cannot see the event status, but the escrow remains operational. Combined with SEC-001, this allows the organizer to quietly archive an event, collect deposits from direct links, and rug pull.

**Affected Code**:
- `worker/src/event_store.rs` L355-375 — `archive_event` only sets `status = Archived` in KV, no escrow state check or sync

**Recommendation**:
Before archiving, enforce:
1. Check if escrow exists (`escrow_address` non-empty)
2. If yes, require the escrow to be deactivated on-chain first
3. If yes, require all deposits to be settled (refunded or claimed)
4. Add an unarchive/restore capability for organizers who archived prematurely

```rust
pub async fn archive_event(kv: &KvStore, id: &str) -> Result<(), String> {
    let mut config = get_event_config(kv, id)
        .await?
        .ok_or_else(|| format!("event '{id}' not found"))?;

    if !config.escrow_address.trim().is_empty() {
        return Err(
            "Cannot archive event with active escrow. Deactivate escrow on-chain first.".to_string()
        );
    }

    // ... rest of archive logic
}
```

**Effort**: Medium (backend logic + frontend UX for the new constraint)

---

### SEC-005: Explorer Links Hardcoded to Devnet

**Severity**: 🟡 Medium
**Status**: Open

**Description**:
5 of 6 Solscan explorer links across the frontend are hardcoded to `?cluster=devnet`. Only `claim.rs` is cluster-aware, dynamically reading the cluster from event config. When deploying to mainnet, these 5 links will point to the wrong cluster, showing "transaction not found" to users.

**Affected Code**:

| File | Line | Context |
|------|------|---------|
| `frontend-leptos/src/pages/escrow_init.rs` | L436 | `format!("https://solscan.io/tx/{sig}?cluster=devnet")` |
| `frontend-leptos/src/pages/admin_escrow.rs` | L357 | `format!("https://solscan.io/tx/{sig}?cluster=devnet")` |
| `frontend-leptos/src/pages/scanner.rs` | L1281 | `format!("https://solscan.io/tx/{}?cluster=devnet", signature)` |
| `frontend-leptos/src/pages/deposit.rs` | L1314 | `format!("https://solscan.io/tx/{}?cluster=devnet", tx_sig)` |
| `frontend-leptos/src/pages/deposit.rs` | L1577 | `format!("https://solscan.io/tx/{}?cluster=devnet", tx_sig)` |

**Cluster-aware (correct)**:
- `frontend-leptos/src/pages/claim.rs` L1413-1416 — uses `{cluster_param}` dynamically

**Recommendation**:
Make all explorer links cluster-aware, reading from event config or a global cluster setting. Follow the pattern already established in `claim.rs`:

```rust
let cluster_param = if cluster == "mainnet-beta" {
    String::new()
} else {
    format!("?cluster={}", cluster)
};
let url = format!("https://solscan.io/tx/{}{cluster_param}", signature);
```

**Effort**: Small (frontend-only, follow existing pattern)

---

### SEC-006: Duplicate Merkle Tree Field in Form

**Severity**: 🟢 Low
**Status**: Open

**Description**:
The Merkle Tree input field is rendered twice in the event form. Both fields bind to the same `form.merkle_tree` value. This is confusing for users and suggests a copy-paste bug during a refactor.

**Affected Code**:
- `frontend-leptos/src/pages/events_page.rs` L784-792 — first Merkle Tree input (inside the NFT section)
- `frontend-leptos/src/pages/events_page.rs` L861-870 — second Merkle Tree input (inside a different settings section)

**Recommendation**:
Remove one of the duplicate fields. Decide which placement makes more sense UX-wise and keep only that one.

**Effort**: Tiny (remove one block of ~10 lines)

---

### SEC-007: Worker Cannot Manipulate Funds

**Severity**: 🟢 Info
**Status**: ✅ Confirmed Safe

The Cloudflare Worker backend never holds private keys. All transactions are built unsigned and require the appropriate wallet (organizer or attendee) to sign via the frontend wallet adapter. The worker only constructs instruction data and account lists — it cannot spend, transfer, or redirect funds at any point in the flow.

This is the correct architecture for a non-custodial escrow system.

---

### SEC-008: On-Chain Escrow Fields Immutable After Creation

**Severity**: 🟢 Info
**Status**: ✅ Confirmed Safe

The `EventEscrow` account uses `set_inner()` which sets all fields once during `create_event`. After creation, only the following fields can be modified:
- `total_deposited` — increment-only via checked arithmetic
- `total_refunded` — increment-only via checked arithmetic
- `total_forfeited` — increment-only via checked arithmetic
- `is_active` — one-way transition: `true → false`

Critical fields are permanently immutable after creation:
- `organizer`
- `deposit_amount`
- `event_end`
- `refund_deadline`
- `usdc_mint`
- `vault`
- `event_id`

This is a strong safety property. Once an escrow is created on-chain, the organizer cannot change the rules.

---

### SEC-009: Token Transfers Use `transfer()` Not `transfer_checked()`

**Severity**: 🟡 Medium
**Status**: Open
**Source**: Safe Solana Builder §7 — Token-2022 Compatibility

**Description**:
All 3 token transfer sites in the escrow program use `transfer()` instead of `transfer_checked()`. The `transfer_checked()` instruction validates the mint and decimals at the token program level, while `transfer()` does not. This is incompatible with Token-2022 tokens that use transfer hooks, and represents a defense-in-depth gap even for legacy SPL token usage.

**Affected Code**:
- `bethere-escrow/src/instructions/deposit.rs` L65-68 — `transfer()` in `transfer_usdc()`
- `bethere-escrow/src/instructions/refund.rs` — `transfer()` in vault → attendee refund
- `bethere-escrow/src/instructions/claim_forfeited.rs` — `transfer()` in vault → organizer claim

**Mitigation**: The program validates `usdc_mint` at the instruction level via account constraints, which partially mitigates this. However, `transfer_checked` is the recommended best practice for forward compatibility and defense-in-depth.

**Recommendation**: Replace all `transfer()` calls with `transfer_checked()`, passing the mint account and decimals. This requires adding the `usdc_mint` account to each instruction's account context (it may already be present for validation).

**Effort**: Small (3 call sites)

---

### SEC-010: AttendeeDeposit PDAs Never Closed (Rent Leak)

**Severity**: 🟡 Medium
**Status**: ✅ Fixed (Phase 4)
**Source**: Safe Solana Builder §6.3 (Account Closing), §13 (Token Dust)

**Description**:
The `close_event` instruction closes the `EventEscrow` PDA and the vault token account, but **no instruction existed to close `AttendeeDeposit` PDAs**. After an event completes and all deposits are settled, every attendee's `AttendeeDeposit` PDA remains on-chain forever, permanently locking the rent-exempt SOL (~0.002 SOL each). For an event with 1000 attendees, that's ~2 SOL permanently locked.

**Fix (Phase 4)**:
Added `close_deposit` instruction (discriminator 7) with two close paths:
1. **Self-close**: Attendee closes their own deposit PDA after `refunded == true`
2. **GC close**: Anyone can close deposit PDAs after the parent `EventEscrow` has been closed (detected by `data_len() == 0`)

Includes TX builder in worker (`build_close_deposit_transaction`), public API endpoint (`POST /api/escrow/close-deposit`), and frontend "Reclaim Rent" button on the deposit page.

---

### SEC-011: No `event_end` Guard on `mark_checked_in`

**Severity**: 🟡 Medium
**Status**: Open
**Source**: Safe Solana Builder §26.4 (Status Transition Guards)

**Description**:
The `mark_checked_in` instruction has no time-based guard — the organizer can mark attendees as checked in at any time, even after the event has ended. This creates a selective favoritism risk: an organizer could wait until after the event ends, selectively mark only friends as checked in, and exclude others from refunds.

**Affected Code**:
- `bethere-escrow/src/instructions/mark_checked_in.rs` — no `Clock` sysvar, no `event_end` comparison

**Impact**: Combined with SEC-001 (check-in gates refunds), this allows post-hoc cherry-picking of who gets refunds. The organizer decides after the fact who "attended."

**Recommendation**:
1. Add `constraints(clock.unix_timestamp <= event_escrow.event_end())` to limit check-ins to during the event
2. Or, if SEC-001 is fixed (refunds allowed regardless of check-in), this becomes less critical but still worth adding for protocol integrity

**Effort**: Small (single constraint addition)

---

### SEC-012: Refund No refund_deadline Upper Bound — Race with claim_forfeited

**Severity**: 🟠 High
**Status**: ✅ Fixed (Phase 5)

**Description**: The `refund` instruction only checked `clock >= event_end` (lower bound) but had no upper bound check against `refund_deadline`. Meanwhile, `claim_forfeited` checks `clock >= refund_deadline`. This created an overlap window after `refund_deadline` where both instructions were valid.

**Attack**: Organizer calls `claim_forfeited` immediately after `refund_deadline`, draining the vault. Attendee then attempts refund — time check passes (still after `event_end`), but `transfer_usdc` fails because the vault is empty. The attendee's deposit PDA remains `refunded = false` with no USDC to recover.

**Fix**: Added `clock >= refund_deadline → RefundDeadlinePassed` check to `validate_and_update()`. Refund window is now strictly `[event_end, refund_deadline)`.

### SEC-013: Vault Griefing via External USDC Airdrop Blocks close_event

**Severity**: 🟡 Medium
**Status**: ✅ Fixed (Phase 5)

**Description**: The `close_event` instruction verified vault emptiness via accounting invariant (`total_deposited == total_refunded + total_forfeited`) but did not check the actual vault token balance. Since anyone can transfer tokens to any SPL token account, an attacker could airdrop 1 lamport USDC to the vault.

**Attack**: After event lifecycle completes normally (accounting balances correct), attacker sends 1 micro-USDC to the vault. `close_event` accounting check passes, but SPL Token's `close_account` CPI fails because `amount != 0`. Organizer is permanently stuck — cannot close event or reclaim vault rent (~0.002 SOL).

**Fix**: Added `vault.amount() != 0` check in `close_event()` before the `close_account` CPI. Returns `VaultNotEmpty` if griefing detected.

### SEC-014: No Wallet Network Detection (Wrong Cluster TX Signing)

**Severity**: 🟡 Medium
**Status**: ✅ Fixed (Phase 6)

**Description**: All wallet signing paths (escrow init, deposit, refund, close, check-in, admin escrow actions) had no cluster verification before requesting transaction signatures. A user whose wallet was connected to mainnet could sign a real-value transaction thinking they were on devnet, or vice versa.

**Attack**: App is configured for devnet. User's wallet is set to mainnet. User clicks "Create & Sign" for escrow initialization. The transaction is built against devnet program IDs but signed and broadcast to mainnet — resulting in either failed TX (program doesn't exist) or, in a worst case, interaction with a different program at the same PDA on mainnet.

**Fix**: Added `getWalletCluster()` JS function that detects the wallet's connected cluster via `getGenesisHash` RPC call against the wallet's own RPC endpoint. The `check_wallet_cluster()` Rust function compares this against the app's expected cluster (from `/api/health`) and blocks signing with a clear error message if they don't match. Applied to all 6 signing paths: `escrow_init`, `events_page`, `admin_escrow`, `deposit` (deposit/refund/close), `scanner`.

---

## Safe Solana Builder Cross-Reference Summary

Cross-referenced against [Safe Solana Builder](https://github.com/Frankcastleauditor/safe-solana-builder) by Frank Castle (124⭐, Solana security researcher, 70+ Rust audits, 250+ Critical/High findings).

| # | Rule | Status | Finding |
|---|---|---|---|
| §1.1 | Signer Checks | ✅ Compliant | All instructions verify signers correctly |
| §1.2 | Ownership Checks | ✅ Compliant | Framework enforces owner == program_id |
| §1.4 | Type Cosplay (Discriminators) | ✅ Compliant | EventEscrow=1, AttendeeDeposit=2 |
| §2.1 | Canonical Bumps | ✅ Compliant | Stored at creation, reused in CPIs |
| §2.2 | PDA Sharing Prevention | ✅ Compliant | Seeds unique per organizer+event and event+attendee |
| §3.1 | Checked Math | ✅ Compliant | All arithmetic uses checked_* operations |
| §4.1 | Duplicate Mutable Accounts | ⚠️ Partial | Low — runtime prevents, no explicit constraint |
| §5.2 | Reload After CPI | ✅ Compliant | No stale reads after CPIs |
| §5.3 | Signer Pass-Through | ✅ Compliant | CPIs correctly scoped to PDA signer only |
| §6.3 | Account Closing | ✅ Compliant | SEC-010 fixed: close_deposit instruction added |
| §7 | Token-2022 Compatibility | ✅ Compliant | SEC-009 fixed: transfer_checked() in Phase 3 |
| §13 | Token Dust | ✅ Compliant | SEC-010 fixed: deposit PDAs can be closed for rent |
| §22.1 | Withdrawal Path | ✅ Compliant | All vaults have refund + claim + close paths |
| §26.1 | Sentinel Timestamps | ⚠️ Partial | Low — no upper bound on event_end |
| §26.4 | Status Transition Guards | ✅ Compliant | SEC-011 fixed: event_end guard on mark_checked_in |

**Overall**: The program scores well on core security — strong signer checks, proper checked arithmetic, correct PDA derivation, solid CPI patterns, Token-2022 compatibility (`transfer_checked`), rent reclamation (`close_deposit`), and check-in timing guards (`event_end`). All audit findings resolved across Phases 1–4.

---

## Remediation Priority

| Priority | Finding | Effort | Type | Status |
|----------|---------|--------|------|--------|
| P0 | SEC-001: Check-in gate enables fund theft | Medium | On-chain program | ✅ Fixed (Phase 3) |
| P1 | SEC-002: Field immutability after escrow init | Small | Backend validation | ✅ Fixed (Phase 1) |
| P2 | SEC-003: Deposit cap | Small | Backend + On-chain | ✅ Fixed (Phase 1) |
| P3 | SEC-009: Use transfer_checked() | Small | On-chain program | ✅ Fixed (Phase 3) |
| P4 | SEC-011: event_end guard on mark_checked_in | Small | On-chain program | ✅ Fixed (Phase 3) |
| P5 | SEC-004: Archive guards | Medium | Backend + Frontend | ✅ Fixed (Phase 1) |
| P6 | SEC-005: Explorer links cluster-aware | Small | Frontend | ✅ Fixed (Phase 2) |
| P7 | SEC-010: Close AttendeeDeposit PDAs | Medium | On-chain + Frontend | ✅ Fixed (Phase 4) |
| P8 | SEC-006: Duplicate Merkle field | Tiny | Frontend | ✅ Fixed (Phase 2) |
| P9 | SEC-012: Refund deadline upper bound | Small | On-chain program | ✅ Fixed (Phase 5) |
| P10 | SEC-013: Vault griefing via airdrop | Small | On-chain program | ✅ Fixed (Phase 5) |
| P11 | SEC-014: Wallet cluster mismatch | Small | Frontend (JS + Rust) | ✅ Fixed (Phase 6) |

---

## Scope for Mainnet

**FIXED**: SEC-001, SEC-002, SEC-003, SEC-004, SEC-009, SEC-011 (Phases 1 + 3)

SEC-001 was a direct fund theft vector (organizer rug pull). SEC-002 caused permanent fund lockup risk. SEC-003 amplified SEC-001 impact. SEC-009 was a Token-2022 compatibility blocker. SEC-011 allowed post-hoc check-in manipulation. SEC-004 was an information asymmetry enabling stealth rug pulls. All six are now resolved.

**SHOULD FIX before mainnet**: SEC-005

SEC-005 was explorer links hardcoded to devnet cluster. ✅ Fixed in Phase 2 — all Solscan links now use cluster-aware URLs via `/api/health` endpoint.

**ALL FINDINGS RESOLVED**: All 12 actionable findings (SEC-001 through SEC-014, excluding SEC-007/008 confirmed safe) are now fixed. SEC-010 (rent reclamation) resolved in Phase 4. SEC-012 (refund deadline upper bound) and SEC-013 (vault griefing via airdrop) resolved in Phase 5. SEC-014 (wallet network detection) resolved in Phase 6.

---

## Appendix: Trust Model

The BeThere escrow system previously had a trust assumption where **the organizer was trusted to fairly check in attendees**. SEC-001 encoded this trust in the refund path. This has been resolved (Phase 3) — refunds no longer require `checked_in == true`, removing the organizer rug pull vector entirely. The escrow now functions as a pure no-show deterrent: attendees can always reclaim their deposit after `event_end`, regardless of whether the organizer checked them in.

The `checked_in` field still serves as an analytics signal and NFT eligibility marker, but no longer gates fund access. The `mark_checked_in` instruction now includes an `event_end` guard (SEC-011 fix) to prevent post-event manipulation of attendance records.

---

## Formal Verification (Kani)

- **Date**: 2025-05-09
- **Tool**: Kani 0.67.0 (CBMC 6.8.0 backend)
- **Scope**: Pure arithmetic properties of escrow financial logic
- **Source**: `bethere-escrow/src/kani.rs` — 13 proof harnesses, 489 lines
- **Result**: ✅ **All 13 harnesses pass** — 0 failures across 729 total checks

### Verified Properties

| # | Harness | Property Proven |
|---|---------|----------------|
| 1 | `create_event_rejects_zero_deposit` | `deposit_amount == 0` → always rejected |
| 2 | `create_event_rejects_past_event_end` | `event_end <= now` → always rejected |
| 3 | `create_event_rejects_bad_refund_deadline` | `refund_deadline <= event_end` → always rejected |
| 4 | `create_event_accepts_valid_inputs` | All valid inputs → always accepted |
| 5 | `deposit_overflow_safe` | `checked_add` never wraps, result ≥ old |
| 6 | `refund_overflow_safe` | `checked_add` never wraps, result ≥ old |
| 7 | `claim_forfeited_double_sub_safe` | Double `checked_sub` never underflows |
| 8 | `close_event_invariant` | `deposited == refunded + forfeited` ↔ vault empty |
| 9 | `accounting_conservation` | **Fundamental invariant**: `deposited ≥ refunded + forfeited` |
| 10 | `forfeited_is_non_negative` | Forfeited calc is non-negative for valid states |
| 11 | `claim_then_close_consistent` | After full claim → close invariant holds |
| 12 | `sequential_deposits_monotonic` | Multiple deposits always increase total |
| 13 | `sequential_refunds_monotonic` | Multiple refunds always increase total |

### Methodology

Pure arithmetic functions were extracted from the 5 critical financial instruction handlers (`create_event`, `deposit`, `refund`, `claim_forfeited`, `close_event`) into standalone functions with no Solana-specific dependencies. Kani's symbolic execution engine (`kani::any()`) verifies each property holds for **all possible u64/i64 inputs** — equivalent to exhaustive testing of 2^64 × 2^64 input combinations.

### Limitations

Kani **cannot** verify (covered by SVM integration tests):
- PDA seed correctness → covered by 26 SVM tests
- CPI call success/failure → covered by SVM `transfer_checked` tests
- Account ownership checks → covered by SVM `has_one` constraint tests
- Signer authority → covered by SVM unauthorized signer tests

---

## Security References

External resources and vulnerability patterns relevant to the BeThere escrow.

### Safe Solana Builder

- **Repo**: https://github.com/Frankcastleauditor/safe-solana-builder (124⭐)
- **Author**: Frank Castle (@0xcastle_chain), Solana security researcher, 70+ Rust audits, 250+ Critical/High findings
- **Usage**: Cross-referenced against BeThere escrow in the table above. 8 rules compliant, 4 partial, 1 violation (Token-2022).

### Solana Audit Arena

- **Repo**: https://github.com/Frankcastleauditor/Solana-Audit-Arena (88⭐)
- **Format**: Weekly competition — new Anchor program each Monday, researchers submit findings as GitHub Issues
- **Relevant patterns from past weeks**:

| Arena Finding | Pattern | BeThere Match |
|---------------|---------|---------------|
| Week 1: StakeFlow `instant_unlock` bypasses lockup check | Time-gate bypass | SEC-011: `mark_checked_in` has no `event_end` guard |
| Week 2: MissionX Token-2022 `transfer()` incompatibility | Token-2022 readiness | SEC-009: Escrow uses `transfer()` not `transfer_checked()` |
| Week 3: Zenon uncapped buy causing underflow | Missing bounds | SEC-003: No max deposit cap on `deposit_amount_usdc` |
| Week 1: StakeFlow `UserStake` PDA rent never reclaimed | Rent leak | SEC-010: ✅ Fixed — `close_deposit` instruction reclaims rent |

### Recommendation

Consider submitting the BeThere escrow program (`bethere-escrow/`) as a future Audit Arena target for community review. The Arena's researchers and Frank Castle's validation would provide independent third-party security assessment at no cost.
