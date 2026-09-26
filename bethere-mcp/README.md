# bethere-mcp

An MCP server (stdio) that makes an AI agent a BeThere attendee. The agent
finds an event, registers a person, and pays the attendance deposit into the
**Solana devnet escrow from its own wallet**. This is plan 033 W3: the one
place where AI and Solana are the same act.

The agent's key stays in this process, on the agent's machine. BeThere only
sees a Sign-In-With-Solana signature and the signed deposit transaction; the
escrow program, not this server, decides whether the deposit is valid.

## Tools

| Tool | Worker route(s) it wraps |
|---|---|
| `find_events` | `GET /api/public/events` |
| `event_details` | `GET /api/public/event/{slug}` |
| `agent_wallet` | devnet `getBalance` for the agent's address |
| `register` | SIWS (`/api/auth/wallet/nonce` + `/verify`), then `POST /api/public/register` |
| `ticket_status` | `GET /api/public/ticket/{id}?event_id=` + `GET /api/deposit/status/{id}?event_id=` |
| `deposit_tx` | `POST /api/deposit/usdc` + `GET /api/deposit/usdc/tx` (unsigned preview) |
| `pay_deposit` | `deposit_tx`, sign locally, send to devnet, `POST /api/deposit/usdc/webhook`, poll `/api/deposit/usdc/confirm` |

There are no new worker endpoints: each one is already called by the web app.

**Safety rails:**
- Staging worker + devnet RPC only. The guard is flow-harness's
  `validate_live_target`.
- A per-deposit spend cap (`BETHERE_MAX_DEPOSIT_USDC`, default 10 USDC). The
  operator sets it; the agent cannot raise it.
- `pay_deposit` is idempotent. A verified deposit returns "already verified".
  A sent but unverified deposit is only polled, never paid a second time.
- `register` refuses to run unless `consent_given` is true.
- IDs interpolated into URL paths are restricted to `[A-Za-z0-9._-]`.

## Build and run

```sh
cd bethere-mcp && cargo build --release      # standalone crate, like flow-harness
cargo test                                   # offline: protocol, catalogue, guards
```

| Env var | Default |
|---|---|
| `BETHERE_API_URL` | `https://bethere-staging.solana-thailand.workers.dev` |
| `BETHERE_RPC_URL` | `https://api.devnet.solana.com` |
| `BETHERE_AGENT_KEYPAIR` | unset: the read-only tools still work |
| `BETHERE_MAX_DEPOSIT_USDC` | `10000000` (10 USDC, 6 decimals) |

**Claude Code:**

```sh
claude mcp add bethere --env BETHERE_AGENT_KEYPAIR=$HOME/.config/solana/bethere-agent-demo.json \
  -- ~/.cargo/target/release/bethere-mcp
```

The agent wallet needs a little devnet SOL for fees and devnet USDC of mint
`4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` (the mint the staging escrow uses).

## Demo fixture

`examples/demo_fixture.rs` creates a public staging event with a 1 USDC
deposit and initializes its devnet escrow. The organizer signs; it uses the
same two admin calls as the Manage Events escrow panel.

```sh
BETHERE_ORGANIZER_KEYPAIR=~/.config/solana/id.json cargo run --example demo_fixture
```

Afterwards, two manual steps:
- Activate the event: `PUT /api/events/{id}` with `{"status":"active"}`.
  Create ignores `status`.
- Give it one D1 attendee row. Until an event has D1 attendees, registration
  reads Google Sheets for its duplicate check, and the fixture's placeholder
  `sheet_id` 404s there. `worker/scripts/seed-staging.sh` does the same.

## Verified run (2026-09-27, staging, devnet)

- Event `agent-demo-meetup-1790455251`; agent wallet `54GKocGxYkGbcbAnLAq43toZj9oYyNSmgaqZmarxX3Yi`.
- `register` → attendee `01a0df80-89f9-7b43-85db-212b2e4e72b7`. Next step: `deposit`.
- `pay_deposit` → tx `5uBPpdQcZ9e4nQzS48gcYkcyUahbuexftBAusjBqxaxEDcMEqYewwtSQ6WAgcToSn8V6c5FtwajUoKCXheYrwqmo`,
  `finalized`, `err: null`. `verified: true`.
- A second `pay_deposit` → "already verified", with no new transaction.
- Opened the ticket page in headless Chrome: event name, QR, "Deposit verified",
  "Ready for Check-In", no page errors.
