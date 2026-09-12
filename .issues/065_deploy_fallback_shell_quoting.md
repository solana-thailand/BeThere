# 065 — Remove shell interpolation from the production PUT fallback

Status: implemented locally — step 5 (isolated-Worker exercise) still open

## Problem

`worker/deploy.sh` embedded sizeable Python programs inside double-quoted
`python3 -c` arguments. Shell expansion stays active inside those strings, so
the Python source was interpreted as shell before Python ever started.

Two defects were confirmed by running the block with a stubbed interpreter and
a stubbed `wrangler` on PATH (2026-09-12):

1. **Backticks in the Python comments ran as commands.** The metadata program
   contained ``# Only `wrangler deploy` parses _headers`` and ``# ... default
   `max-age=0, must-revalidate` ...``. The shell executed both. The first is a
   literal `wrangler deploy` — a full production deploy fired as a side effect
   of *building the metadata JSON*, on a code path that is only reached because
   `wrangler deploy` already failed. Its stdout was then spliced into the Python
   source. On the machine used for this audit `wrangler` is not on PATH, so the
   substitution failed harmlessly; on any machine with wrangler installed
   globally (or `node_modules/.bin` on PATH) it would have run.
2. **A quoted word inside the string broke the shell quoting state.** Line 549
   contained `... falls back to the "devnet" default.`, which closed and
   reopened the double-quoted argument (ShellCheck SC2140). Benign as written
   only because no whitespace sat between the quotes; any future edit would
   have shattered the program across multiple argv words.

Separately, the Cloudflare OAuth token was interpolated into the program text,
so it appeared in the process command line.

## Configuration drift found while fixing it

The metadata generator carried a *hardcoded copy* of `[vars]` and three
bindings. Measured against `worker/wrangler.toml` on 2026-09-12, the copy:

- **omitted six declared variables** — `NOTIFICATIONS_ENABLED`,
  `NOTIFICATION_FROM`, `TELEGRAM_BOT_USERNAME`, `HELIUS_RPC_URL`,
  `CROSSMINT_HOST`, `CROSSMINT_COLLECTION_ID`. A fallback deploy would have
  reverted each to its in-code default, silently disabling Crossmint minting
  and the Telegram login widget.
- **sent four variables wrangler.toml has commented out** —
  `EVENT_DEPOSIT_ENABLED`, `EVENT_DEPOSIT_AMOUNT_USDC`,
  `EVENT_DEPOSIT_AMOUNT_THB`, `EVENT_PROMPTPAY_ID`.
- **omitted all four `[[ratelimits]]` bindings**, so a fallback deploy dropped
  production to the in-memory limiter without saying so.

So the fallback did not deploy the configuration `wrangler deploy` deploys.

## What was done

1. **Generators moved to tracked modules** — `worker/scripts/deploy_fallback/`:
   `manifest.py` (wrangler-compatible BLAKE3 asset hashes), `upload.py`
   (assets-upload-session + multipart upload + completion JWT), `metadata.py`
   (PUT script metadata), `mime.py` (per-extension Content-Type), `cli.py`
   (subcommands). No shell quoting, no interpolation of shell variables into
   Python text. `deploy.sh` now calls
   `python3 -m deploy_fallback.cli {manifest,upload,metadata}`.
   Verified the new manifest output is **byte-identical** to the old inline
   program against the real `frontend-leptos/dist` (37 assets).
2. **Secrets out of argv** — the OAuth token is passed as
   `CLOUDFLARE_OAUTH_TOKEN` and the assets JWT as `CLOUDFLARE_ASSETS_JWT`.
   Neither is printed. The CLI refuses to run if they are missing.
3. **Metadata is derived from wrangler.toml**, not copied. That removes the
   drift class above. What the PUT API genuinely cannot express is now printed
   as an explicit warning rather than dropped in silence: Durable Objects
   (10021), cron triggers, smart placement, `_headers` rules (#057), and the
   rate limiters.
4. **Tests** — `tests/deploy_fallback/`, 40 unit tests: MIME mapping (no
   shipped asset may resolve to `octet-stream`; the 2026-07-26 incident),
   metadata binding completeness checked against the **real** `wrangler.toml`
   so drift fails CI, API binding field names (`namespace_id` vs `id`), the
   manifest hash recipe and config-file exclusion, and the upload flow's three
   historical bugs (upload-all, JSON instead of multipart, `cfwau_` prefix).
5. **CI** — new `deploy-scripts` job runs `scripts/verify/shellcheck_gate.sh`
   plus the unit tests. The gate is **fail-closed**: it checks every `.sh` in
   the repo minus an explicit deferral list, so a newly added script is covered
   automatically rather than silently skipped. Verified by dropping a script
   containing the original defect (a backtick inside a double-quoted string)
   into the tree — the gate caught it and went clean again on removal.
   18 scripts gated, 6 deferred to #072.
6. **Portability** — the hardcoded `/opt/homebrew/bin/python3` is replaced by a
   `resolve_python` preflight that requires 3.11+ (tomllib) and the `blake3`
   package, overridable with `DEPLOY_PYTHON`, and fails before any upload
   starts rather than half-way through one.

## Remaining

- **Step 5 — exercise the fallback against an isolated Worker before using it
  for production again.** Owner-gated: it needs a real deploy. Until then the
  rate-limiter bindings stay withheld (`--include-ratelimits` is opt-in),
  because the `ratelimit` binding type is unverified against this API and a
  wrong type fails the whole PUT with 10021.
## Swept while clearing ShellCheck repo-wide

Enabling the gate meant every script had to be clean. Two were live defects of
the same class as the one above:

- **`scripts/backfill_first_event.sh:176`** — the health-check failure branch
  read ``fail "worker not healthy — is `bash deploy.sh dev --remote` running?"``.
  Unescaped backticks inside a double-quoted string are command substitution,
  so **failing the health check executed `bash deploy.sh dev --remote`**, and
  the message rendered with the text blanked out. Reproduced with a stub, then
  fixed by escaping; the message now renders intact and runs nothing.
- **`scripts/e2e/test_full_e2e.sh:136`** — a Python comment containing
  `{"alg":"HS256",…}` inside a `python3 -c "…"` argument closed and reopened
  the shell quoting (SC2140), the same trap as the `"devnet"` case above.

Correctness fixes:

- **`worker/scripts/migrate_kv_walkins_to_d1.sh`** — `SKIPPED=0` was dead: the
  skip is decided by SQLite's `INSERT … WHERE NOT EXISTS`, so the shell can
  never count it. Removed, and the summary line no longer claims
  "`$MIGRATED` record(s) migrated" when `MIGRATED` counts statements *written*,
  not rows inserted.
- **`scripts/d1/validate_d1.sh`** — four `cmd && pass … || fail …` chains are
  not if-then-else (`fail` also runs if `pass` returns non-zero). Harmless only
  because `pass`/`fail` are plain echoes; made explicit.
- **`frontend-leptos/optimize-wasm.sh`** — `WASM_OPT_FLAGS` is now an array;
  it relied on unquoted word-splitting to become separate argv words.
- **`frontend-leptos/build.sh`**, **`worker/scripts/check_size.sh`** — replaced
  `ls | head -1` with globs, preserving the unmatched-glob behaviour (verified
  both branches). `check_size.sh`'s SC2329 is a genuine false positive
  (`cleanup` runs via `trap`), suppressed with a reason rather than restructured.
- **`scripts/e2e/run_all_e2e.sh`**, **`tests/integration/run.sh`** — quoting and
  `local x=$(…)` return-masking fixes.

Six e2e/integration scripts are **deferred, not suppressed** — their unused
variables mark checks that were written and never wired up. Filed as
**#072**, since fixing them means deciding what each assertion should be.
