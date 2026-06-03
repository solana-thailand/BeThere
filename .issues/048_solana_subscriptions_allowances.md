# 048 — Evaluate Solana Subscriptions & Allowances for Event Deposits

> **Status**: Research
> **Priority**: P2 (post-mainnet stabilization)
> **Related**: `docs/tdd_ddd_architecture.md`, `docs/escrow_protocol.md`
> **Source**: https://solana.com/news/subscriptions-and-allowances (2026-06-02)

## Summary

Solana launched a native **Subscriptions & Allowances** program on mainnet. The **Fixed Delegation** model could replace our custom Quasar escrow program for USDC event deposits, offering an audited, standard primitive instead of maintaining our own escrow.

## Current State: Quasar Escrow

```
Attendee → deposits USDC into escrow PDA → checks in → refund from escrow
                                               ↓ no-show
                                    Organizer claims forfeited deposit
```

- Custom program: 63 KB, `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`
- 85+ tests (47 unit + 38 on-chain SVM)
- Dual-track: USDC (on-chain escrow) + THB (PromptPay slip upload)
- Not professionally audited

## Proposed: Fixed Delegation (Allowances)

```
Attendee → authorizes allowance ($5 USDC, expires after event + refund window)
                                               ↓
                                    Staff scans QR → check-in
                                               ↓
                     ┌── checked in? → allowance auto-expires (funds stay in wallet)
                     └── no-show? → organizer draws from allowance
```

**Program ID**: `De1egAFMkMWZSN5rYXRj9CAdheBamobVNubTsi9avR44`

## Technical Comparison

| Aspect | Current (Quasar Escrow) | Fixed Delegation (Allowances) |
|---|---|---|
| **Fund custody** | USDC locked in escrow PDA | USDC stays in attendee's wallet until drawn |
| **Refund UX** | Attendee signs deposit TX + separate refund TX | Single authorization, no manual refund needed |
| **Program audit** | Unaudited | Cantina/Spearbit audited |
| **Token support** | SPL Token only | SPL Token + Token-2022 |
| **Multi-delegate** | Single escrow per event | One program, many simultaneous delegations |
| **Expiry** | Manual refund deadline logic in worker | Native `expiryTs` parameter |
| **Revocation** | Organizer must close escrow | User can revoke anytime, or auto-expires |
| **Smart wallet** | Unknown | Tested with Squads multisig + Swig |
| **On-chain events** | Custom event parsing | Built-in events via self-CPI |
| **THB deposits** | N/A (on-chain only) | N/A (on-chain only) — THB flow unchanged |

## Flow Mapping: BeThere Deposit → Fixed Delegation

### Step 1: Attendee authorizes allowance (replaces deposit TX)

```typescript
// Current: attendee signs transfer to escrow PDA
// Proposed: attendee authorizes organizer as delegatee

const delegationPda = await client.subscriptions.instructions
  .createFixedDelegation({
    tokenMint: USDC_MINT,
    delegatee: organizerWallet,    // organizer can draw on no-show
    nonce: 0n,
    amount: depositAmountUnits,    // e.g., 5_000_000 (5 USDC)
    expiryTs: BigInt(eventEndMs / 1000 + refundDeadlineSeconds),
  })
  .sendTransaction();
```

### Step 2a: Attendee checks in → allowance expires naturally

No action needed. The `expiryTs` passes, delegation becomes unusable.

Optional: attendee can revoke early for immediate release.

### Step 2b: No-show → organizer draws from allowance

```typescript
// Organizer (delegatee) signs the transfer
await client.subscriptions.instructions
  .transferFixed({
    delegatee: organizerSigner,
    delegator: attendeeWallet,
    delegatorAta: attendeeUsdcAta,
    tokenMint: USDC_MINT,
    delegationPda,
    amount: depositAmountUnits,
    receiverAta: organizerUsdcAta,
    tokenProgram: TOKEN_PROGRAM_ADDRESS,
  })
  .sendTransaction();
```

### Step 3: Refund (checked-in, before event end)

```typescript
// Attendee revokes the delegation — returns rent + releases authorization
await client.subscriptions.instructions
  .revokeDelegation({
    authority: attendeeSigner,
    delegationAccount: delegationPda,
  })
  .sendTransaction();
```

## Key Technical Details

### Program Architecture

- **Subscription Authority (SA)**: One PDA per `(user, mint)` pair. Set as delegate on user's token account with `u64::MAX` approval.
- **Delegation PDA**: Per `(SA, delegatee, nonce)`. Authorizes specific transfers.
- **SA cannot move funds alone** — a transfer only succeeds when a matching Delegation PDA exists.
- Built with **Pinocchio** (not Anchor), IDL via **Codama**.
- **Rust client** available: `subscriptions` crate in `clients/rust/`.

### Token-2022 Support

The program supports Token-2022 but **rejects** these extensions during SA init:
- `ConfidentialTransfer`, `NonTransferable`, `PermanentDelegate`, `TransferHook`, `TransferFee`, `MintCloseAuthority`, `Pausable`

Standard USDC (SPL Token) works without issues.

### On-Chain Events

Events emitted via self-CPI for indexer integration:
- Subscription created/cancelled
- Fixed/recurring/subscription transfers

This could replace our custom `escrow_indexer` module.

### Account Versioning

Three-tier migration: lazy in-place update → explicit migrate → revoke/recreate.
Future-proof for program upgrades without breaking existing users.

## Advantages for BeThere

1. **Eliminates custom escrow program** — no Quasar maintenance
2. **Professional audit** — Cantina audit, reduces our security surface
3. **Better UX** — funds stay in attendee's wallet, no "locked" anxiety
4. **Auto-expiry** — `expiryTs` handles refund deadline natively
5. **Multi-event** — one program, many delegations, no escrow lifecycle management
6. **Standard primitive** — wallets will natively support this
7. **Simplifies worker** — removes `solana_escrow/`, `escrow_indexer/`, `claim/` complexity
8. **Rust client available** — fits our 100% Rust stack

## Risks & Concerns

1. **Brand new** (launched 2026-06-02). Zero battle-testing on mainnet.
2. **Migration effort** — 85+ tests, Playwright E2E, devnet deployment built around Quasar.
3. **Refund flow change** — currently attendee gets explicit refund TX; with allowances, it's auto-expire + optional revoke. UX different.
4. **Deposit order / tier logic** — our `max_refundable_deposits` tier system needs to be handled at the worker level, not on-chain.
5. **No partial draw tracking** — Fixed Delegation tracks remaining amount, but our per-attendee deposit tracking still needs KV/DB.
6. **THB deposits unchanged** — dual-track complexity remains for PromptPay flow.
7. **Organizer key management** — delegatee must be the organizer's wallet. Multi-organizer events need key rotation or a shared delegatee address.
8. **Nonce management** — each delegation needs a unique nonce per `(SA, delegatee)` pair.

## Migration Strategy (If We Proceed)

### Phase 1: Research (1-2 days)
- [ ] Deploy subscriptions program on local Surfpool
- [ ] Build Fixed Delegation flow with Rust client
- [ ] Map every BeThere escrow instruction to delegation equivalent
- [ ] Identify gaps (deposit order, tier limits, multi-organizer)

### Phase 2: Devnet Prototype (3-5 days)
- [ ] Add `@solana/subscriptions` / Rust client to workspace
- [ ] Replace `solana_escrow` module with delegation instructions
- [ ] Update deposit handler to create delegation instead of escrow deposit
- [ ] Update no-show handler to draw from delegation instead of claim
- [ ] Update check-in handler to revoke delegation on refund
- [ ] Keep THB flow unchanged

### Phase 3: Testing (2-3 days)
- [ ] Port on-chain SVM tests to delegation flow
- [ ] Update Playwright E2E for new deposit/refund UX
- [ ] Load test with concurrent delegations
- [ ] Edge cases: expiry, revoke, partial draw, smart wallets

### Phase 4: Mainnet Migration (1-2 days)
- [ ] Deploy to mainnet with feature flag
- [ ] A/B test: new events use delegations, existing events stay on escrow
- [ ] Monitor for 2-4 weeks
- [ ] Full cutover once stable

## Open Questions

1. **Can organizer be a PDA?** — Our current escrow uses PDAs. If delegatee must be a keypair, we need a different approach for multi-organizer.
2. **Deposit deadline enforcement** — Our `deposit_deadline_hours` from registration date is per-attendee. Fixed Delegation has a single `expiryTs`. Need worker-level enforcement.
3. **Rolling deposits / credits** — Issue #032 rolling deposit credit system. Does this work with delegations?
4. **Rust client maturity** — TypeScript SDK is primary. Rust client is generated but may be less documented.
5. **CU costs** — Need to benchmark delegation create + transfer vs escrow deposit + refund.

## Verdict

**Recommendation: Track and evaluate in 2-3 months.**

The Fixed Delegation model is a strong fit for BeThere's deposit flow, but the program is too new (1 day old) to bet production on. Continue with Quasar escrow for mainnet launch, and revisit when:

- Program has 3+ months of mainnet activity
- Edge cases and bugs are surfaced and fixed
- Rust client is documented and stable
- Wallet support (Phantom, Solflare) includes delegation UX

Add this to the post-mainnet roadmap as a potential architecture simplification.

## References

- News: https://solana.com/news/subscriptions-and-allowances
- Docs: https://solana.com/docs/payments/subscriptions/overview
- Fixed Delegation: https://solana.com/docs/payments/subscriptions/fixed-delegation
- Source: https://github.com/solana-program/subscriptions
- Audit: Cantina (report in `audits/` directory)
- Program ID: `De1egAFMkMWZSN5rYXRj9CAdheBamobVNubTsi9avR44`
- MPP Spec PR: https://github.com/tempoxyz/mpp-specs/pull/270
