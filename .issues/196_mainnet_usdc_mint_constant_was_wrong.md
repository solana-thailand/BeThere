# 196 — The worker's mainnet USDC mint was not the USDC mint

**Status:** in progress. Fix and guard on `fix/usdc-mainnet-mint` (2026-11-23); no prod effect, mainnet escrow is not enabled.

## What

`worker/src/solana_escrow/mod.rs` declared `USDC_MINT_MAINNET` with the last
character wrong (`…wyTDt1m`). The real mint ends `…wyTDt1v`. Six docs and the
README carried the same wrong spelling; the deployment checklist and
`scripts/e2e/test_escrow_surfpool.sh` had the right one.

## Proof (2026-11-23, public mainnet RPC, `getAccountInfo`)

| Address ending | Result |
|---|---|
| `Dt1v` | owner `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`, type `mint`, 6 decimals |
| `Dt1m` | no account |

## Why nothing caught it

- Both spellings are valid base58, so parsing passes.
- The unit test asserted `usdc_mint_for("mainnet-beta") == USDC_MINT_MAINNET`: the constant against itself.
- Nothing selects `mainnet-beta` for the escrow yet (`ESCROW_PROGRAM_ID_MAINNET` is empty on purpose).

Had the escrow gone to mainnet as written, every deposit would have failed at
the mint account. It would have failed loudly, with no money at risk, but on
launch day.

## Fix

- The constant and the seven files now carry the real mint.
- `worker/tests/usdc_mint_guard.rs` pins the constant to the address read from
  chain and fails on any other spelling in the worker source or those docs.

## Found with it (2026-11-23 audit of `docs/mainnet_readiness_runbook.md`)

- `docs/mainnet_deployment_checklist.md` carried a literal value where a
  `HELIUS_API_KEY` placeholder belongs, since 2026-08-09. Replaced with the
  placeholder here. The value stays in git history, so the owner should
  rotate that key whether or not it is still in use.
- The same checklist is out of date in other ways (wrong binary name, an
  `--env production` that does not exist). Not fixed here; that is left for the
  runbook rewrite.
