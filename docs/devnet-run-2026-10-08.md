# Devnet run, 8 Oct 2026: one fresh deposit, end to end

Status: done 2026-10-08 00:31–00:41 Bangkok (2026-10-07 17:31–17:41 UTC),
session `event-checkin-90`, `.plans/044` item 1. Not a replay: a new event, a
new attendee wallet, new signatures. Every signature below is `Success` and
finalized on devnet (`getTransaction`, `commitment=finalized`, `meta.err =
null`), and each Explorer link was opened during the run (recording below).

- Program `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` (devnet)
- Devnet USDC `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`
- Event `devnet-run-20261008-0031` on staging; on-chain event id
  `16783445828572554312`; escrow (event) PDA `GeDV2z2ks36qcRzroCr5xBhRLwBYRNoqbBkYZmncL2rZ`; vault ATA
  `DeKvgkBFHT8C1Cf7en6wbgrwV9agVHufy1m3KtDRNXNF`
- Attendee: fresh keypair made for this run, `84GzG1BnMKd5Xt4AitCTmfftBmBMJG5xYCyAgZf9pGmt`
  (not the 27 Sep wallet). Organizer: `9Bz7p4RWdX7eaR4hFUeCc7aSZjDHsie8q1u8imwavkBN`.
- Script: `scripts/e2e/test_escrow_devnet.sh` steps 1–10 against staging
  (`BASE_URL=https://bethere-staging.solana-thailand.workers.dev`,
  `EVENT_END_SECS=600`, `STOP_AFTER_STEP10=1`). Result: 25 pass, 0 fail.

## Clock gates

| | unix | Bangkok |
|---|---|---|
| create_event landed | 1791394289 | 2026-10-08 00:31:29 |
| `event_end` (on-chain) | 1791394880 | 2026-10-08 00:41:20 |
| mark_checked_in landed (must be before `event_end`) | 1791394317 | 2026-10-08 00:31:57 |
| refund landed (must be after `event_end`) | 1791394887 | 2026-10-08 00:41:27 |
| `refund_deadline` (on-chain, `event_end` + 7 days) | 1791999680 | 2026-10-15 00:41:20 |

`event_end` and `refund_deadline` were read from the escrow account itself
(`getAccountInfo GeDV2z2ks36qcRzroCr5xBhRLwBYRNoqbBkYZmncL2rZ`, 192 bytes, owner = the program): the two i64 values
at bytes 114 and 122, the only unix times in the account.

## Steps

| step | signer | signature | slot | block time (Bangkok) | status |
|---|---|---|---|---|---|
| fund attendee 0.05 SOL | organizer `9Bz7p4…avkBN` | [`5F1X7Z5Y6g…`](https://explorer.solana.com/tx/5F1X7Z5Y6gnGWrUvzTq4AZZWWD1CHJgTVgQL2EzK8W68hqk6CCg72NR5v17qzx5G6YaaJCcTxGcgaPwbX9Kp2rHr?cluster=devnet) | 508,530,327 | 2026-10-08 00:29:19 | Success |
| fund attendee 2 USDC | harness wallet `7ABX2Z…LSNC` | [`4kfk6NQURa…`](https://explorer.solana.com/tx/4kfk6NQURaSARd8gVaayJe5kFEcRF5BLeDkz2Akdzn22UbsQxHvJAS5QwkTx4JtpYUgD2UzGmkF7DAxkqd1WN4sh?cluster=devnet) | 508,530,332 | 2026-10-08 00:29:20 | Success |
| create_event (init escrow) | organizer `9Bz7p4…avkBN` | [`3C5qn9cRQT…`](https://explorer.solana.com/tx/3C5qn9cRQTRyhH8uHM6RUSWyGM14xN1SGxjK6qTBTHmMYYNPXzpZvChK9t1iXQjcfsCQVGnciUwo3EBCbcdxbV1V?cluster=devnet) | 508,530,872 | 2026-10-08 00:31:29 | Success |
| deposit | attendee `84GzG1…pGmt` | [`VTdEGqbzuz…`](https://explorer.solana.com/tx/VTdEGqbzuzttmq2XWe7SRK4zpeo4BohozpXGKjPkhwfBduMLo2zZrjmZq9Hxeb8otC4pipiLB3cR5MxTioSNUZg?cluster=devnet) | 508,530,940 | 2026-10-08 00:31:45 | Success |
| mark_checked_in | organizer `9Bz7p4…avkBN` | [`3JzdKAXJUa…`](https://explorer.solana.com/tx/3JzdKAXJUas67TBpwXGjUCL2pxYPCdu1YXLp3vR1TgvmTxygidyoZ6scQPvPsbgcVHeeBQdgYqvQW6CGEQxP1C7W?cluster=devnet) | 508,530,993 | 2026-10-08 00:31:57 | Success |
| refund (+ close deposit) | attendee `84GzG1…pGmt` | [`ve3gPvkvBx…`](https://explorer.solana.com/tx/ve3gPvkvBx6qYweSqdcDycomgY8gY487VwMkxH2WLsAvjaK9uuMW2tPafgj2x8WgXj1mMxHmbfVUEkHcW3m9WgQ?cluster=devnet) | 508,533,386 | 2026-10-08 00:41:27 | Success |

The refund transaction also closes the deposit account ("Refund USDC and
reclaim deposit rent in one transaction"), so no separate `close_deposit`.

## USDC balances

From each transaction's `preTokenBalances` / `postTokenBalances`
(`getTransaction <sig> jsonParsed`), cross-checked against the script's own
`spl-token balance … --owner …` lines in the run log.

| moment | attendee `84GzG1…` | vault (owner `GeDV2z…`) | command |
|---|---|---|---|
| before deposit | 2 | 0 | `spl-token balance 4zMM… --owner 84GzG1… --url devnet`; vault from `create_event` post balance |
| after deposit | 1 | 1 | deposit tx `postTokenBalances` |
| after refund | 2 | 0 | refund tx `postTokenBalances`; script Step 10 `spl-token balance` |

Attendee SOL: 0.05 → 0.04999 (fees; deposit rent returned by the refund).

## Funding the fresh wallet (before the run)

0.05 devnet SOL from the organizer and 2 devnet USDC from the harness wallet
`7ABX2Z…` (6.98998 → 4.98998), rows 1–2 above.

## Recording

`~/Movies/bethere-devnet-e2e-2026-10-08.mov`: 1920×1080 H.264, 10:30, the live
run (terminal left, Explorer right; each transaction opened as it landed,
refund shown Finalized/Success at the end). Recorded as a fixed screen
rectangle (`screencapture -v -R`), so no other window, menu bar or Dock is in
it. The script output passed through a filter that replaces keypair paths and
the home directory; the run log was checked for `/Users`, `.json` and
`api-key=` (0 hits). No secrets on screen.
