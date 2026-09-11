# Operator Handover

This is the starting point for operating or continuing BeThere without the
original developer or coding assistant. It records current deployed state,
safe commands, release gates, and where to find deeper system documentation.

Last verified: **2026-09-11 (Asia/Bangkok)** on `develop` commit `ca9ff28`.

## Current state

| Area | State |
|---|---|
| Git | `develop` is clean and matches `origin/develop` at `ca9ff28` |
| Staging | Version `963012dc-55ef-4654-ae41-74315dff43ff`, 100% traffic, healthy |
| Production | Version `8c2813cc-e2f8-498e-96a3-e56693eaa509`, 100% traffic; it does not yet contain PRs #64–#68 |
| Networks | Escrow stays on devnet. Staging RPC/NFT/escrow roles report devnet |
| Staging NFT | Disabled because no staging Crossmint collection is configured; health reports `nft_not_configured` |
| Credit integrity | PR #68 is merged and staged. Production has six aggregate-only legacy THB credit statuses eligible for the guarded reconciliation repair after release |
| Unfinished branch | `feat/flow_harness_onchain_seam` contains useful older harness/on-chain commits and must be rebased or selectively integrated with current `develop` |

No production rows were changed during the 2026-09-11 audit. The count of six
contains no attendee PII. Do not put emails, claim tokens, wallet signatures,
OAuth values, or API keys into tickets or logs.

## Start every work session

```sh
git switch develop
git pull --ff-only origin develop
git status --short --branch

curl -fsS https://bethere-staging.solana-thailand.workers.dev/api/health \
  | python3 -m json.tool
curl -fsS https://bethere.solana-thailand.workers.dev/api/health \
  | python3 -m json.tool
```

Expected: both health endpoints return `status: ok` and `d1.connected: true`.
Production and staging may intentionally show different version/config state.

Create one focused branch per change. Target pull requests at `develop`, wait
for all five CI jobs, and merge only when every required check passes. Do not
commit directly to `main` or combine unrelated fixes in one commit.

## Validation before a pull request

```sh
cargo fmt --all -- --check
cargo check --workspace --locked --all-targets
cargo clippy --workspace --locked --all-targets -- -D warnings
cargo test --workspace --locked
cargo build -p event-checkin-worker \
  --target wasm32-unknown-unknown --locked --release
```

Run the frontend, Playwright, and Quasar checks when their areas change. GitHub
CI is the final cross-platform gate and currently runs five jobs covering these
surfaces.

## Deploy staging

```sh
bash worker/deploy.sh staging

cd worker
pnpm exec wrangler deployments status --env staging --json
cd ..

curl -fsS https://bethere-staging.solana-thailand.workers.dev/api/health \
  | python3 -m json.tool
```

The deploy script builds the frontend and Worker, deploys only the staging
bindings, then verifies HTML and JavaScript content types. Record the printed
version ID and health result. Staging data must remain disposable and isolated
from production.

## Production release gate

Do not deploy production merely because `develop` is green. Before release:

1. Complete the staging web checklist in
   [Web Verification Runbook](web-verification-runbook.md).
2. Complete the live flow harness or record why its remaining P0 stubs block the
   release. Never manufacture a `.last-green` sentinel.
3. Review pending D1 migrations and environment/secret names. Do not print
   secret values.
4. Record the current production version ID for rollback.
5. Obtain explicit release authorization, including the intended commit and
   expected production data effects.
6. Deploy, run read-only production smoke checks, and verify the six legacy
   credit statuses become clean after the scheduled reconciler. Stop if any
   money-integrity counter remains nonzero.

The current production release has a known data effect: the scheduled credit
reconciler may change exactly matching legacy `deposit_statuses.method` values
from `thb` to `credit_thb`. It requires matching attendee, event, ledger amount,
currency, and `ROLLING_CREDIT_AUTO_APPLIED` marker, and refuses any record with
a cash deposit row.

Use [Gradual Deploy Runbook](gradual_deploy_runbook.md) for the supported rollout
and rollback procedure. If its Wrangler command differs from `pnpm exec wrangler
--help`, stop and use the installed CLI help plus current Cloudflare docs before
changing production traffic.

## Incident response

If health, assets, auth, registration, or deposit state regresses:

1. Stop writes and capture timestamp, correlation ID, route, deployed version,
   and non-sensitive error text.
2. Roll back the Worker version. Do not delete D1/KV/R2 resources.
3. Run only aggregate/read-only queries until the failure mode is understood.
4. For credit incidents, compare `credit_ledger`, `thb_deposits`,
   `deposit_statuses`, and attendee ownership. Never compensate balances by hand
   without an append-only ledger entry and a reviewed incident record.
5. For ambiguous NFT results, inspect `nft_mint_jobs` and reuse its idempotency
   key. Never submit a second mint while the first outcome is unknown.

## Documentation map

| Need | Document |
|---|---|
| Verify the real website safely | [Web Verification Runbook](web-verification-runbook.md) |
| Deploy and isolate staging | [Staging Deploy Runbook](staging_deploy_runbook.md) |
| Release or roll back production | [Gradual Deploy Runbook](gradual_deploy_runbook.md) |
| Understand THB/USDC/refund/credit behavior | [Deposit & Refund Flows](deposit-refund-flows.md) |
| Understand NFT idempotency and cost boundaries | [Crossmint Minting](crossmint-minting.md) |
| Judge escrow mainnet readiness | [Mainnet Readiness Runbook](mainnet_readiness_runbook.md) |
| See architecture and source-of-truth boundaries | [Architecture](architecture.md) |
| Track remaining core risks | [Core services audit](../.issues/066_core_services_readiness_audit.md) |

## Next implementation order

1. Rebase or selectively integrate `feat/flow_harness_onchain_seam`, removing
   every live-flow stub only when staging fixtures and capped devnet wallets are
   available.
2. Provision the dedicated capped devnet harness fixture and record a real green
   live run; the production deploy gate is default-on.
3. Consolidate THB verification projections and reconciliation.
4. Add browser E2E coverage for registration recovery and attendee deposit,
   quiz, ticket, and claim states.
