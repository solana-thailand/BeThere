---
name: deploy-guard
description: Ordered pre/post-deploy checklist for the bethere worker (staging or prod). Load before running worker/deploy.sh, applying D1 migrations, or claiming anything is "deployed". Prod still needs an explicit owner go in the current session.
---

# deploy-guard

One ordered path from "tree is green" to "deploy verified". Every step has a
stop condition. A red step ends the run; do not skip ahead and do not
"deploy anyway, fix after". Why each step exists is linked, not retold.

`<env>` is `staging` or `prod`. Values per env:

| | staging | prod |
|---|---|---|
| D1 name | `bethere-db-staging` | `bethere-db` |
| wrangler env flag | `--env staging` | (none) |
| deploy | `worker/deploy.sh staging` | `worker/deploy.sh` |
| base URL | `https://bethere-staging.solana-thailand.workers.dev` | `https://bethere.solana-thailand.workers.dev` |

## 0. Gate: who said go

- **prod:** an explicit owner go **in this session**, naming prod. A go from a
  handover, an earlier session or a plan doc is not a go. No go → stop and ask.
- **staging:** ungated, but check `ListAgents` first. A peer mid-deploy owns
  staging until it says otherwise.

## 1. Tree

```sh
git --no-pager status --short        # nothing unexpected staged or modified
git --no-pager log --oneline -1       # record this SHA; it is what you deploy
```

Stop if the SHA is not on `develop` (staging) or `main` (prod). "Pushed" is not
"deployed" and "deployed" is not "pushed"; record both separately.

## 2. D1 backup (prod always; staging when a migration is pending)

```sh
cd worker && npx wrangler d1 export <db> --remote [--env staging] \
  --output backup-<env>-$(date +%Y%m%d-%H%M).sql
```

Stop if the file is empty or the command errors. The backup holds PII: it
never enters git, and `git status` must not list it.

## 3. Build + size budgets

```sh
frontend-leptos/build.sh                          # dist/ is not in git
bash scripts/verify/frontend_size_budget.sh
bash scripts/verify/worker_size_budget.sh
```

Stop on a budget failure. Do not `--update-baseline` to get past it without
saying why in a commit. If `BUILD_TAG` needs a bump, `deploy.sh` says so; see
memory `cloudflare-assets-content-type-poisoning`.

## 4. Ledger

```sh
python3 scripts/verify/issue_ledger.py --strict
```

The ledger carries older flags (28 on 2026-09-24, mostly `UNVERIFIABLE`), so
a red exit alone is not the stop condition. Stop if a flag names an issue
this deploy ships, or if the flag count went up since the last run in the run
log below. An issue that claims "fixed on develop" for code you are about to
ship gets its repro re-run, not trusted (memory `issue-status-needs-a-verdict`).

## 5. Migrations (before the code)

`deploy.sh` does **not** apply migrations.

```sh
cd worker && npx wrangler d1 migrations list <db> --remote [--env staging]
cd worker && npx wrangler d1 migrations apply <db> --remote [--env staging]
```

Then read the schema back (`PRAGMA table_info(<table>)` via
`wrangler d1 execute --command`) and confirm the new columns / constraints
exist. For every `CHECK`, `NOT NULL` or `UNIQUE` a migration adds, `rg` every
writer of that table and check what it binds for the absent case
(`.issues/138`, memory `migration-constraints-break-existing-writers`).
Never `d1 execute` an `events` row (KV masks it).

## 6. Deploy

```sh
worker/deploy.sh staging      # or: worker/deploy.sh   (prod, owner go only)
```

The prod path runs the §3.5 preflight gate (green flow-harness within the
hour). `--force --reason "..."` bypasses it and writes
`worker/scripts/.preflight-bypass.log`, which is never committed. Needing
`--force` is itself something to tell the owner.

Then confirm it landed: `cd worker && npx wrangler deployments list [--env staging]`.
The newest deployment's timestamp must be this run's.

## 7. Post-deploy smoke (reads **and** writes)

```sh
bash scripts/verify/post_deploy_smoke.sh                        # staging
bash scripts/verify/post_deploy_smoke.sh --url <prod> --i-know-this-writes
```

It checks status **and** Content-Type. A 200 with `application/octet-stream`
on a JS/wasm asset is a failure, not a pass. Frontend changes are also verified
by opening the page (`docs/web-verification-runbook.md`).

## 8. Write volume (prod; staging tables are near-empty)

For every table a migration in this deploy touched, and always for
`thb_deposits`:

```sql
SELECT substr(uploaded_at,1,10) AS day, COUNT(*) FROM thb_deposits
 WHERE uploaded_at >= date('now','-5 days') GROUP BY day;
```

Run it again a few hours after deploy. A zero on deploy day, against a
non-zero baseline, is an outage until proven otherwise.

## 9. Status updates

- Move each shipped issue's `Status:` to `deployed` (or back to `open` if a
  check failed), using the vocabulary in `issue_ledger.py --vocab`.
- Record the SHA, env and deployment id in the relevant plan or issue.
- Only now say "deployed".

## Run log (capped)

Append one line per run, per step outcome, to `.git/deploy-guard.log`. It is
per-clone, never committed, and capped at 200 lines:

```sh
log=.git/deploy-guard.log
printf '%s\t%s\t%s\t%s\n' "$(date -u +%FT%TZ)" "<env>" "$(git rev-parse --short HEAD)" "<step>: <ok|FAIL detail>" >> "$log"
tail -n 200 "$log" > "$log.tmp" && mv "$log.tmp" "$log"
```

No secrets, tokens, URLs with query strings or attendee data in the log.
