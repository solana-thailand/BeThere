# cNFT Minting via Crossmint — Ops Runbook

BeThere mints proof-of-attendance **compressed NFTs (cNFTs)** through
**Crossmint's hosted API**. Crossmint custodies the merkle tree, signs, and pays
fees, so the Worker needs **no on-chain signer**. This replaced Helius's retired
`mintCompressedNft` RPC (which now returns HTTP 410 `-32410 "Method no longer
supported"`).

Verified working end-to-end on **devnet** 2026-08-13.

---

## How it works

`worker/src/solana.rs::mint_compressed_nft` (called from
`worker/src/claim/mint.rs`):

1. **Fire:** `POST https://{host}/api/2022-06-09/collections/{collection_id}/nfts`
   with header `X-API-KEY`, body:
   ```json
   { "recipient": "solana:<wallet>", "compressed": true,
     "metadata": { "name": "...", "description": "...", "image": "..." } }
   ```
2. **Poll:** `GET .../nfts/{id}` every 1.2s (max 12 polls ≈ 14s) until
   `onChain.status == "success"`, then return the tx signature + asset id.
   - Response mapping (defensive, unit-tested in `solana.rs`): asset_id ←
     `onChain.assetId` / `mintHash` / `address`; signature ← `onChain.txId`.
3. **Idempotency:** the created NFT id is persisted to KV
   (`crossmint:pending:<claim_token>`, 24h TTL) *before* polling, so a retry after
   a poll timeout resumes the same mint instead of firing a duplicate.

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
| `still pending after 12 polls` | Slow confirmation; the 24h idempotency marker lets the attendee retry and resume the same mint (no duplicate). |

See also: `[[crossmint-minting]]` memory, `docs/architecture.md`.
