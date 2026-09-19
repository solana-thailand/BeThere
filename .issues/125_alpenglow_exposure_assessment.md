# 125 — Alpenglow: what it costs us (assessed: almost nothing, with one thing to watch)

**Status:** assessed 2026-09-19, **no work scheduled**
**Source:** <https://solana.com/upgrades/alpenglow> (owner, 2026-09-19)
**Severity:** informational — no defect, no deadline, nothing to redeploy

## What Alpenglow is

A replacement of Solana's **consensus** layer, in two phases — **Votor** (votes
move off the ledger, direct validator-to-validator, ~150ms finality; ships in
Agave 4.3) and **Rotor** (block propagation, later). Execution is explicitly
untouched: the SVM, transaction format, programs and fees are unchanged.
Protocol proposal is SIMD-0326.

The people it costs something are **validators** (must register a BLS key on the
vote account) and **RPC/indexer operators** (multiple candidate blocks per slot,
a new `bank_id` in Geyser streams, TPS metrics need re-baselining because vote
transactions vanish from blocks). We are neither.

## Our exposure, checked rather than assumed

**Off-chain (worker).** Every Solana call goes through plain JSON-RPC:
`getAccountInfo` (commitment `confirmed`), `getLatestBlockhash` (`finalized`),
`getAssetsByOwner`. No Geyser subscription, no block streaming, no `bank_id`, no
slot arithmetic, no TPS or transaction-count metric. The commitment levels keep
their meaning; `finalized` just arrives sooner. **Nothing to change.**

**On-chain (`bethere-escrow`).** This is the only real contact point. Six
handlers read `Clock::get()?.unix_timestamp`:

| handler | what the clock decides |
|---|---|
| `create_event` | reject an `event_end` already in the past |
| `deposit`, `rollover_deposit` | stamp `deposited_at` |
| `mark_checked_in` | refuse after `event_end` |
| `refund` | refuse before `event_end`; branch at `refund_deadline` |
| `claim_forfeited` | refuse before `refund_deadline` |

The solana.com page's one note for program developers is that code reading
`Clock.unix_timestamp` "may need logic review (new source, different bounds)".
For these comparisons it does not matter: the windows are hours and days, the
comparisons are plain `<`/`<=`/`>=` against stored second-resolution deadlines,
and the correction is bounded to 25% of elapsed epoch time. A drift of seconds
cannot flip a decision measured in days, and nothing here does arithmetic *on*
two timestamps whose sources could differ.

SIMD-0363 ("Simple Alpenglow Clock") would have moved the clock to nanosecond
resolution — in a **separate off-curve account**, explicitly preserving the
existing sysvar (`unix_timestamp` = nanoseconds / 1e9, floored). It was closed
as stale on 2026-06-21 and never merged. Either way the legacy field survives.

**No redeployment is required**, which is just as well: SBPFv3 deployment is
still disabled on every cluster, so we could not redeploy right now anyway
(`.issues/123`).

## What would change this

- A future SIMD that changes `Clock.unix_timestamp`'s **semantics** rather than
  its source — that would be a real review of the six handlers above.
- Us taking on anything that streams blocks (a Geyser-based indexer, a
  block-scanning reconciler). Today's reconcile reads D1, not the chain.

## Verify activation, when it matters

`solana alpenglow-genesis-info`, or the `getAgGenesisCert` RPC method — `null`
while the cluster is still on TowerBFT, certificate details once migrated.

## Related

- `.issues/123` — SBPFv3, the other Solana-side upgrade, and the one with an
  actual (distant) deadline. `scripts/check_sbpf_v3_gate.sh` watches it.
