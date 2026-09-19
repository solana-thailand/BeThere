# 126 — The nightly purge deleted the deposits without keeping the amounts

**Status:** archive built 2026-09-19 (not deployed) · R2 orphans open (owner) ·
RTM#3 not restored, deliberately
**Raised by:** the DevRel agent next door, which read prod D1 and changed
nothing. Verified here against the code, prod D1, prod KV, prod R2 and the
2026-09-17 backup before anything was built.
**Severity:** high — it is a schedule, not an incident. RTM#4's 20 deposits
(10,000 THB) are due **2026-10-17 17:00 +07**, 28 days out.

## What happened

`worker/src/cleanup.rs` phase 2, at `event_end + refund_deadline + 90 days`:

```rust
let financial_cutoff = event_end_secs + refund_deadline_secs + RETENTION_FINANCIAL_SECS;
if now_ms / 1000 > financial_cutoff {
    crate::db::thb_deposits::delete_thb_deposits_for_event(db, event_id).await
```

which is `DELETE FROM thb_deposits WHERE event_id = ?1`. It behaved exactly as
written. The 90-day window is defensible: that table holds `attendee_name`,
`bank_account`, `bank_name`, `account_name` and a URL to an image of a bank
slip.

What is not defensible is that the **amounts** went with them. RTM#3's 14 rows
(7,000 THB) were deleted on 2026-09-19 at 03:00 UTC.

## Verified, including two corrections to the report

| claim | verdict |
|---|---|
| hard `DELETE`, ungated | **confirmed** — and the old `if let Err(e) = … { warn }` swallowed failures, so nothing could ever stop it |
| RTM#3: 14 rows / 7,000 THB gone from prod | **confirmed** — 0 rows now, 14 in `bethere-db-20260917-pre-118-119.sql` |
| "3 rows / 1,500 THB never marked refunded" | **corrected → 1 row / 500 THB.** The 14 are 11 `refunded` + 2 `held_as_credit` + **1 neither**. Two of the three unrefunded rows ended in credit, which is a terminal state, not a gap — and they match the 2 `credit_ledger` holds (1,000 THB) exactly |
| cleanup never touches R2 | **confirmed** — no R2 reference in the module |
| slip images outlived their rows | **confirmed — 13 of 13 still in `bethere-assets`**, 4.5 MB, probed object by object with a positive *and* a negative control (a first probe that reported "0 present" was wrong: it grepped for `not found` while wrangler says `The specified key does not exist`) |
| RTM#4 due 2026-10-17, RTM#5 2026-11-21 | **confirmed exactly** — but only if you read `refund_deadline_hours` from KV. It is **1**, not the 7-day default the code falls back to; assuming the default puts both dates a week late |
| `credit_ledger` survives and covers only credit | **confirmed** — 2 rows against those 14 deposits |
| the id/slug off-by-one | **half right.** Real for *duplicated* events: RTM#4's id is `…mainnet-3-bangkok-copy`. RTM#3 is an original — its id **is** `…mainnet-3-bangkok`. Checking RTM#3 under a `-copy` id finds nothing and looks like a clean bill |

### One more, found here

The cron deletes the financial rows and **leaves `attendees` alone** — 55 rows
for RTM#3, with names and emails, are still in prod. `credit_ledger` keeps the
email forever too, and is never cleaned. So the purge removes bank details and
slips but not identity; the privacy rationale for destroying the amounts was
weaker than it looked, because the people were never actually forgotten.

## Built (not deployed)

**Migration `0044_thb_deposit_archive.sql`** — a retained, never-cleaned table
holding what the money reconciliation needs and nothing personal: event id +
denormalised slug, attendee id, amount, verified/refunded/held flags and their
timestamps, and two booleans for *whether* a slip and a refund proof existed.
Deliberately absent: `attendee_name`, `bank_account`, `bank_name`,
`account_name`, `slip_url`, `refund_proof_url`, `verified_by`. `attendee_id` is
kept — it is an opaque uuid that `attendees`, `deposit_statuses` and the audit
log all retain anyway, and without it the archive can say "every deposit was
resolved" but not "here is what happened to yours".

`event_slug` is denormalised because the archive outlives the event row, and
reading the id alone is how you conclude RTM#4's deposits belong to RTM#3.

**`db::thb_deposits::archive_thb_deposits_for_event`** — one
`INSERT … SELECT … ON CONFLICT (event_id, attendee_id) DO NOTHING`, so a re-run
cannot double-count. It returns the archive's **coverage** for the event, not
rows inserted, so a retry after a partial failure is still allowed to proceed.

**The delete is now gated on it.** A failed archive logs at `error` and leaves
the rows for tomorrow's run. Same reasoning as the payout reversal in #120: the
data is gone for good either way, so the only recoverable choice is not to
delete.

Tests: `worker/tests/security/test_deposit_archive.py` (8) runs the real SQL
against the production migrations — full coverage before the delete, the
11/2/1 reconciliation surviving, no personal column and no personal *value*
carried over, evidence kept as a boolean rather than a pointer, idempotence,
partial-retry, and the slug fallback. Reverse-patched to drop `held_as_credit`
and exactly one test failed. `worker/tests/cleanup_guards.rs` (3) guards the
gate; reverse-patched to a compiling ungated version, the gate guard failed as
intended.

## Open — owner's call

1. **The orphaned slips.** 13 RTM#3 bank-slip images are in R2 with nothing left
   to say whose they are. Both options are defensible and they are opposites:
   delete them (the purge finishes what it started) or index them (they are
   payment evidence). Deleting from R2 is irreversible, so it is not something
   to do on a peer's report and a 28-day clock. Whatever is decided, cleanup
   should do it **at purge time** rather than leaving objects behind — the
   archive's `had_slip` already records that one existed.
2. **RTM#3.** Do not restore yet — the cutoff is in the past, so tonight's run
   would delete the rows again. Once this ships, a restore lands in the archive
   on the next pass. The one row worth chasing is the 500 THB that is neither
   refunded nor held.
3. **`credit_ledger` as the system of record** (the report's option b) — making
   it record deposit_received / verified / refunded would make "neither boolean
   set" structurally impossible. Bigger than this issue; worth a plan.

## Related

- `.issues/079` — the id/slug divergence that makes this easy to check wrongly.
- `.issues/118` — why held credit is a terminal state, not an unpaid refund.
- `~/solana-thailand-devrel-helper/.issues/002_rtm3_thb_deposits_missing_from_prod.md`
  — the original report, and their PII-free snapshot of all 61 deposits.
