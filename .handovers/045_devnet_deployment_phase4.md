# Handover 045: Devnet Deployment — Phase 4 Rent Reclamation

**Date**: 2026-05-07
**Branch**: `feature/010_deposit_refund_escrow`
**Commits**: 0 new (deployment session only)

---

## What Happened

Continued from handover 044. The Phase 4 SEC-010 implementation was already complete and committed. This session focused on **deployment preparation and devnet upgrade**.

### 1. Code Review

Reviewed all Phase 4 artifacts across 3 layers:

- **On-chain** (`close_deposit.rs`): Self-close (attendee after refund) + GC close (anyone after event closed). Uses `UncheckedAccount` for potentially-closed event_escrow. Two new errors (17, 18), one new event (discriminator 7).
- **Worker** (`solana_escrow.rs` + `handlers/deposit.rs`): TX builder with 4 account metas, public endpoint `POST /api/escrow/close-deposit`.
- **Frontend** (`pages/deposit.rs`): 4 new `DepositPageState` variants, "♻️ Reclaim Rent" buttons in `RefundConfirmed` and `AlreadyDeposited` views.

All code is clean and well-structured. No edge case issues found.

### 2. Build Verification

| Layer | Command | Result |
|-------|---------|--------|
| Escrow program | `quasar build` | ✅ 65.1 KB (66,680 bytes) |
| Worker | `cargo check -p event-checkin-worker` | ✅ Clean |
| Frontend | `cargo check --target wasm32-unknown-unknown` | ✅ Clean |
| Escrow tests | `cargo test` (in bethere-escrow) | ✅ 26/26 pass |
| Worker tests | `cargo test -p event-checkin-worker` | ✅ 39/39 pass |
| Diagnostics | Full workspace | ✅ Zero |

### 3. Devnet Deployment

**Program upgrade** on existing program ID:

| Field | Before | After |
|-------|--------|-------|
| Program ID | `2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo` | Same |
| Data Length | 63,040 bytes | 66,680 bytes (+3,640) |
| Deployed Slot | 460,226,544 | 460,743,945 |
| Authority | `9Bz7p4RWdX7eaR4hFUeCc7aSZjDHsie8q1u8imwavkBN` | Same |

Deploy command:
```bash
solana program deploy target/deploy/bethere_escrow.so --url devnet \
  --program-id 2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo
```

**Note**: Initial `solana program deploy` without `--program-id` created an accidental new program at `6FXVdvNHzUwbPdqspSqtyJuGy5exDeBbytUkcuZunnW4`. This was immediately closed with `solana program close` and 0.465 SOL reclaimed.

Devnet balance after deployment: **3.635 SOL**.

### 4. Documentation Update

Updated `docs/devnet_testing_guide.md`:
- Added **Flow C: Attendee Deposit → Refund → Reclaim Rent** (Steps 9–13)
- Added Step 13: Reclaim Rent (Phase 4 — SEC-010) with verification instructions
- Added alternative: Reclaim Rent from Already Deposited View
- Reorganized checklist into 3 flows (A: Event Setup, B: Admin Escrow, C: Attendee Deposit)

---

## Remaining Work

### Worker + Frontend Deployment (not done yet)
- [ ] Deploy worker to Cloudflare Workers (`npx wrangler deploy` in worker/)
- [ ] Build frontend (`./build.sh` in frontend-leptos/) and deploy dist/
- [ ] Verify worker health endpoint returns correct cluster info

### Browser E2E Test (manual)
- [ ] Full Flow A: Create event → connect wallet → create vault ATA → init escrow
- [ ] Full Flow B: Deactivate → claim forfeited → close event
- [ ] Full Flow C: Deposit → check-in → refund → reclaim rent
- [ ] Verify all Solscan links show `cluster=devnet`
- [ ] Test wrong wallet rejection on admin escrow panel
- [ ] Test order enforcement (skip steps → error)

### After Devnet Validation
- [ ] Mainnet deployment (requires ~0.5 SOL for program buffer + deployment)

---

## Key Commands Reference

```bash
# Build escrow
cd bethere-escrow && quasar build

# Deploy to devnet
solana program deploy target/deploy/bethere_escrow.so --url devnet \
  --program-id 2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo

# Verify deployment
solana program show 2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo --url devnet

# Check balance
solana balance --url devnet

# Build frontend
cd frontend-leptos && ./build.sh

# Deploy worker
cd worker && npx wrangler deploy
```

---

## Issues Ref
- `.issues/013_escrow_rug_pull_prevention.md` — RESOLVED (all 4 phases complete)
