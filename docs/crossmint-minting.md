# cNFT Minting via Crossmint — Ops Runbook

BeThere mints proof-of-attendance **compressed NFTs (cNFTs)** through
**Crossmint's hosted API**. Crossmint custodies the merkle tree, signs, and pays
fees, so the Worker needs **no on-chain signer**. This replaced Helius's retired
`mintCompressedNft` RPC (which now returns HTTP 410 `-32410 "Method no longer
supported"`).

Verified working end-to-end on **devnet** 2026-08-13.

---

## How it works

`worker/src/solana.rs::mint_compressed_nft` is wrapped by the shared attendee
mint journal in `worker/src/claim/mint/journal.rs`:

1. **Intent:** insert one `nft_mint_jobs` row before external I/O. The provider
   mint ID is the SHA-256 digest of the private claim token; the token itself is
   not sent in the URL.
2. **Fire idempotently:**
   `PUT https://{host}/api/2022-06-09/collections/{collection_id}/nfts/{provider_mint_id}`
   with header `X-API-KEY`, body:
   ```json
   { "recipient": "solana:<wallet>", "compressed": true,
     "metadata": { "name": "...", "description": "...", "image": "..." } }
   ```
3. **Poll:** `GET .../nfts/{id}` every 1.2s (max 12 polls ≈ 14s) until
   `onChain.status == "success"`, then return the tx signature + asset id.
   - Response mapping (defensive, unit-tested in `solana.rs`): asset_id ←
     `onChain.assetId` / `mintHash` / `address`; signature ← `onChain.txId`.
4. **Persist:** save the confirmed result in D1 before updating the attendee.
   KV retains a seven-day recovery copy until the attendee projection succeeds.
   A retry uses the same provider mint ID, so a crash or timeout cannot submit a
   second NFT.
5. **Reconcile:** the daily cron repairs a missing attendee projection from an
   exact confirmed journal result. It alerts on conflicts and pending jobs older
   than one hour without issuing a new provider request.

**Cluster is chosen by host** — `staging.crossmint.com` = devnet,
`www.crossmint.com` = mainnet. Helius DAS reads (`getAssetsByOwner`) are
unaffected and still used for on-chain verification.

**Images must be raster** — Crossmint rejects SVG. Our badge SVG is served with a
PNG twin (`/api/badge-hd.png`, pre-rendered `worker/src/badge_hd.png`); the mint
rewrites the `.svg` image URL to `.png` (`crossmint_image_url`).

---

## Config

| Key | Where | Value (devnet) |
|---|---|---|
| `CROSSMINT_HOST` | `wrangler.toml` var | `staging.crossmint.com` |
| `CROSSMINT_COLLECTION_ID` | `wrangler.toml` var (or secret) | `4130f03c-7246-4447-be09-2a6bc3196898` |
| `CROSSMINT_API_KEY` | **Worker secret** | *(staging server key)* |

Defaults: if `CROSSMINT_HOST` is unset it's derived from the `HELIUS_RPC_URL`
cluster (devnet→staging, mainnet→www). Empty API key or collection id → mint
returns a clear "crossmint not configured" error (not a crash).

Set the secret with:
```
cd worker && npx wrangler secret put CROSSMINT_API_KEY
```
(The collection id is NOT sensitive — keep it a plain var; a secret + var of the
same name collide.)

---

## Mainnet cutover checklist

1. In the Crossmint console, switch to **Production**, create a **Solana
   collection** (enable "Deliver using compression"), and add a **billing method**
   (card/credits — mainnet mints cost ~$0.01 each; devnet/staging is free).
2. Set the mainnet values:
   - `CROSSMINT_COLLECTION_ID = "<mainnet-collection-id>"` in `wrangler.toml`.
   - `CROSSMINT_HOST = "www.crossmint.com"` (or rely on the cluster default).
   - `npx wrangler secret put CROSSMINT_API_KEY` with the **production** server key.
3. Flip the Solana cluster: `SOLANA_CLUSTER = "mainnet-beta"` and
   `HELIUS_RPC_URL = "https://mainnet.helius-rpc.com"` (see the escrow mainnet
   runbooks for the on-chain side — `docs/mainnet_readiness_runbook.md`).
4. Deploy, then run one real claim and confirm a live `txId` + asset id.

> The payment/checkout settings on the Crossmint collection (NFT price, "who
> pays") are inert for us — we mint free via the API, never Crossmint's checkout.

---

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `crossmint not configured` | `CROSSMINT_API_KEY` secret or `CROSSMINT_COLLECTION_ID` unset. |
| `HTTP 400 ... non-supported extension ... metadata['image']` | Image is SVG. Ensure the event `nft_image_url` ends `.svg` (rewritten to the PNG twin) or points to a raster URL. |
| `HTTP 401` from Crossmint | Wrong environment key — a **staging** key against `www` (or vice-versa). Match key env to `CROSSMINT_HOST`. |
| Mint succeeds but `asset_id`/`signature` empty | Field-name mismatch — check the "crossmint poll #1 raw response" log line and extend the fallbacks in `parse_crossmint_success`. |
| `still pending after 12 polls` | Slow confirmation; retry uses the same Crossmint custom mint ID and resumes without a duplicate. Jobs older than one hour are alerted by the daily reconciliation pass. |

See also: `[[crossmint-minting]]` memory, `docs/architecture.md`.
