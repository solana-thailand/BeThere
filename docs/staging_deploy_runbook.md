# Staging Deploy Runbook

> Operational checklist for deploying the `bethere-staging` Worker environment
> (Plan 005 §3.1). This is the next validation gate before the mainnet cutover:
> it validates the merged Plan 005/008 code (incl. PR #21's cluster-aware escrow)
> in a *real* Cloudflare environment — not just `wrangler dev` — and is the
> prerequisite for activating the §3.5 preflight gate and the `flow-harness` E2E.
>
> **Audience:** the operator performing the staging deploy. **Scope:** one-time
> provisioning is already done; this covers secrets, OAuth, migrations, deploy,
> seed, and isolation verification.

---

## 1. Provisioning state (verified on `develop` @ `c7a6e5d`)

The Cloudflare resources are **already provisioned** (real IDs committed in
`worker/wrangler.toml`, dated 2026-07-10). Plan 005's status note saying
"real provisioning NOT STARTED" is stale — ignore it.

| Resource | Binding | ID / Name | Status |
|----------|---------|-----------|--------|
| D1 | `DB` | `bethere-db-staging` (`951fce4e-b874-4ab0-ae70-39a27dbf1d8c`) | ✅ provisioned |
| KV | `EVENTS` | `dd1d541c936242ecb1533ba6ac6a9851` | ✅ provisioned |
| R2 | `ASSETS_BUCKET` | `bethere-assets-staging` | ✅ provisioned |
| `[env.staging]` block | — | `worker/wrangler.toml` | ✅ merged (PR #19) |
| `deploy.sh staging` path | — | resolves to `wrangler deploy --env staging` | ✅ merged |
| `seed-staging.sh` | — | `worker/scripts/seed-staging.sh` | ✅ merged |
| Staging secrets | — | not yet set | ❌ **pending (§3)** |
| Google OAuth staging redirect URI | — | not yet registered | ❌ **pending (§4)** |
| D1 migrations applied to staging | — | unverified | ⚠️ **check (§5)** |
| Staging Worker deployed | — | not yet deployed | ❌ **pending (§6)** |
| Isolation verified | — | not yet verified | ❌ **pending (§8)** |

The staging Worker URL is `https://bethere-staging.solana-thailand.workers.dev`
(declared in `[env.staging.vars].SERVER_URL`).

---

## 2. How staging differs from production

- **Deploy path:** `bash worker/deploy.sh staging` runs the standard
  `wrangler deploy --env staging`. It does **not** use the production PUT-API
  fallback (`worker/deploy.sh#L200` — staging intentionally has no fallback).
  If the Cloudflare `/versions` API is degraded (error 10013), staging deploy
  fails fast; retry once it recovers.
- **Preflight gate:** staging deploys **skip** the §3.5 gate (it is
  production-only; `worker/deploy.sh#L125`). No `BETHERE_PREFLIGHT_GATE` needed.
- **DEV_MODE = "1"** (staging-only) permits test shortcuts; production stays `0`.
- **Bindings are non-inheritable** — `[env.staging]` redeclares the full `vars`
  set + D1/KV/R2 with staging IDs. Adding a new prod var requires mirroring it
  here or it will be absent on staging.

---

## 3. Secrets checklist (the main missing piece)

Secrets are **non-inheritable** — staging needs its own copies, set with
`--env staging`. Verify what's already set first:

```/dev/null/sh#L1
npx wrangler secret list --env staging
```

Then set each missing one. **Required** (Worker will malfunction without these):

```/dev/null/sh#L1
# Auth
npx wrangler secret put JWT_SECRET --env staging

# Google OAuth (use a SEPARATE staging client if possible; see §4)
npx wrangler secret put GOOGLE_CLIENT_ID --env staging
npx wrangler secret put GOOGLE_CLIENT_SECRET --env staging
npx wrangler secret put GOOGLE_REDIRECT_URI --env staging
#   → enter: https://bethere-staging.solana-thailand.workers.dev/api/auth/callback

# Google Service Account (Sheets access — can reuse prod SA, point at a staging sheet)
npx wrangler secret put GOOGLE_SERVICE_ACCOUNT_EMAIL --env staging
npx wrangler secret put GOOGLE_SERVICE_ACCOUNT_PRIVATE_KEY --env staging
npx wrangler secret put GOOGLE_SERVICE_ACCOUNT_TOKEN_URI --env staging
npx wrangler secret put GOOGLE_SHEET_ID --env staging
#   → enter the staging Google Sheet ID (NOT the production PLATFORM_SHEET_ID)

# Staff / admin
npx wrangler secret put STAFF_EMAILS --env staging

# Solana / Helius (cluster-aware escrow — see §3 note)
npx wrangler secret put HELIUS_API_KEY --env staging
npx wrangler secret put SOLANA_CLUSTER --env staging
#   → enter: devnet   (staging must NOT point at mainnet-beta until §3 of the
#     canary runbook is fully resolved and ESCROW_PROGRAM_ID_MAINNET is filled)
```

**Optional** (leave unset to use defaults / disable the feature):

```/dev/null/sh#L1
npx wrangler secret put HELIUS_RPC_URL --env staging          # defaults to https://devnet.helius-rpc.com
npx wrangler secret put NFT_COLLECTION_MINT --env staging     # omit → Helius mints to its own tree
npx wrangler secret put NFT_METADATA_URI --env staging        # omit → no NFT badge metadata
npx wrangler secret put NFT_IMAGE_URL --env staging           # omit → no NFT badge image
```

> **Cluster guardrail:** `SOLANA_CLUSTER=devnet` is the only safe value for
> staging today. PR #21 made `escrow_program_id()` cluster-aware, but
> `ESCROW_PROGRAM_ID_MAINNET` is the intentional `""` fail-loud guard. Setting
> `mainnet-beta` on staging would error at base58 parsing — by design.

---

## 4. Google OAuth staging redirect URI

This is a **human/web action** in the Google Cloud Console — it cannot be done
from the CLI:

1. Open the Google Cloud OAuth client used by the Worker (the one matching
   `GOOGLE_CLIENT_ID`).
2. Add an **Authorized redirect URI**:
   `https://bethere-staging.solana-thailand.workers.dev/api/auth/callback`
3. Save. Propagation is usually under a minute.
4. Set `GOOGLE_REDIRECT_URI` to that exact value (§3).

If using a **separate OAuth client for staging** (cleaner isolation), set both
`GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET` to the staging client's values.
Reusing the prod client with an additional redirect URI is also fine.

---

## 5. D1 migrations (verify before first deploy)

The staging D1 was created but migrations may not have been applied. Check the
schema version, then apply if missing:

```/dev/null/sh#L1
# Inspect current tables (if empty/missing → migrations not applied)
npx wrangler d1 execute bethere-db-staging --remote \
  --command "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;"

# Apply migrations (idempotent — safe to re-run)
npx wrangler d1 migrations apply bethere-db-staging --remote --env staging
```

Expected tables after migration: `events`, `attendees`, `deposit_statuses`,
`contacts`, `staff`, `developer_profiles`, `claim_locks`, and any audit trail
tables per `worker/migrations/`.

---

## 6. Deploy sequence

```/dev/null/sh#L1
# 1. Ensure the frontend is built (the Worker serves ../frontend-leptos/dist)
cd frontend-leptos && bash build.sh && cd ..

# 2. Deploy staging (standard wrangler path; no PUT-API fallback for staging)
bash worker/deploy.sh staging
```

A successful deploy prints `✅ Deployed via wrangler`. If it fails with error
10013, the Cloudflare `/versions` API is degraded — wait and retry (staging has
no fallback by design; `worker/deploy.sh#L200`).

---

## 7. Seed staging data

Once deployed and healthy, seed the deterministic test event + attendee that the
`flow-harness` exercises:

```/dev/null/sh#L1
bash worker/scripts/seed-staging.sh
```

This inserts `flow-test-event` with `event_start=now-4h`, `event_end=now-2h`,
`refund_deadline_hours=6`, plus a checked-in test attendee. It touches **only**
staging data and never production. Re-run is idempotent (`INSERT OR REPLACE`);
pass `--clean` to wipe the test rows first.

---

## 8. Isolation verification (Plan 005 §3.1 acceptance criterion)

Confirm staging D1 contains only the seeded test data, not production attendees:

```/dev/null/sh#L1
# Should show n=1 (the seeded flow-test attendee), NOT the production count
npx wrangler d1 execute bethere-db-staging --remote \
  --command "SELECT count(*) AS n FROM attendees WHERE event_id = 'flow-test-event';"

# Sanity: total attendee count should be small (just the seed), not prod-scale
npx wrangler d1 execute bethere-db-staging --remote \
  --command "SELECT count(*) AS total FROM attendees;"
```

Then verify the Worker is live and serving:

```/dev/null/sh#L1
curl -s https://bethere-staging.solana-thailand.workers.dev/api/health | python3 -m json.tool
# Expect: {"status":"ok", ...}  (and cluster reflects devnet)

curl -s -o /dev/null -w "/ → %{http_code}\n"  https://bethere-staging.solana-thailand.workers.dev/
curl -s -o /dev/null -w "/staff → %{http_code}\n" https://bethere-staging.solana-thailand.workers.dev/staff
```

---

## 9. Next step: activate the flow-harness + preflight gate

With staging live, the rest of Plan 005 unblocks:

1. **Wire `flow-harness` to staging** — replace the `// TODO(staging-live):` stubs
   with real HTTP calls against the staging URL (Plan 005 §3.4).
2. **Run the harness** — `cargo run -p flow-harness` against staging; expect all
   deposit/refund/claim flows green.
3. **Activate the preflight gate** (opt-in) — set `BETHERE_PREFLIGHT_GATE=1` so
   future *production* deploys require a green harness run within the last hour
   (`worker/deploy.sh#L121`). Until staging is live, the gate stays OFF (it
   would otherwise block all prod deploys with no way to get a green run).

---

## 10. Rollback / teardown

- **Roll back a bad staging deploy:** `npx wrangler deployments rollback --env staging`
  (the standard versions-API path; works for staging since it doesn't use the
  PUT fallback).
- **Tear down staging entirely (not recommended):** delete the Worker
  (`npx wrangler delete --env staging`), then the D1/KV/R2 resources. The
  resources are zero-cost on the free tier, so prefer to leave them in place.

---

## 11. Refs

- Plan 005 — `.plans/005_flow_verification_and_staging.md` (design + status)
- `worker/wrangler.toml` — `[env.staging]` block (provisioned IDs, vars)
- `worker/deploy.sh` — `staging` arg handling (`#L59`, `#L77-79`, `#L200`)
- `worker/scripts/seed-staging.sh` — idempotent staging seed
- `worker/.env.staging.example` — staging URL for plan 007 consumption
- `docs/mainnet_canary_mitigation_runbook.md` — §3 cluster-aware `ESCROW_PROGRAM_ID`
  (explains why staging must stay on `SOLANA_CLUSTER=devnet`)
- `docs/gradual_deploy_runbook.md` — normal-path canary (versions API)
```

I created `docs/staging_deploy_runbook.md` — the consolidated, operational staging-deploy checklist. It captures the **verified provisioning state** (D1/KV/R2 are already provisioned, contradicting Plan 005's stale "NOT STARTED" note), the **complete secrets checklist** with exact `wrangler secret put --env staging` commands, the **Google OAuth staging redirect URI** step (human action), D1 migration verification, the deploy sequence, seeding, isolation verification, and the flow-harness/preflight-gate activation that staging unblocks.

That completes the three autonomous items:
1. ✅ **PR #17 closed** — provably redundant (empty diff vs `develop`; content-twin `f4b75b1` already landed the identical reconciliation). Branch deleted, audit-trail comment left.
2. ✅ **Canary Path A/B recommendation updated** — `docs/mainnet_canary_mitigation_runbook.md` §2/§3 were stale (said `ESCROW_PROGRAM_ID` is *not* cluster-aware). PR #21 resolved exactly that — Option A2 implemented, all 6 consumer sites rewired, devnet e2e validated. I updated the runbook to mark the prerequisite **RESOLVED** and recommend **Path A** unambiguously (only ops-time work remains: fill `ESCROW_PROGRAM_ID_MAINNET` after deploy).
3. ✅ **Staging deploy checklist produced** — `docs/staging_deploy_runbook.md` (the secrets/infra checklist the dry-run would have produced, made actionable since I can't execute it without your CF credentials).

Now let me verify the canary runbook edit reads cleanly and commit everything. Let me view the updated §2-§3 region to confirm coherence, then commit with a conventional message.