# Security Audit: BeThere Escrow System

## Audit Info

- **Date**: 2025-05-07
- **Auditor**: Internal review
- **Scope**: On-chain escrow program, backend TX builders, frontend wallet integration, KV store
- **Codebase**: `bethere-escrow/`, `worker/src/solana_escrow.rs`, `worker/src/handlers/deposit.rs`, `frontend-leptos/`
- **Status**: Pre-mainnet audit

---

## Findings Summary

| ID | Severity | Title | Status |
|----|----------|-------|--------|
| SEC-001 | 🔴 Critical | Check-in Gate Enables Complete Fund Theft | Open |
| SEC-002 | 🟠 High | Escrow-Critical Fields Mutable After On-Chain Init | Open |
| SEC-003 | 🟡 Medium | No Maximum Deposit Cap | Open |
| SEC-004 | 🟡 Medium | Archive Doesn't Deactivate On-Chain Escrow | Open |
| SEC-005 | 🟡 Medium | Explorer Links Hardcoded to Devnet | Open |
| SEC-006 | 🟢 Low | Duplicate Merkle Tree Field in Form | Open |
| SEC-007 | 🟢 Info | Worker Cannot Manipulate Funds | Confirmed Safe |
| SEC-008 | 🟢 Info | On-Chain Escrow Fields Immutable After Creation | Confirmed Safe |

---

## Finding Details

### SEC-001: Check-in Gate Enables Complete Fund Theft

**Severity**: 🔴 Critical
**Status**: Open

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

## Remediation Priority

| Priority | Finding | Effort | Type |
|----------|---------|--------|------|
| P0 | SEC-001: Check-in gate enables fund theft | Medium | On-chain program |
| P1 | SEC-002: Field immutability after escrow init | Small | Backend validation |
| P2 | SEC-003: Deposit cap | Small | Backend + On-chain |
| P3 | SEC-004: Archive guards | Medium | Backend + Frontend |
| P4 | SEC-005: Explorer links cluster-aware | Small | Frontend |
| P5 | SEC-006: Duplicate Merkle field | Tiny | Frontend |

---

## Scope for Mainnet

**MUST FIX before mainnet**: SEC-001, SEC-002, SEC-003

SEC-001 is a direct fund theft vector. SEC-002 can cause permanent fund lockup. SEC-003 amplifies the impact of SEC-001. None of these three should ship to mainnet in the current state.

**SHOULD FIX before mainnet**: SEC-004, SEC-005

SEC-004 is an information asymmetry issue that enables stealth rug pulls when combined with SEC-001. SEC-005 will break the user experience on mainnet (broken explorer links).

**NICE TO FIX**: SEC-006

Cosmetic issue with no security impact.

---

## Appendix: Trust Model

The BeThere escrow system has an inherent trust assumption: **the organizer is trusted to fairly check in attendees**. SEC-001 exists because the current design encodes this trust in the refund path. The recommended fix removes this trust assumption by allowing refunds regardless of check-in status, which aligns with the escrow's purpose as a no-show deterrent rather than an organizer-controlled gate.

If the product intent is for the organizer to have complete control over refunds, that should be an explicit design decision documented and communicated to depositors before they sign.
