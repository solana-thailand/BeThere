# On-Chain CPI Event Indexing

## Overview

The BeThere escrow program emits 9 on-chain event types via `emit!()`. The CPI event indexing system bridges these on-chain events with the off-chain audit trail in KV.

### Event Types

| Discriminator | Event             | Instruction      | Description                     |
|---------------|-------------------|------------------|---------------------------------|
| 0             | `EventCreated`    | `create_event`   | Escrow PDA initialized          |
| 1             | `Deposited`       | `deposit`        | Attendee deposited USDC         |
| 2             | `CheckedIn`       | `mark_checked_in`| Attendee checked in on-chain    |
| 3             | `Refunded`        | `refund`         | Attendee claimed refund         |
| 4             | `ForfeitedClaimed`| `claim_forfeited`| Organizer claimed forfeited     |
| 5             | `EventClosed`     | `close_event`    | Escrow closed, rent reclaimed   |
| 6             | `EventDeactivated`| `deactivate_event`| Registration closed           |
| 7             | `DepositClosed`   | `close_deposit`  | Attendee deposit PDA closed     |
| 8             | `DepositRolledOver` | `rollover_deposit` | Deposit moved to new event escrow (`source_escrow`, `target_escrow`, `attendee`, `amount`) |

## Architecture

### Indexing Modes

1. **Helius Webhook** (primary) — Real-time push via `POST /api/escrow/onchain-webhook`
2. **Manual Sync** (admin) — Triggered via `POST /api/escrow/sync`
3. **Daily Cron** (fallback) — Polls `getSignaturesForAddress` during cleanup

### KV Key Schema

| Key Pattern                        | Value                           | Purpose              |
|------------------------------------|---------------------------------|----------------------|
| `event:{id}:onchain`               | `Vec<OnChainEvent>` (max 200)  | Per-event events     |
| `onchain:sig:{signature}`          | `"1"`                           | Dedup marker         |
| `onchain:cursor:{escrow_addr}`     | Last processed signature        | Polling cursor       |

### Data Flow

```
Escrow Program → emit!() → Solana Block
                              ↓
                    Helius Indexer picks up TX
                              ↓
                    Helius Webhook fires
                              ↓
                    POST /api/escrow/onchain-webhook
                              ↓
                    Parse instruction discriminator + accounts
                              ↓
                    Resolve escrow_address → event_id
                              ↓
                    Save OnChainEvent to KV
                              ↓
                    Append to audit trail
```

## API Endpoints

### POST /api/escrow/onchain-webhook

**Public** — Called by Helius when a transaction involves the escrow program.

Request body (Helius enhanced webhook format):
```json
{
  "transactions": [
    {
      "signature": "5UfDuX7...",
      "slot": 253123456,
      "timestamp": 1703001234,
      "transactionError": null,
      "instructionData": [
        {
          "programId": "C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T",
          "data": "<base58 instruction data>",
          "accounts": ["attendee_pubkey", "escrow_pda", ...]
        }
      ],
      "tokenTransfers": [
        {
          "tokenAmount": 15.0,
          "mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m"
        }
      ]
    }
  ]
}
```

Response:
```json
{
  "ok": true,
  "data": {
    "indexed": 1,
    "duplicates": 0,
    "skipped_failed": 0,
    "skipped_no_event": 0,
    "errors": 0
  }
}
```

### POST /api/escrow/sync

**Protected** — Manual sync trigger. Polls `getSignaturesForAddress` for recent transactions.

Request:
```json
{
  "event_id": "solana-bangkok-2025"
}
```

Response: Same `IndexSummary` format as webhook.

### GET /api/escrow/events/{event_id}

**Protected** — Query indexed on-chain events for an event.

Response:
```json
{
  "ok": true,
  "data": {
    "event_id": "solana-bangkok-2025",
    "escrow_address": "DRp7...base58",
    "events": [
      {
        "signature": "5UfDuX7...",
        "slot": 253123456,
        "block_time": 1703001234,
        "instruction": "deposit",
        "escrow_address": "DRp7...base58",
        "organizer": null,
        "attendee": "ATTENDEE_WALLET_BASE58",
        "amount": 15000000,
        "indexed_at": "2025-01-15T10:30:00Z"
      }
    ]
  }
}
```

## Helius Webhook Setup

### 1. Create the webhook

```bash
curl -X POST "https://api.devnet.helius-rpc.com/v0/webhooks" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "webhookURL": "https://bethere.solana-thailand.workers.dev/api/escrow/onchain-webhook",
    "transactionTypes": ["Any"],
    "accountAddresses": ["C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T"],
    "webhookType": "enhanced",
    "authHeader": "your-secret-token"
  }'
```

**Key settings:**
- `accountAddresses`: The escrow program ID — all transactions to this program trigger the webhook
- `webhookType`: `"enhanced"` for parsed transaction data (instruction data, token transfers, etc.)
- `authHeader`: Optional secret for webhook authentication (configure in wrangler secrets)

### 2. Verify webhook registration

```bash
curl "https://api.devnet.helius-rpc.com/v0/webhooks?api-key=YOUR_API_KEY"
```

### 3. Test with a deposit transaction

1. Initialize escrow via the admin UI
2. Make a USDC deposit
3. Check the webhook response in Cloudflare Workers logs

## Retention & Cleanup

On-chain events follow the same retention policy as financial data:

| Key Pattern              | Retention                          |
|--------------------------|------------------------------------|
| `event:{id}:onchain`     | Deleted at financial cutoff (90d past event end + refund deadline) |
| `onchain:sig:*`          | Purged daily by cron               |
| `onchain:cursor:*`       | Deleted with onchain events        |

Indexed events also appear in the per-event audit trail as `OnChainEventIndexed` entries.

## Security Considerations

1. **Webhook authentication**: Configure `authHeader` in Helius webhook settings and validate in the handler
2. **Dedup**: Signature-based dedup prevents double-counting from webhook retries
3. **Failed TX filtering**: Transactions with errors are skipped
4. **Access control**: Sync and query endpoints require staff/organizer auth
5. **Non-blocking**: Indexing failures don't affect main operations
6. **Rollover events**: `DepositRolledOver` events are indexed the same way as other deposit events — the indexer resolves both `source_escrow` and `target_escrow` addresses to their respective event IDs
