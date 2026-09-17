# 121 — Frontend Solana cluster cache is only seeded on the dev dashboard

**Status:** Merged to `develop` (`197126d`, PR #131); staging `e6404cbb`; prod deploy awaits owner go
**Found:** 2026-09-17, while verifying the prod cluster config (mainnet `HELIUS_RPC_URL` next to `SOLANA_CLUSTER = "devnet"`)
**Severity:** Low today, High at escrow mainnet cutover

## Prod config: intentional, not a bug

Prod `/api/health` (version `8a3d6d9d`) reports `cluster: devnet`,
`solana.escrow_cluster: devnet`, `nft_cluster: mainnet-beta`, `rpc_cluster: mainnet-beta`,
`consistent: false` with `rpc_cluster_mismatch` + `nft_cluster_mismatch`. That matches
commit `8265d13` (badges-only mainnet go-live; escrow stays on devnet because
`ESCROW_PROGRAM_ID_MAINNET` is empty). No code consumes those warnings. The only prod event
with a USDC deposit is the `islanddao-v4-demo` devnet demo.

## What was wrong

`utils::fetch_cluster()` (the SEC-005 cache) was called only by `DevDashboard`. On every
other page `get_cluster()` returned its `"devnet"` fallback. Correct by accident while escrow
is on devnet, but after `SOLANA_CLUSTER = "mainnet-beta"`:

- the SEC-014 wallet-cluster guard (scanner, deposit refund, rollover, escrow init, admin
  escrow) would expect devnet and block mainnet wallets;
- escrow explorer links would point at devnet.

Also three links hardcoded `?cluster=devnet`: `onchain_events_panel.rs` (escrow txs),
`nfc_checkin.rs` (route disabled server-side), and `dev_profile.rs` (the profile wallet link,
which is **wrong in prod today**: badges live on mainnet).

## Fix

- `App` seeds the cache once at boot.
- The cache holds `SolanaNetworks { escrow, nft }` parsed by `parse_health_networks`
  (`nft` from `solana.nft_cluster`, falls back to escrow); `get_nft_cluster()` added.
- Escrow links use `solscan_tx_url(.., get_cluster())`; the profile wallet link uses
  `solscan_address_url(.., get_nft_cluster())`.

## Guard

`frontend-leptos/tests/cluster_networks.rs` (5 tests). Mutation-checked: reading
`escrow_cluster` instead of `nft_cluster` fails `prod_split_keeps_escrow_and_badge_clusters_apart`.

## Verification

wasm32 clippy `-D warnings` clean, `cargo fmt --check` clean, all frontend tests pass.
PR #131 CI: all 6 jobs green (incl. playwright e2e).

Staging `e6404cbb`, headless Chromium (2026-09-17):
- `/` issues exactly one `/api/health` request at boot (before the fix: zero outside the dashboard).
- `/dashboard` logged out issues three: boot, `DevDashboard`'s own pre-existing on-mount
  fetch, and a second boot after the full-page redirect to `/login?next=/dashboard`.
  The dashboard refetch is redundant but harmless (dev-only page); left as is.
- Not checked in a browser: the `/profile` wallet link. The page needs SIWS sign-in, and
  staging's `nft_cluster` is `devnet`, so it cannot tell the clusters apart anyway. The prod
  split is covered by `prod_split_keeps_escrow_and_badge_clusters_apart`.

After the prod deploy: signed in, `/profile` wallet link should carry `?cluster=mainnet-beta`.
