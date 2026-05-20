# Escrow Operations Runbook

## Overview

This runbook covers the full lifecycle of on-chain escrow for event deposits.

**Escrow Lifecycle:**
```
None → Initialized → Deactivated → Closed → None (re-init possible)
                                ↘ Cancelled → None (re-init possible)
```

**Two Admin UIs:**
| UI | Location | Use Case |
|----|----------|----------|
| Event Edit → Escrow Tab | `/admin/events/{id}` edit → Escrow | Primary — init, deactivate, close, re-init |
| Admin Escrow Panel | `/admin/escrow` | Sequential step view — backup |

**Environment Map:**
| Environment | KV ID | RPC | Escrow Program |
|-------------|-------|-----|----------------|
| Production | `c8a6a87f9ed34ce0a3c8e48b84039214` | mainnet-beta RPC | `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` |
| Preview/Dev | `7d74e1f62fb545be811eaefc8b059dee` | devnet RPC | Same program, devnet deployment |

**Deploy commands:**
```bash
# Production
cd worker && npx wrangler deploy
cd frontend-leptos && bash build.sh

# Dev (uses production KV — be careful)
cd worker && bash deploy.sh dev

# Dev (local — uses preview KV)
cd worker && npx wrangler dev
```

---

## 1. Fresh Event Escrow Setup

### Prerequisites
- [ ] Event created in admin panel with `deposit_enabled: true`
- [ ] `deposit_amount_usdc` set (e.g., `15_000_000` = $15 USDC, max $1,000)
- [ ] `organizer_wallet` set to organizer's Solana wallet (base58)
- [ ] `in_person_capacity` set
- [ ] Google Sheet connected for attendee sync

### Steps
1. Go to **Admin → Events → Edit → Escrow tab**
2. Verify `organizer_wallet` is correct
3. Click **"Initialize Escrow"**
4. Wallet extension pops up → **Approve the transaction**
5. Wait for confirmation (TX signature shown)
6. Verify: escrow status changes to `Initialized`, PDA address shown

### Verify
```bash
# Check on-chain (devnet)
solana account <ESCROW_PDA_ADDRESS> --url devnet

# Check health endpoint
curl -H "Authorization: Bearer $TOKEN" \
  "https://api.bethere.events/api/escrow/health?event_id=<EVENT_ID>"
```

---

## 2. Close Escrow After Event

### When to close
After the event ends AND:
- All refunds have been processed, OR
- The refund deadline has passed (forfeited deposits)

### Steps
1. **Deactivate** the escrow (stops new deposits)
   - Escrow tab → Click "Deactivate Escrow"
   - Or Admin Escrow Panel → Step 1

2. **Claim Forfeited** (optional — only if there are unclaimed deposits)
   - Skip this step if `total_deposited == 0` or all deposits were refunded
   - ⚠️ Known bug: `claim_forfeited` TX builder missing `attendee_deposit` account — may fail if deposits exist

3. **Close Event** (reclaims rent SOL)
   - Escrow tab → Click "Close Event"
   - Or Admin Escrow Panel → Step 3 (gated on Step 1 only)
   - Signs close transaction → all escrow accounts deleted on-chain

### Verify
```bash
# On-chain account should NOT exist
solana account <ESCROW_PDA_ADDRESS> --url devnet
# Expected: Error: AccountNotFound
```

---

## 3. Re-Initialize Escrow (Re-run Event)

### When to re-init
When the same event slug will be used again (e.g., recurring event series).

### Prerequisites
- Previous escrow must be **fully closed** on-chain (PDA gone)
- `Closed → None` transition requires on-chain verification

### Steps
1. Go to **Escrow tab** — should show "Escrow closed — rent reclaimed!"
2. Click **"Re-initialize Escrow"**
   - This resets: `escrow_status → None`, `escrow_address → ""`, `on_chain_event_id → 0`
   - On-chain verification confirms old PDA is gone
3. UI returns to `Idle` state
4. Follow **Fresh Event Escrow Setup** (Section 1) from step 2

### Important notes
- If re-init fails with "on-chain escrow account still exists" → the close TX didn't land. Retry the close first.
- `on_chain_event_id` resets to `0` → auto-derives from event slug hash → same PDA as before (safe since old one was deleted)
- To use a DIFFERENT PDA, manually set `on_chain_event_id` to a new value before initializing

---

## 4. Escrow Health Check

### Endpoint
```
GET /api/escrow/health?event_id=<EVENT_ID>
```

### Response fields
| Field | Meaning |
|-------|---------|
| `kv_escrow_status` | What the server (KV) thinks |
| `on_chain_exists` | Whether the escrow PDA exists on-chain |
| `consistent` | `true` if KV and on-chain agree |
| `diagnosis` | Human-readable explanation |

### When to check
- Before making announcements about deposits
- After any escrow lifecycle operation
- If something "looks wrong" in the admin UI
- During incident investigation

### Common diagnoses
| Diagnosis | Action |
|-----------|--------|
| `healthy: no escrow` | Normal — event hasn't initialized escrow yet |
| `healthy: escrow initialized` | Normal — deposits being accepted |
| `DRIFT: server says Closed but escrow exists` | Close TX failed — retry close on-chain |
| `DRIFT: server says Initialized but escrow not found` | Init TX failed or was reverted — reset to None and re-init |

---

## 5. Troubleshooting

### "cannot reset escrow: on-chain escrow account still exists"
- **Cause:** The close TX didn't land on-chain, or only partially executed
- **Fix:** Go back to the Escrow tab, try Close Event again. If it shows "Deactivated", sign the close TX.

### "escrow PDA collision"
- **Cause:** Trying to initialize an escrow at a PDA that already exists
- **Fix:** Either close the existing escrow first, or change `on_chain_event_id` to use a different PDA

### "deposit not enabled for this event"
- **Fix:** Edit the event → set `deposit_enabled: true` and `deposit_amount_usdc > 0`

### "event has no organizer wallet configured"
- **Fix:** Edit the event → set `organizer_wallet` to the organizer's Solana address

### Wallet shows "Transaction simulation failed"
- **Cause:** On-chain program rejected the instruction (wrong account, wrong state, etc.)
- **Fix:** Check the specific error in the wallet's "View on Solscan" link. Common: wrong cluster (devnet wallet but mainnet RPC), insufficient SOL for rent, account already initialized.

### claim_forfeited fails
- **Known bug:** TX builder missing `attendee_deposit` account
- **Workaround:** Skip claim_forfeited — go directly to close_event (works when `total_deposited == total_refunded + total_forfeited`)

---

## 6. Security Checklist (Pre-Production)

- [ ] `organizer_wallet` is a mainnet address (not devnet test wallet)
- [ ] `deposit_amount_usdc` is correct (in lamports: $15 = 15_000_000)
- [ ] Escrow program deployed on mainnet (same program ID)
- [ ] RPC URL points to mainnet-beta
- [ ] USDC mint is mainnet: `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m`
- [ ] Production KV has the event seeded
- [ ] `refund_deadline_hours` is set appropriately
- [ ] Organizer wallet has SOL for transaction fees
- [ ] Test the full lifecycle on devnet first

---

## 7. Audit Trail

All escrow operations are logged in the event audit trail:

```
GET /api/events/{id}/audit
```

Key audit actions:
| Action | Meaning |
|--------|---------|
| `EscrowInitialized` | Escrow created on-chain |
| `EscrowDeactivated` | Escrow stopped accepting deposits |
| `EscrowClosed` | Escrow fully closed, rent reclaimed |
| `EscrowReinitialized` | Escrow reset to None for re-init |
| `OnChainEventIndexed` | On-chain event synced to KV |

Filter by `action: "escrow_*"` to see only escrow-related events.
