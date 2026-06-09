# BeThere — Protocol POC Requirements

> Deposit-backed event check-in protocol on Solana.
> Program ID: `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`
> Cluster: Devnet (deployed) / Mainnet-Beta (configured)
> Status: **POC Complete — Devnet Deployed & Tested**
>
> **Companion document:** [Solana Protocol Architecture](solana_protocol_architecture.md) — diagrams and visual flows for all requirements below.

---

## 1. Event Lifecycle Requirements

The protocol shall allow an organizer to create an on-chain event escrow with a fixed USDC deposit amount, event end timestamp, and refund deadline.

The protocol shall allow an organizer to choose event format — In-Person, Online, or Hybrid — which determines whether on-chain escrow is created.

The protocol shall only create on-chain escrow PDAs for In-Person and Hybrid events. Online-only events shall have no on-chain accounts.

The protocol shall allow an organizer to deactivate an event escrow, which stops accepting new deposits while preserving existing deposits and refund eligibility.

The protocol shall allow an organizer to close an event escrow and reclaim all rent-exempt SOL after all deposits have been settled (refunded or forfeited).

The protocol shall enforce that the deposit amount, event end time, and refund deadline are immutable after escrow creation.

---

## 2. Deposit Requirements

The protocol shall allow an attendee to deposit USDC tokens into a vault owned by the event escrow PDA via the SPL Token program.

The protocol shall accept deposits only in the USDC mint configured at escrow creation (devnet: `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`, mainnet: `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m`).

The protocol shall reject any deposit where the transferred amount does not exactly match the escrow's configured `deposit_amount`.

The protocol shall reject deposits when the escrow `is_active` flag is false.

The protocol shall reject deposits when the event has already ended (`current_time > event_end`).

The protocol shall create one `AttendeeDeposit` PDA per attendee per event with seeds `["deposit", event_escrow_pubkey, attendee_pubkey]`.

The protocol shall track `total_deposited` as a running sum across all attendee deposits in the `EventEscrow` account.

The protocol shall also accept off-chain THB (Thai Baht) deposits via PromptPay bank transfer with admin-verified payment slip, stored off-chain in D1 and R2.

---

## 3. Check-In Requirements

The protocol shall allow the organizer (authority) to mark an attendee as checked in by setting `AttendeeDeposit.checked_in = true`.

The protocol shall enforce that only the organizer designated at escrow creation can sign the `mark_checked_in` instruction (has_one authority constraint).

The protocol shall reject check-in attempts after the event has ended (`current_time > event_end`).

The protocol shall allow one check-in per attendee per event (idempotent — re-checking in has no effect).

For online attendees, the protocol shall not require on-chain check-in. Attendance shall be verified off-chain via quiz or adventure challenge completion.

---

## 4. Refund Requirements

The protocol shall allow an attendee to claim a full refund of their deposited USDC after the event end timestamp, regardless of check-in status (anti-rug-pull guarantee).

The protocol shall transfer USDC from the escrow vault back to the attendee's associated token account.

The protocol shall set `AttendeeDeposit.refunded = true` and increment `EventEscrow.total_refunded` upon successful refund.

The protocol shall reject double refunds — an attendee whose `refunded = true` cannot refund again.

The protocol shall support a combined atomic transaction (`refund` + `close_deposit`) so the attendee signs once and receives both USDC and rent-exempt SOL in a single transaction.

The protocol shall allow checked-in attendees to refund anytime after `event_end` with no deadline.

The protocol shall allow non-checked-in attendees to refund after `event_end` but before `refund_deadline` — after which their deposit may be forfeited.

For THB deposits, the protocol shall process refunds manually off-chain, with admin-uploaded refund proof (transfer receipt) for auditability.

---

## 5. Forfeiture Requirements

The protocol shall allow the organizer to claim forfeited deposits from no-show attendees after the `refund_deadline` has passed.

The protocol shall reject forfeiture claims on attendees who were checked in (`checked_in = true`) — their deposits are protected.

The protocol shall reject forfeiture claims before the `refund_deadline` has elapsed.

The protocol shall reject forfeiture claims on already-refunded deposits.

The protocol shall transfer forfeited USDC from the escrow vault to the organizer's associated token account.

The protocol shall track `total_forfeited` as a running sum in the `EventEscrow` account.

The protocol shall enforce the invariant: `total_deposited == total_refunded + total_forfeited + vault_balance`.

---

## 6. NFT Claim Requirements

The protocol shall mint a compressed NFT (cNFT) to an attendee's Solana wallet as proof of attendance via the Helius DAS API.

The protocol shall only allow NFT minting for attendees who have been checked in (in-person) or have completed the quiz/adventure challenge (online).

The protocol shall prevent double-claiming — once an NFT has been minted for an attendee, no second mint is allowed.

The protocol shall use the Helius `mintCompressedNft` JSON-RPC method for cNFT minting, with optional collection mint and metadata URI.

The protocol shall record the claim transaction signature and asset ID in the off-chain database for verification.

---

## 7. Rollover Requirements

The protocol shall allow a checked-in attendee's deposit to be rolled over from one event escrow to another (same organizer, same deposit amount).

The protocol shall perform an atomic vault-to-vault USDC transfer, closing the source `AttendeeDeposit` PDA and creating a new one for the target event.

The protocol shall require both source and target escrows to belong to the same organizer.

The protocol shall require both escrows to have the same `deposit_amount`.

---

## 8. Account Management Requirements

The protocol shall derive the `EventEscrow` PDA with seeds `["escrow", organizer_pubkey, event_id_u64_le]`.

The protocol shall derive each `AttendeeDeposit` PDA with seeds `["deposit", event_escrow_pubkey, attendee_pubkey]`.

The protocol shall create a vault as an Associated Token Account owned by the `EventEscrow` PDA, holding USDC deposits.

The protocol shall allocate 192 bytes for `EventEscrow` accounts and 96 bytes for `AttendeeDeposit` accounts, both including reserved padding for future schema migrations.

The protocol shall enforce schema version validation on both `EventEscrow` and `AttendeeDeposit` accounts to support future migrations.

---

## 9. Security Requirements

The protocol shall enforce that only the designated organizer can invoke `mark_checked_in`, `claim_forfeited`, `deactivate_event`, and `close_event`.

The protocol shall enforce time-based refund eligibility to prevent organizer rug pulls — attendees can always self-refund after `event_end`.

The protocol shall validate vault and mint accounts match the escrow's stored references before any token transfer.

The protocol shall reject arithmetic overflow on all running sum operations (`total_deposited`, `total_refunded`, `total_forfeited`).

The protocol shall pair `refund` with `close_deposit` in the same transaction (instruction introspection) to prevent rent leaks from abandoned deposit PDAs.

The protocol shall reject event creation with `event_end` in the past or `deposit_amount` of zero.

The protocol shall reject closing an event escrow while the vault still holds tokens — all funds must be settled first.

---

## 10. Online / Quest Requirements

The protocol shall allow online event attendees to RSVP without deposit, wallet, or on-chain interaction.

The protocol shall verify online attendee participation via quiz completion (configurable passing score, max attempts) or adventure challenge completion (multi-level).

The protocol shall treat quest/adventure completion as equivalent to physical check-in for NFT claim eligibility.

The protocol shall store quiz questions, quiz progress, adventure config, and adventure progress in D1 (primary) with KV fallback.

The protocol shall not create any on-chain accounts for online-only attendees.

---

## 11. Indexing Requirements

The protocol shall index all on-chain escrow transactions via Helius enhanced transaction webhooks (primary) and RPC polling fallback (`getSignaturesForAddress`).

The protocol shall parse instruction discriminators (0–8) to identify the instruction type for each indexed transaction.

The protocol shall extract event metadata (organizer, attendee, amount, escrow address) from indexed transactions and store in D1.

The protocol shall update off-chain deposit verification status based on confirmed on-chain deposit transactions.

---

## 12. Dual-Track Payment Requirements

The protocol shall support USDC on-chain deposits (trustless, self-serve refund) for attendees with Solana wallets.

The protocol shall support THB off-chain deposits (PromptPay bank transfer + admin-verified slip) for attendees without Solana wallets.

The protocol shall store THB payment slip images in R2 and verification status in D1.

The protocol shall require admin approval for THB deposits (slip verification) and admin action for THB refunds (manual bank transfer with proof).

The protocol shall support rolling deposit credits (CreditTHB, CreditUSDC) allowing attendees to carry deposits forward to future events instead of refunding.

---

## 13. Platform Fee (Future)

The protocol shall charge a platform fee on forfeited deposits only (not on refunds), collected during `claim_forfeited` instruction execution.

The protocol shall route the platform fee to a separate treasury vault PDA derived from a protocol-owned seed.

The protocol shall allow the fee rate to be configured between 2–5% (TBD), set at the program level, not per-event.

> **Note:** This is planned for a future upgrade. Current POC does not collect platform fees — organizer receives 100% of forfeited deposits.

---

## 14. Event Format Behavior Matrix

| Requirement | In-Person | Online | Hybrid |
|---|---|---|---|
| On-chain escrow PDA | ✅ Created | ❌ None | ✅ Created (in-person track only) |
| USDC deposit required | ✅ Mandatory | ❌ Not applicable | ✅ In-person attendees only |
| THB deposit accepted | ✅ Optional | ❌ Not applicable | ✅ In-person attendees only |
| On-chain check-in | ✅ QR scan + mark_checked_in | ❌ N/A | ✅ In-person: QR scan |
| Off-chain quest verification | ❌ N/A | ✅ Quiz / adventure | ✅ Online: quiz / adventure |
| NFT claim eligibility | After physical check-in | After quest completion | After respective track completion |
| Refund available | ✅ Self-serve on-chain | ❌ N/A | ✅ In-person: self-serve on-chain |
| Forfeiture possible | ✅ After refund deadline | ❌ N/A | ✅ In-person track only |
| Rollover available | ✅ Same organizer | ❌ N/A | ✅ In-person deposits only |

---

## 15. Instruction Set Summary

| # | Instruction | Signer | Trigger | On-Chain Effect |
|---|---|---|---|---|
| 0 | `create_event` | Organizer | Event setup | Initialize EventEscrow PDA + vault ATA |
| 1 | `deposit` | Attendee | Registration | USDC → vault, create AttendeeDeposit PDA |
| 2 | `mark_checked_in` | Organizer | QR scan at event | Set `checked_in = true` on AttendeeDeposit |
| 3 | `refund` | Attendee | After event_end | Vault → attendee USDC ATA, set `refunded = true` |
| 4 | `claim_forfeited` | Organizer | After refund_deadline | Transfer no-show deposits to organizer |
| 5 | `close_event` | Organizer | After settlement | Reclaim rent, close EventEscrow PDA |
| 6 | `deactivate_event` | Organizer | Registration close | Set `is_active = false` |
| 7 | `close_deposit` | Anyone | After refund or event close | Close AttendeeDeposit PDA, reclaim rent |
| 8 | `rollover_deposit` | Attendee | Cross-event transfer | Atomic vault-to-vault USDC transfer |

---

## 16. Account Schema

### EventEscrow (PDA — one per event)

| Field | Type | Size | Description |
|---|---|---|---|
| discriminator | u8 | 1 | Account type discriminator |
| version | u8 | 1 | Schema version (currently v1) |
| organizer | Pubkey | 32 | Event organizer authority |
| event_id | u64 | 8 | Unique event identifier |
| deposit_mint | Pubkey | 32 | USDC mint address |
| vault | Pubkey | 32 | Vault ATA address |
| deposit_amount | u64 | 8 | Fixed deposit amount (USDC lamports) |
| event_end | i64 | 8 | Event end timestamp (unix) |
| refund_deadline | i64 | 8 | Deadline for no-show refunds (unix) |
| total_deposited | u64 | 8 | Running sum of all deposits |
| total_refunded | u64 | 8 | Running sum of all refunds |
| total_forfeited | u64 | 8 | Running sum of all forfeitures |
| is_active | bool | 1 | Whether deposits are accepted |
| bump | u8 | 1 | PDA bump seed |
| _padding | [u8; 36] | 36 | Reserved for future fields |
| **Total** | | **192** | |

### AttendeeDeposit (PDA — one per attendee per event)

| Field | Type | Size | Description |
|---|---|---|---|
| discriminator | u8 | 1 | Account type discriminator |
| version | u8 | 1 | Schema version (currently v1) |
| attendee | Pubkey | 32 | Attendee wallet address |
| event | Pubkey | 32 | EventEscrow reference |
| amount | u64 | 8 | Deposit amount (USDC lamports) |
| deposited_at | i64 | 8 | Deposit timestamp (unix) |
| checked_in | bool | 1 | Whether attendee checked in |
| refunded | bool | 1 | Whether deposit was refunded |
| bump | u8 | 1 | PDA bump seed |
| _padding | [u8; 11] | 11 | Reserved for future fields |
| **Total** | | **96** | |

---

## 17. Error Codes

| Code | Name | Condition |
|---|---|---|
| 0 | `IncorrectDepositAmount` | Deposit amount ≠ escrow.deposit_amount |
| 1 | `RefundNotYetAllowed` | current_time < event_end |
| 2 | `NotCheckedIn` | Attendee not checked in (legacy — relaxed by anti-rug-pull) |
| 3 | `RefundDeadlineNotPassed` | Organizer claiming before refund_deadline |
| 4 | `AlreadyRefunded` | Double refund attempt |
| 5 | `AttendeeCheckedIn` | Cannot forfeit a checked-in attendee's deposit |
| 6 | `NoForfeitedFunds` | Amount to claim is zero |
| 7 | `EventNotActive` | Deposits rejected (is_active = false) |
| 8 | `EventStillActive` | Cannot close while is_active = true |
| 9 | `Unauthorized` | Wrong signer for instruction |
| 10 | `VaultMismatch` | Vault account ≠ escrow.vault |
| 11 | `MintMismatch` | Mint ≠ escrow.deposit_mint |
| 12 | `InvalidDepositAmount` | deposit_amount = 0 at creation |
| 13 | `EventEndInPast` | event_end is not in the future |
| 14 | `Overflow` | Arithmetic overflow on running sums |
| 15 | `VaultNotEmpty` | Close attempted with funds remaining |
| 16 | `EventEnded` | Check-in after event_end |
| 17 | `DepositNotRefunded` | Close attempted on unrefunded deposit |
| 18 | `EventEscrowStillActive` | Deposit still has unrefunded funds |
| 19 | `RefundDeadlinePassed` | No-show's refund window has closed |
| 20 | `EscrowVersionMismatch` | Unsupported EventEscrow schema version |
| 21 | `DepositVersionMismatch` | Unsupported AttendeeDeposit schema version |
| 22 | `RefundRequiresClose` | refund not paired with close_deposit in same TX |

---

## 18. Economics

### For Organizer

- **Revenue:** Forfeited deposits from no-show attendees (100% in POC, minus platform fee post-launch)
- **Cost:** SOL for rent-exempt PDAs (~0.002 SOL per EventEscrow + ~0.001 SOL per AttendeeDeposit)
- **No-show rate target:** Reduce from 40–60% to <15% via deposit commitment mechanism

### For Attendee

- **Cost if they show up:** Zero — full USDC refund after check-in
- **Cost if they no-show:** Full deposit amount forfeited to organizer after refund_deadline
- **Transaction fees:** Paid by attendee's wallet (SOL for transaction, no protocol fee)

### For Platform (Future)

- **Revenue:** Protocol fee on forfeited deposits only (2–5%, TBD)
- **No fee on:** Refunds, deposits, check-ins, NFT minting
- **Aligned incentives:** Platform only profits when organizers profit — honest events generate zero platform revenue

---

## 19. Testing & Verification

The protocol has been tested using:

| Layer | Method | Coverage |
|---|---|---|
| On-chain program | Quasar SVM (local validator) | Unit tests for all 9 instructions |
| On-chain program | Kani formal verification | Mathematical proof of overflow safety |
| Backend (worker) | Rust unit tests | Deposit, quiz, adventure, claim flows |
| Backend (worker) | Playwright E2E | Full attendee journey on devnet |
| Integration | Surfpool | Local Solana + worker integration |
| Devnet | Live deployment | Real transactions on Solana devnet |

---

> **See also:** Architecture diagrams for [PDA Seed Derivation (§9)](solana_protocol_architecture.md#9-pda-derivation--seed-hierarchy) and [Instruction Lifecycle Flow (§3)](solana_protocol_architecture.md#3-program-instructions--lifecycle-flow).

## Appendix A — Program-Derived Address Derivation

```
EventEscrow PDA:
  seeds = [b"escrow", organizer_pubkey, event_id_u64_le_bytes]
  program_id = C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T

AttendeeDeposit PDA:
  seeds = [b"deposit", event_escrow_pubkey, attendee_pubkey]
  program_id = C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T

Vault ATA:
  owner = EventEscrow PDA
  mint = USDC (devnet or mainnet)
  derived via Associated Token Program
```

## Appendix B — Instruction Ordering Constraints

```
create_event → deposit (N times) → deactivate_event
                                   ↓
                              mark_checked_in (N times)
                                   ↓
                              refund | refund_and_close (N times, after event_end)
                                   ↓
                              claim_forfeited (after refund_deadline)
                                   ↓
                              close_event (after full settlement)
```

## Appendix C — Cluster Configuration

| Cluster | Program ID | USDC Mint | RPC |
|---|---|---|---|
| Devnet | `C6HDeZES9aPpNwe3...` | `4zMMC9srt5Ri5X14...` | `devnet.helius-rpc.com` |
| Mainnet-Beta | `C6HDeZES9aPpNwe3...` | `EPjFWdd5AufqSS...` | `mainnet.helius-rpc.com` |
