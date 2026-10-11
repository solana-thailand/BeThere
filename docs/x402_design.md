# x402: bookable by agents (Super GOAT SG2, design only)

Status: **design, no build until the owner picks an option** (`.plans/045` SG2,
devrel `SUPER-GOAT-SPEC.md` §SG2). Devnet only.

## 1. The idea

An agent that asks for a seat gets `402 Payment Required` with the deposit
terms, pays, and retries with proof. Any x402-capable agent could then book,
not only our MCP server (`bethere-mcp`, which already pays the escrow deposit
from its own wallet with a 10 USDC cap).

## 2. What x402 is (checked against docs.x402.org on 2026-10-11)

- **Protocol version 2.** All three headers carry Base64-encoded JSON:
  - `PAYMENT-REQUIRED` (server → client): the `PaymentRequired` object, i.e.
    the schemes the server accepts, price, network and destination.
  - `PAYMENT-SIGNATURE` (client → server): the `PaymentPayload`.
  - `PAYMENT-RESPONSE` (server → client): the `SettlementResponse`.
  - (The spec note of 7 Oct named the same headers. Version 1 used
    `X-PAYMENT`; do not build against v1 examples.)
- **Networks** are CAIP-2 ids. Solana devnet is
  `solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1` (mainnet:
  `solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp`).
- **Token:** USDC is the default asset; its devnet mint is
  `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`, **the same mint our escrow
  uses on devnet**. Any SPL token is allowed.
- **The `exact` scheme supports SVM.** The payment is a token transfer to
  `payTo`, signed by the payer, usually with a separate fee payer (the
  facilitator pays the fees). Verification checks the instruction layout
  (or, for smart wallets, simulates and looks for a valid `TransferChecked`).
- **Facilitator:** a server may verify and settle itself or call a
  facilitator's `/verify` and `/settle`. In the default flow verification runs
  before the handler and settlement after it; with `paymentFlow: "upfront"`,
  `/settle` validates and commits at once.
- **Duplicates:** a server that settles Solana payments itself must reject a
  payload that is already being settled (the Go SVM implementation ships a
  `SettlementCache` "for duplicate detection").

Field names inside `PaymentRequirements` (amount, asset, payTo,
`maxTimeoutSeconds`, `extra`) are not all spelled out on the overview pages.
Confirm them against the published schema before building.

## 3. The catch: `exact` is custodial for us

`exact` moves USDC to a `payTo` address. A BeThere deposit must go through the
escrow program: `deposit` creates the attendee's deposit account and moves
USDC into the event's vault (a PDA). `refund` later closes that account and
returns the USDC to the wallet that paid it. If `payTo` were a BeThere wallet,
BeThere would hold the money, which contradicts the product's central claim.

## 4. Options

### (a) A BeThere scheme whose payload is the escrow `deposit` transaction (recommended)

- The `402` lists a scheme of our own, for example `bethere-escrow-deposit`,
  with the terms. The client builds and signs the escrow `deposit` instruction
  for this event and its own wallet, and sends it in `PAYMENT-SIGNATURE`.
- The server verifies before submitting:
  - program id = the escrow program;
  - instruction = `deposit`;
  - event escrow PDA and vault = this event's;
  - amount = the event's configured USDC deposit;
  - the attendee deposit PDA belongs to the paying wallet;
  - no other instructions, except a compute-budget allowlist;
  - a recent blockhash.
- The server then submits the transaction, waits for confirmation, and answers
  `200` with `PAYMENT-RESPONSE` (signature, network, payer).
- **Non-custodial:** the USDC lands in the program's vault. **No program
  change:** this is the transaction `bethere-mcp` already signs.
- **Cost:** generic x402 clients only pay schemes they know, so ours works with
  `bethere-mcp` and any client that adds it. We would publish the scheme. A
  `402` can list several schemes, so `exact` could be added later for (c).

### (b) `exact` to the vault, plus a server-side deposit record

- A standard x402 client could pay: a `TransferChecked` to the event vault's
  token account.
- **The program cannot accept this today.**
  - A plain transfer creates no attendee deposit account, so `refund` has
    nothing to close and nothing records who paid.
  - It also breaks the vault's accounting: `close_event` refuses unless the
    recorded deposits equal refunds plus forfeits **and** the vault's actual
    balance is zero. It checks the balance precisely because anyone can send
    tokens to the vault outside the program, and a plain transfer is that
    case.
- Making it work needs a new instruction that attributes an existing vault
  transfer to an attendee, plus a re-audit of the accounting. That is large,
  and it weakens the property that the program alone decides refunds.

### (c) x402 only for non-refundable parts

Nothing BeThere sells is non-refundable today, so there is nothing to put
behind `exact`. Keep this open for a future paid add-on.

## 5. The `402` body for option (a)

The shape, with field names to confirm against the v2 schema:

```json
{
  "x402Version": 2,
  "accepts": [{
    "scheme": "bethere-escrow-deposit",
    "network": "solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1",
    "asset": "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU",
    "amount": "<event deposit_amount_usdc, 6 decimals>",
    "maxTimeoutSeconds": 90,
    "extra": {
      "programId": "<escrow program id>",
      "onChainEventId": "<u64>",
      "eventEscrow": "<PDA>",
      "vault": "<vault token account>",
      "eventEnd": "<unix seconds>",
      "refundDeadline": "<unix seconds>",
      "refundRule": "Refunded to the paying wallet: after check-in once the event ends, and for a no-show until the refund deadline."
    }
  }]
}
```

- **Amount:** the event's own configured USDC deposit, not a fixed "500 THB
  equivalent". Events set their deposit; the escrow enforces that amount.
- **Refund rule, stated as the program enforces it:** a checked-in attendee
  refunds after `event_end`, and a no-show can still refund until
  `refund_deadline`. After the deadline the program lets the organizer claim
  an unclaimed deposit (`claim_forfeited`).
  - The owner's policy (2026-09-28) is that nothing is forfeited: BeThere does
    not use that path.
  - The `402` should say what the code allows and what BeThere does, without
    promising more than the program enforces.

## 6. Duplicate settlement

- **Key:** the transaction's first signature. It is fixed once signed, so a
  replayed payload has the same key.
- **Claim before submitting:** use the same `advisory_locks` claim the sandbox
  quotas use (`sandbox/quota.rs`), with a key like `x402:settle:{signature}`
  and a TTL of about two minutes (the blockhash lifetime plus a margin).
  - A second request while the first is in flight gets `409`.
  - A request after the first one settled gets the stored `PAYMENT-RESPONSE`
    again.
- **Second layer, on chain:** `deposit` initializes the attendee deposit PDA,
  so a second deposit for the same attendee and event fails anyway. The cache
  exists so the server never submits twice and never reports a duplicate as a
  new payment.

## 7. The agent's 10 USDC cap

- `bethere-mcp` refuses any deposit above `BETHERE_MAX_DEPOSIT_USDC` (default
  10 USDC) before it signs. Under x402 the same check applies to the `402`'s
  `amount`.
- **The agent must not trust the `402` alone.** A spoofed or compromised
  server could ask it to sign something else.
- So the MCP keeps building the `deposit` instruction itself, from the
  program id and mint it is configured with and from the public event details
  (`event_details`). It uses the `402` only for the terms, and refuses when the
  `402` disagrees with them (other program, mint, vault or amount).

## 8. What option (a) would take to build

- **Worker:**
  - an endpoint that answers `402` for an event with an open USDC escrow;
  - a verifier for the signed `deposit` (section 4a);
  - the settle cache (section 6);
  - submit and confirm, reusing the escrow builders and
    `verify_attendee_deposit_onchain`.
  - Devnet only, by `SOLANA_CLUSTER`.
- **MCP:** a tool that follows a `402` with the cross-check in section 7.
- **Tests:** the verifier refuses each wrong field; a replay gets the cached
  response; the cap refuses above 10 USDC; one devnet end-to-end run.
- **Estimate:** M (about 2–3 days). Option (b) is L and needs a program change
  and audit.

## 9. Decisions for the owner

1. Pick (a), (b) or (c). Recommended: **(a)**.
2. Who pays the transaction fee: the agent (simplest), or BeThere as fee payer
   (friendlier, but a devnet SOL budget and abuse caps like the sandbox's).
3. Do generic x402 clients matter now? If yes, only (b) serves them, at the
   cost in section 4b.
