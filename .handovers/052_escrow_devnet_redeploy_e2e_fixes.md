# Handover 052: Escrow Devnet Redeploy + E2E Test Script Fixes

## What Happened

Redeployed the security-hardened BeThere escrow program to Solana devnet and fixed two E2E test script issues (cost analysis crash + ApiOk wrapper parsing). Both test suites now pass with the new deployment.

## Changes Made

### 1. Escrow Program Redeployed to Devnet

- Previous deployment (slot 460743945) was missing the latest fix: `refund_deadline upper bound + vault balance check` (commit `77922bb`, May 8)
- Rebuilt with `quasar build` → 66,736 bytes (+56 B from previous)
- Deployed: `solana program deploy target/deploy/bethere_escrow.so --url devnet --program-id 2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo`
- New slot: 461513316
- TX: `CVq4Rgst13LK3EKW2GDLYrxKHZC44jqMEZ5epGWsLQRqazrt37idh7c62ej7qF57cRXNznPCxk6cBT7byNLhJ1J`

### 2. Secret/Sensitive Scan (Pre-Commit)

Scanned all diffs before committing — **clean**:
- `dev-token` — dev-mode auth bypass (only works with `DEV_MODE=1`)
- `ratchapon.poc@gmail.com` — test email, already in existing code
- `$JWT_SECRET` — read from env var, not hardcoded
- `~/.config/solana/id.json` — path reference only, keypair never committed
- `0812345678` — fake promptpay test number

### 3. E2E Test Script Fixes (2 commits)

**Commit `e809083`** — Major escrow test rewrite:
- Replace non-existent `/api/escrow/create-vault-ata` + `/api/escrow/create-event` with combined `/api/escrow/init`
- Fix on-chain timing: event_end_ms 2min future for create_event, extend server-side, wait for on-chain event_end before refund
- Reorder to 14-step lifecycle (was 7 steps)
- Fix `test_full_e2e.sh` Step 2: unwrap `data.email` from ApiOk wrapper
- Skip quiz when status is `not_required`/`not_configured`

**Commit `69c11fb`** — Bug fixes:
- Cost analysis: 3x retry for `getTransaction` on devnet indexing lag
- Cost analysis: handle null result gracefully (was crashing with `NoneType`)
- Cost analysis: guard division-by-zero in savings calc
- Deposit status: unwrap `data.*` from ApiOk wrapper (pre-deposit, post-deposit, confirm endpoints)

## Test Results

| Suite | Result | Details |
|-------|--------|---------|
| Escrow E2E (devnet) | ✅ 29/29 PASS | Full 14-step lifecycle against new deployment |
| Browser E2E (devnet) | ✅ 10/10 PASS | Full 10-step browser flow, cNFT minted |
| Escrow unit tests | ✅ 22/22 (from handover 041) | QuasarSVM tests |
| Worker tests | ✅ 39/39 (from handover 045) | Cargo tests |

### Key On-Chain TXs (This Run)

| Step | TX Signature |
|------|-------------|
| Init escrow | `2YMQwjLRbTX3TD3uso3wxx6rP8aipJbHpb3A3B3LXtVcTMhnS9n1wv1CWjo8dTcEqNvxaFPW9SQwnLGYKfZ5hJ8G` |
| Deposit | `4cQnNGRa5CHfcuWGzmE2LU7cUj58KreenSswnuypnZyVYTApx2vU7KPqeDYRjNyn8x1WHe6GunnwehS8CUuuoEJe` |
| Refund | `5PA5wPRnHuhqSrvPs8T7t4nS3yjZtoV3sLzJtwYes3zSyW5sKDZ8CXQj1a1ep7gEyvPoV4UsPvgXwd6KEgGtWVHT` |
| Deactivate | `3SLThXHJRvQSXWZd9psTAhvmVD17ZZNZYTkYN5Ntquy97Pjpubf7uT2K5zjz5yK1LjvsCY5hvhhLCW1pJQFCjJV9` |
| Claim forfeited | `2jnDziWsqziCsZ7dQqzh9X3YYVduEtooB3VruJJTyPpkJ3vZJHvZBish3ZaCKYsgtMNXXj96jj6ApEvEvBWrnTYd` |
| Close event | `5WzDH6gDRAkFCq5aBRn3DGdn4ixfWCGg992SAS7ZStjWm5E4Kqq9DuapTig8b1ECnHkE52t2YDgjt8frevPF9BP6` |
| cNFT mint | `4omCGAuSYEj5yCoif3soUGMGy7sZdXvZvXUfL3dUF5goYhhxrzj6LDrndKohEML6zX96fZSkvBTtJ8riwbBFP2Sh` |

### Cost Analysis (from E2E run)

| Metric | Value |
|--------|-------|
| Network fee | 5,051 lamports (0.000005051 SOL) |
| Compute units | 41,225 |
| USD cost | ~$0.00087 at $172/SOL |
| vs Traditional NFT | ~990x cheaper |
| 1000 attendees | ~$0.87 total |

## Commits

1. `e809083` — `test: fix escrow & browser E2E scripts for devnet validation`
2. `69c11fb` — `fix: E2E test script fixes — cost analysis retry + ApiOk wrapper parsing`

Both pushed to `develop/feature/016_e2e_testing_and_escrow_ux`.

## Remain Work

### Pre-Mainnet (Blockers)
- [ ] Deploy escrow program to mainnet (~0.5 SOL for rent)
- [ ] Configure worker production secrets (`HELIUS_API_KEY`, Google service account, `JWT_SECRET`, `SOLANA_CLUSTER=mainnet-beta`)
- [ ] Build frontend with `DEV_MODE=0`
- [ ] Deploy worker via `wrangler deploy --env production`
- [ ] Mainnet E2E smoke test with real USDC

### Non-blocking
- [ ] Retry logic for devnet SOL airdrop (currently fails silently)
- [ ] `--skip-deposit` flag for testing escrow lifecycle without on-chain deposit
- [ ] Issue #007 status update — most items now done
- [ ] Issue #013 Phase 2 remaining — frontend escrow account verification indicator
- [ ] mark_checked_in TX failing with "unknown" — investigate worker-side TX builder

## How to Dev/Test

```bash
# Escrow program
cd bethere-escrow
quasar build                          # 66.7 KB .so
cargo test --lib -- tests --nocapture # 22 unit tests
quasar deploy --url devnet            # or: solana program deploy target/deploy/bethere_escrow.so --url devnet --program-id 2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo

# E2E tests (requires worker running)
cd worker && npx wrangler dev --port 8787
bash scripts/e2e/test_escrow_devnet.sh   # 29 steps, ~90s (includes 90s wait for event_end)
bash scripts/e2e/test_full_e2e.sh         # 10 steps, ~30s

# Mainnet deploy (future)
quasar build
solana program deploy target/deploy/bethere_escrow.so --url mainnet-beta --program-id <MAINNET_KEYPAIR>
```

## Issues Ref
- Issue #007 — Devnet E2E Test (update status)
- Issue #010 — Deposit/Refund Escrow
- Issue #013 — Escrow Rug Pull Prevention (security hardening deployed)
- Handover 041 — Security audit findings
- Handover 045 — Previous devnet deployment
