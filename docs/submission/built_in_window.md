# What was built inside the hackathon window (14 Sep – 12 Oct 2026)

Colosseum judges only the work done between the competition's start and end
dates, and requires disclosure of everything before it. This page is that
disclosure, from the repository's own history. Re-cut the counts on the day of
submission; the commands are given with each one.

## Before the window (disclosed, not claimed)

- BeThere has run real events in Bangkok since spring 2026. The first commit
  is on **21 Apr 2026**, and **1,167 of 1,389 commits** predate 14 Sep
  (`git rev-list --no-merges --count --before=2026-09-14 HEAD`).
- It was submitted to a previous Colosseum hackathon (**Frontier**, Consumer
  Apps, `colosseum.com/arena/projects/bethere`).
- These already existed: THB deposits over PromptPay with manual refunds,
  rolling deposit credit, QR check-in, quiz-gated badge claims (Crossmint,
  mainnet), and the `bethere-escrow` program on **devnet**.

## Inside the window: 222 commits, 15 migrations, 43 tracked issues

`git rev-list --no-merges --count --since=2026-09-14 HEAD` → 222. Of these, 104
are `feat`/`fix`. `worker/migrations` gained 0041–0055.

### 1. AI and Solana in one act: an agent commits for a human (plan 033 W3)

- **`bethere-mcp`** is a new standalone MCP server with seven tools:
  `find_events`, `event_details`, `agent_wallet`, `register`,
  `ticket_status`, `deposit_tx`, `pay_deposit`. It wraps existing public
  routes. The agent signs the escrow deposit with **its own** devnet wallet;
  BeThere never holds an agent key. There is a spend cap
  (`BETHERE_MAX_DEPOSIT_USDC`), `pay_deposit` is idempotent, and `register`
  requires consent.
- **Verified with Claude as the MCP client:** a headless Claude Code session
  got a plain-language task, then found the event, registered, paid 1 USDC and
  confirmed the ticket (8 turns, $0.54). Tx
  `v9H1A84wzLPieB4bp7exkLcrcjVDLdzcR9pkhkZwe4sC2yqEHtTJLoYzWjegqsaTTiYiXC2QzpXDL4XbpQtQB4B`,
  finalized on devnet. Record: `bethere-mcp/README.md`, plan 033 W3 logs.
- CI gates `bethere-mcp` and `flow-harness` (fmt, clippy, tests, floors) as
  of 27 Sep.

### 2. The escrow exercised end to end (plan 033 W2)

- `scripts/e2e_devnet_test.sh` covers the whole loop on devnet: init escrow →
  deposit → verify → `mark_checked_in` → `refund` → deposit account closed.
- Green on 23 Sep (`d67f06fb`, `.issues/074`) and on 27 Sep. The 27 Sep
  transactions, all finalized with no errors:
  - deposit `5xPB2EbLey7nyFmLrfCmTJZJaZCcRJ3gU5bRKZxyxHSptcQYiVk3EiM8Jwauf4hcvMsJoVxdsoAB6Jy3682mLKgG`
    (attendee −1.00 USDC, escrow +1.00);
  - check-in `4VtB7Ctj2fzaz3sQGQxQimQybuzMjXekXXzaUeEfTk6jsfR6qoFrSYqmq2hPc2sifL3f6ToECL7vwHntFUE8GCph`
    (organizer key, no token movement);
  - refund `QgsveL7uLf7zBC8Xw6RNBExBMuTz3D8DMLCC7Ug4AMRTXJBEhpc2jBEz7bUa9kMQZkhuFkUu3WK6W71ZR8PNSxC`
    (escrow −1.00, attendee +1.00).
- The escrow's instruction-data encoder now lives in one place and is pinned
  to the program (plan 030). Real devnet account bytes serve as decoder
  fixtures (`.issues/074`, `.issues/075`).
- **Still true, and stated:** no real attendee money has gone through the
  escrow. Real deposits are THB. Mainnet is on the roadmap.

### 3. Slip agent: AI proposes, checks dispose (plan 033 W1)

- The Thai slip mini-QR parser lives in `domain` and is shared by worker and
  browser. The browser decodes the QR (self-hosted jsQR), and the worker
  re-parses it.
- A **deterministic checker** judges each slip: reference never seen (a
  database `UNIQUE`, not code), amount, time window, receiver. It proposes
  `accepted` / `needs_review` / `rejected`.
- **Shadow mode, on prod since 27 Sep:** the organizer still decides, and
  the proposal shows next to each slip. Admin-recorded slips get a proposal
  too.
- The duplicate-image fingerprint flags the same slip image sent by two
  attendees.
- A Claude vision fallback exists but is **off**, because the owner's target
  is 0 THB running cost. The planned free path is OCR in the browser.

### 4. One person, several emails (plan 025)

- Linked emails share rolling credit, and one registration and one badge per
  person per event. Self-service Google linking, plus super-admin
  link/unlink with a required reason and an audit entry.
- One badge per recipient wallet per event: `UNIQUE (event_id, wallet)`
  (migration 0055), with a 409 in place of a second mint.
- A possible-duplicate flag on the roster (same name, wallet or handle under
  another email).

### 5. Organizer tooling around real events

- **Postponement toolkit** (plan 032): a banner, per-audience announcements
  on the ticket, "can you still come?" answers, and a refund-queue filter.
- **Money correctness fixes**, each with a guard test:
  - admit without owing a refund;
  - write-off with a reason;
  - bank info on held credit;
  - auto-verify honoured on admin slips (`.issues/154`);
  - holding as credit no longer depends on the Sheets mirror (`.issues/155`);
  - slip names come from D1;
  - a sheet row-addressing bug (`.issues/151`) plus a D1 → sheet backfill.
- **Security** (plan 029): magic-byte validation on slips, credentialed-401
  and 429 spike alerts, and a self-hosted, SRI-pinned jsQR. The PII log
  guards predate the window (12 Sep) and are not claimed here.
- **Verification** as CI gates: a post-deploy smoke test that writes, deploy
  provenance tags, issue-status-vs-git checks, bundle-size budgets,
  test-count floors, and a wasm leak scan.

## How to re-check any line here

- `git --no-pager log --since=2026-09-14 --no-merges --format='%ad %s' --date=short`
- Transactions:
  `https://explorer.solana.com/tx/<signature>?cluster=devnet`
- Deployed state: `npx wrangler deployments list` (prod `7448a398` =
  git `c6c09380` on 27 Sep).
