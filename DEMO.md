# 🎮 BeThere — Live Demo Script

> **Duration:** 3 minutes | **Network:** Solana Devnet | **Wallet:** Phantom

## Pre-Demo Setup (do before recording/live)

```bash
# 1. Start the worker
cd worker && npx wrangler dev --port 8787

# 2. Ensure Phantom wallet has devnet SOL
# Open Phantom → Settings → Developer Settings → Testnet Mode → Get SOL
# Or: solana airdrop 2 <WALLET_ADDRESS> --url devnet
```

## Demo Flow

### 1. Landing Page (15 sec)
- Open `http://localhost:8787`
- Show the swimlane: Organizer → Attendee → Staff flow
- Key stat: "30-40% no-show rate for free events"

### 2. Create Event (30 sec)
- Login as organizer (Google OAuth)
- Go to Admin → Events → Create Event
- Set deposit amount: $5 USDC
- Show the event config: quiz, adventure, NFT badge settings

### 3. Attendee Deposit (45 sec)
- Open deposit page as attendee
- Connect Phantom wallet
- Click "Deposit USDC"
- **Show Solana Pay QR** → scan with Phantom
- Confirm on-chain transaction
- **Open Solscan** → show the real on-chain TX
- Key stat: "$0.00087 transaction cost"

### 4. Staff Check-In (20 sec)
- Switch to staff scanner
- Scan attendee QR code
- Show check-in confirmed
- Key stat: "< 500ms latency, edge-deployed"

### 5.5. Rollover Deposit (Optional — 15 sec)
- Show attendee's ticket page after check-in
- Click "Roll Over Deposit" → wallet signs → deposit moves to next event atomically
- Key stat: "Atomic on-chain rollover — no withdraw + re-deposit needed"

### 5. Claim Refund + NFT Badge (30 sec)
- Attendee opens claim page
- Complete quiz (1-2 questions)
- Click "Claim Refund" → wallet signs → USDC returned
- Click "Mint Badge" → cNFT minted
- **Show in Phantom wallet** → the NFT badge appears
- Key stat: "$0.001 mint cost, 990x cheaper than POAP"

### 6. No-Show Path (20 sec)
- Show organizer dashboard
- "Attendee didn't show up → deposit forfeited"
- Organizer claims forfeited deposit → USDC goes to organizer
- Key point: "Skin in the game works"

### 7. Wrap-Up (10 sec)
- Cost comparison: $0.87 for 1000 attendees vs $500+ on Ethereum
- "We started with manual deposits — now it's on-chain"
- "100% Rust, open source, ready for mainnet"

## Key Talking Points

| Point | Detail |
|-------|--------|
| **Problem** | 30-40% no-show rate for free events |
| **Solution** | USDC deposit commitment, refundable on check-in |
| **Cost** | $0.001 NFT, $0.00087 TX — 990x cheaper than alternatives |
| **Security** | 15 findings audited, 12 fixed, on-chain escrow with time-locked refunds |
| **Innovation** | Dual-track (USDC + PromptPay THB), quiz/adventure gating, atomic deposit rollover, 100% Rust |
| **Traction** | 85 tests (47 unit + 38 on-chain SVM), devnet validated, ready for mainnet |

## What If Demo Fails?

| Issue | Fallback |
|-------|----------|
| Wallet won't connect | Show pre-recorded video of deposit flow |
| Devnet is slow | Show Solscan TX links from previous runs (see handover 052) |
| Worker crashes | Show architecture diagrams from `docs/presentation_materials.md` |
| QR scan fails | Use manual attendee lookup instead |

## Real Devnet TXs (Backup Evidence)

| Action | TX Signature |
|--------|-------------|
| Init escrow | `2YMQwjLRbTX3TD3uso3wxx6rP8aipJbHpb3A3B3LXtVcTMhnS9n1wv1CWjo8dTcEqNvxaFPW9SQwnLGYKfZ5hJ8G` |
| Deposit | `4cQnNGRa5CHfcuWGzmE2LU7cUj58KreenSswnuypnZyVYTApx2vU7KPqeDYRjNyn8x1WHe6GunnwehS8CUuuoEJe` |
| Refund | `5PA5wPRnHuhqSrvPs8T7t4nS3yjZtoV3sLzJtwYes3zSyW5sKDZ8CXQj1a1ep7gEyvPoV4UsPvgXwd6KEgGtWVHT` |
| cNFT mint | `4omCGAuSYEj5yCoif3soUGMGy7sZdXvZvXUfL3dUF5goYhhxrzj6LDrndKohEML6zX96fZSkvBTtJ8riwbBFP2Sh` |
| Rollover deposit | _(pending — see handover 077)_ |

View on Solscan (devnet): `https://solscan.io/tx/<SIGNATURE>?cluster=devnet`
