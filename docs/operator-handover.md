# Operator Handover

This is the starting point for operating or continuing BeThere without the
original developer or coding assistant. It records current deployed state,
safe commands, release gates, and where to find deeper system documentation.

Last verified: **2026-09-12 (Asia/Bangkok)**. Check `git status --short --branch`
and the current deployment status at the start of every release; do not rely on
a copied commit or Worker version in this document.

## Current state

| Area | State |
|---|---|
| Git | `develop` is the integration branch. The separate Wrangler 4.131.1 lockfile update is intentionally uncommitted pending dependency-review approval. |
| Staging | Healthy and isolated. Read the current version through Wrangler before a release. |
| Production | Read the current version through Wrangler before a release; do not infer it from staging or Git history. |
| Networks | Escrow stays on devnet. Staging RPC/NFT/escrow roles report devnet |
| Staging NFT | Disabled because no staging Crossmint collection is configured; health reports `nft_not_configured` |
| Credit integrity | PR #68 is merged and staged. Production has six aggregate-only legacy THB credit statuses eligible for the guarded reconciliation repair after release |
| Escrow staging proof | Focused SIWS auth, authenticated deposit confirmation, D1 verified status, and matching Devnet attendee PDA passed on 2026-09-12. See [Fixture Strategy](flow_harness_fixture_strategy.md). |
| Escrow release gate | Still blocked: refund and NFT fixtures are not yet provisioned, so there is no full-suite green sentinel for production preflight. |

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
2. Complete the full live flow-harness fixture matrix. The focused SIWS/deposit
   proof is green, but it cannot refresh `.last-green`; never manufacture a
   sentinel.
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
| Operate named Devnet harness fixtures | [Fixture Strategy](flow_harness_fixture_strategy.md) |
| Release or roll back production | [Gradual Deploy Runbook](gradual_deploy_runbook.md) |
| Understand THB/USDC/refund/credit behavior | [Deposit & Refund Flows](deposit-refund-flows.md) |
| Understand NFT idempotency and cost boundaries | [Crossmint Minting](crossmint-minting.md) |
| Judge escrow mainnet readiness | [Mainnet Readiness Runbook](mainnet_readiness_runbook.md) |
| See architecture and source-of-truth boundaries | [Architecture](architecture.md) |
| Track remaining core risks | [Core services audit](../.issues/066_core_services_readiness_audit.md) |

## Next implementation order

1. Reassess and prioritize customer-facing work before expanding infrastructure:
   post-event learning/recap, registration recovery, attendee ticket/deposit
   UX, and event-detail pages are the highest-value candidates.
2. Keep the full staging fixture matrix as a release requirement: provision
   refund and NFT fixtures, run the complete suite twice, and retain both
   summaries before any escrow production release.
3. Consolidate THB verification projections and reconciliation.
4. Add browser E2E coverage for registration recovery and attendee deposit,
   quiz, ticket, and claim states.
