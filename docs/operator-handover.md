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
| Staging | Healthy and isolated. The last verified deploy is version `0292d07d-c1b4-4174-a8e2-09ecae26a1ec` (2026-09-12); read the current version through Wrangler before a later release. |
| Completed-event learning gateway | Merged to `develop` at `8e8673b`, with retrospective-isolation hardening at `9954731`, and deployed to staging. The protected staging API proof passed: the disposable completed fixture's retrospective lead produces `online_count = 0`, has no live actions, and remains closed to new enrollment. Browser UI journey proof through organizer controls remains pending. |
| Production | Read the current version through Wrangler before a release; do not infer it from staging or Git history. |
| Networks | Escrow stays on devnet. Staging RPC/NFT/escrow roles report devnet |
| Staging NFT | Disabled because no staging Crossmint collection is configured; health reports `nft_not_configured`. Claim lookup now uses these actual Crossmint prerequisites, not Helius read credentials. |
| Worker log privacy | Issue 070 is verified. Every email, wallet, transaction signature, and attendee display name in the Worker log stream is a keyed one-way fingerprint, and the request path is redacted so capability tokens no longer reach the stream. The `log_pii_guard` source guard is now 8 tests: it also catches identifiers interpolated into a log *message*, fields recorded without a `%`/`?` sigil, and identifiers written into an **error message** that a caller then logs as `error = %e` (the class the probe could not reach, because its unusable Google credentials stop every Sheets helper before its error paths run). Two of the tests guard the guard itself — tracing macros must stay `tracing::`-qualified, or the scans go blind, and `domain` must stay log-free, or its sources need covering too. Event, org, and escrow addresses stay readable by design. Re-run the probe with `scripts/verify/pii_log_probe.sh` (see its header) after touching logging. **Residual:** Cloudflare's own request log records the raw URL, so a claim token in a path is visible in Workers Logs no matter what the Worker logs — a follow-up decision, not a regression. |
| Claim token replay window | Issue 071, option 2 implemented. Claim/quiz/adventure capability tokens travel in the URL path, so Cloudflare's own request log records them no matter what the Worker logs; a Workers Logs reader could previously replay a claim forever. A token is now accepted only within `CLAIM_TOKEN_TTL_SECS` of the attendee's **check-in** (**production runs 15552000 = 180 days**, staging 2592000 = 30 days; declared in both `[vars]` and `[env.staging.vars]`, which do not inherit). The 180-day production value is a deliberate **migration window**, not the steady state: the window is measured from `checked_in_at`, so enabling it is retroactive, and a 2026-09-13 count of production D1 found 85 checked-in-but-unclaimed attendees of whom **71 had checked in more than 30 days earlier** (oldest 139 days). Shipping 30 days would have revoked a real entitlement for those 71. Tighten to 30 days only after re-running the backlog query in `.issues/071_capability_token_in_url_platform_logs.md`. Outside the window the token behaves exactly as unknown, so a replay cannot tell a stale token from a typo. `0` disables the check and is the kill switch if it ever denies a real attendee. Not-yet-checked-in attendees are unaffected - there is no anchor and the existing "must be checked in" guards already apply. This bounds the exposure; it does not remove it. The full fix is to stop putting the capability in the path (option 3), which needs a frontend claim-flow change and accounting for QR codes already in circulation. |
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

Before deploying, require a clean committed application change. Do not use a
package-manager release-age bypass merely to run a newer Wrangler; review and
commit a Wrangler/lockfile update separately, after its dependencies have aged
normally. The pending 4.131.1 files are deliberately not part of `8e8673b`.

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

The completed-event gateway, retrospective-isolation hardening, and NFT claim
readiness correction were deployed as staging version
`0292d07d-c1b4-4174-a8e2-09ecae26a1ec`; health returned
`status: ok`, D1 connected, and the root response served `text/html`. The
isolated `test` fixture is completed, points to the canonical Genesis archive,
and has one disposable retrospective lead. Its public payload was verified with
`online_count = 0`; post-event enrollment is closed and a new submission returns
`409`. Do not use this fixture as production-like data. The remaining R1
browser test must still create or edit a disposable event using normal organizer
controls and follow the completed-event section in the Web Verification Runbook.

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
| Continue Worker log PII redaction | [Issue 070](../.issues/070_worker_log_pii_redaction.md) |

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
5. Finish the staged Worker log PII-redaction migration in Issue 070 before
   widening operational diagnostics or retention.
