# Mainnet Deployment Checklist & Runbook

This runbook guides the step-by-step launch of **BeThere Protocol** on Solana Mainnet-Beta.

---

## 1. Environment & Constants Matrix

| Parameter | Mainnet-Beta Value | Notes |
| :--- | :--- | :--- |
| **Solana Cluster** | `mainnet-beta` | Target RPC cluster |
| **USDC Mint Address** | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | Native SPL USDC on Mainnet |
| **Mainnet RPC URL** | `https://mainnet.helius-rpc.com/?api-key=<HELIUS_API_KEY>` | Production RPC endpoint |
| **Worker Environment** | `production` | Cloudflare Workers `[env.production]` |
| **Deployer Keypair** | `~/.config/solana/id.json` | Requires ~3.0 SOL for program storage rent |

---

## 2. Pre-Deployment Preparation

### 2.1 Verify Deployer Wallet SOL Balance
Check that the deployer keypair has sufficient SOL on Mainnet-Beta (~3.0 SOL required for ~380 KB program bytecode rent exemption):

```bash
solana balance --url mainnet-beta
```

*If balance is < 3.0 SOL, transfer SOL from your primary wallet to the deployer keypair address:*
```bash
solana address
```

### 2.2 Verify Bytecode SHA256 Match
Verify that local `.so` matches Devnet/staging bytecode before deploying:

```bash
bash scripts/verify_devnet_binary.sh
```
Expected output: `✓ MATCH — on-chain bytecode == pinned source.`

---

## 3. On-Chain Smart Contract Deployment

### 3.1 Deploy Program to Mainnet-Beta
Deploy `event_checkin.so` to Mainnet:

```bash
solana program deploy ./target/deploy/event_checkin.so \
  --keypair ~/.config/solana/id.json \
  -u mainnet-beta
```

*Record the returned Program ID:*
```text
Program Id: <YOUR_MAINNET_PROGRAM_ID>
```

### 3.2 Verify Program Deployment
Verify that the program account is active on Mainnet:

```bash
solana program show <YOUR_MAINNET_PROGRAM_ID> -u mainnet-beta
```

---

## 4. Cloudflare Worker Production Configuration

### 4.1 Set Production Worker Secrets
Upload required secrets to the Cloudflare Worker `production` environment:

```bash
cd worker

# 1. Mainnet RPC API Key
echo "ba776ca2-44eb-44f2-b264-02caa12db98d" | npx wrangler secret put HELIUS_API_KEY --env production

# 2. Mainnet USDC Mint
echo "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" | npx wrangler secret put USDC_MINT --env production

# 3. Mainnet Program ID
echo "<YOUR_MAINNET_PROGRAM_ID>" | npx wrangler secret put SOLANA_ESCROW_PROGRAM_ID --env production

# 4. Production JWT Secret
echo "<STRONG_PRODUCTION_JWT_SECRET>" | npx wrangler secret put JWT_SECRET --env production

# 5. Production Google OAuth credentials
echo "<PROD_GOOGLE_CLIENT_ID>" | npx wrangler secret put GOOGLE_CLIENT_ID --env production
echo "<PROD_GOOGLE_CLIENT_SECRET>" | npx wrangler secret put GOOGLE_CLIENT_SECRET --env production
```

### 4.2 Deploy Production Worker
Deploy the worker to the live production endpoint (`https://bethere.solana-thailand.workers.dev`):

```bash
bash worker/deploy.sh production
```

---

## 5. Post-Deployment Verification & Smoke Tests

### 5.1 Health Check Probe
Query production health endpoint to confirm cluster configuration:

```bash
curl -s https://bethere.solana-thailand.workers.dev/api/health
```
**Expected Response**:
```json
{
  "status": "ok",
  "cluster": "mainnet-beta",
  "d1": { "connected": true }
}
```

### 5.2 Create Live Mainnet Event
Sync initial event brief (e.g. Meetup #5) to Production D1 database:

```bash
python3 scripts/sync_event_metadata.py \
  --yaml /Users/ozone/solana-thailand-devrel-helper/events/2026-08-23-in-person-meetup-5-solana-x-ai-builders.yml \
  --genesis \
  --seed-prod
```

### 5.3 Verify Live Public Quiz Endpoint
Confirm that the live quiz endpoint serves public questions:

```bash
curl -s "https://bethere.solana-thailand.workers.dev/api/quiz?event_id=solana-x-ai-builders-the-road-to-mainnet-5-bangkok"
```

---

## 6. Post-Launch Sweep & Rent Recovery

### 6.1 Sweep Unused SOL Back to Primary Wallet
Sweep leftover SOL from the deployer keypair back to your main wallet:

```bash
solana transfer <YOUR_PRIMARY_WALLET_ADDRESS> ALL \
  --keypair ~/.config/solana/id.json \
  -u mainnet-beta
```

### 6.2 Program Closure (If Ever Retiring Contract)
If you close the smart contract in the future, reclaim 100% of the storage rent (~2.8 SOL) directly to your primary wallet:

```bash
solana program close <YOUR_MAINNET_PROGRAM_ID> \
  --recipient <YOUR_PRIMARY_WALLET_ADDRESS> \
  --keypair ~/.config/solana/id.json \
  -u mainnet-beta
```
