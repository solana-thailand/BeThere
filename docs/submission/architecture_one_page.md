# BeThere on one page

One commitment layer, two ways in: **people pay in baht** (checked by the slip
agent) and **agents pay in USDC on Solana** (settled by the escrow program).
Everyone who shows up gets their deposit back and a badge on mainnet.

```mermaid
flowchart LR
  subgraph People["People (THB, production)"]
    A[Attendee phone] -->|slip image + QR read in browser| W
  end
  subgraph Agents["AI agents (USDC, devnet)"]
    C[Claude via MCP] --> M[bethere-mcp<br/>agent's own wallet]
    M -->|register| W
    M -->|sign + send deposit| E
  end
  W[Cloudflare Worker<br/>Rust → WebAssembly] --> D[(D1: source of truth)]
  W --> R[(R2: slip images)]
  W -.mirror.-> S[Google Sheet<br/>organizer view]
  W -->|slip_agent: deterministic checks| P[(slip_proposals<br/>bank_ref UNIQUE)]
  P -->|proposal shown next to slip| O[Organizer queue<br/>approves or rejects]
  W -->|Solana Pay tx request| E[bethere-escrow<br/>devnet]
  O -->|door: mark_checked_in| E
  E -->|refund after event| A2[Attendee / agent wallet]
  W -->|badge mint via Crossmint| B[Solana mainnet<br/>compressed NFT]
```

## The three flows judged in this window

| Flow | What decides | Where | Evidence |
|---|---|---|---|
| **Slip check (AI proposes, checks dispose)** | Deterministic rules: reference never seen (DB `UNIQUE`), amount, time window, receiver. The organizer makes the final call | prod, shadow mode | `worker/src/handlers/deposit/thb/handlers/slip_agent.rs`, migration `0054` |
| **Escrow loop** | The Solana program: `deposit` → `mark_checked_in` → `refund`; `claim_forfeited` refuses checked-in attendees | devnet | `bethere-escrow/src/lib.rs`, `scripts/e2e_devnet_test.sh` (green 27 Sep) |
| **Agent commits for a human** | The escrow again; the agent only signs with its own key, within an operator spend cap | staging + devnet | `bethere-mcp/`, Claude run tx `v9H1A84w…` |

## Design rules that hold across all three

- **No model output moves money.** The slip agent writes proposals only;
  guards live in `worker/tests/slip_duplicate_guards.rs`.
- **D1 is the source of truth.** The Google Sheet is a mirror, and a failed
  mirror write never blocks a money action (`.issues/155`).
- **One `domain` crate** is shared by the worker (wasm), the frontend (wasm)
  and the native tools, so the slip QR is parsed by the same code in the
  browser and on the server.
- **0 THB running cost target.** It runs on the Cloudflare free plan. The
  paid vision model is off; the planned OCR runs in the browser.
