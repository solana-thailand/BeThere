# 127 — `thb_deposits` allows two rows per (event, attendee), and the save path races

**Status:** the archive consequence is **fixed and deployed**; the root cause is
**open** and deliberately not fixed here
**Raised by:** a devrel-helper session reading this repo read-only, 2026-09-19,
hours after #126 shipped. Reproduced here before anything was changed.
**Severity:** high if it ever fires (silent money loss), zero occurrences in prod

## What they found

`thb_deposit_archive` (migration 0044) was `UNIQUE (event_id, attendee_id)`.
`thb_deposits` is not:

- `worker/migrations/0013_thb_deposits.sql:24` creates
  `idx_thb_deposits_attendee ON thb_deposits(event_id, attendee_id)` as a plain
  **INDEX**, no UNIQUE.
- `worker/src/event_store/write/deposit.rs:83-98` (`save_thb_deposit`) is
  read-then-insert-or-update on that same pair — a check-then-act with no
  constraint behind it, so two concurrent uploads for one attendee can both take
  the insert branch.

So `cleanup.rs` archived one row of a duplicate pair and deleted both. Worse, the
old return was `SELECT COUNT(*) FROM thb_deposit_archive WHERE event_id = ?1` —
every row for the event, not the rows that run inserted — so the summary still
read as success.

**Reproduced against the production migrations with the real archive and delete
SQL, before any change:**

```
deposits before : 2 rows, 1200 THB
archived        : 1 row,   500 THB
deposits after  : 0 rows            <- 700 THB gone, run logged success
```

Not happening today: prod has **0** duplicate pairs (`HAVING COUNT(*) > 1` → 0,
`> 0` → 47 against 47 rows, so the zero is a real none and not a query reaching
nothing). Staging: 0. Confirmed independently of their check.

## Fixed (migration 0045, deployed)

**The archive is keyed on the deposit row.** `source_deposit_id` +
`UNIQUE(source_deposit_id)`, and `ON CONFLICT (source_deposit_id) DO NOTHING`.
`thb_deposits.id` is `INTEGER PRIMARY KEY AUTOINCREMENT`, so it is unique for
the life of the database and never reused — re-archiving the same id really is
the same deposit. The index is a plain UNIQUE, not partial, so the `ON CONFLICT`
needs no repeated WHERE (the trap `credit_ledger` hits).

Additive: pre-existing rows carry `source_deposit_id IS NULL`, and SQLite treats
NULLs as distinct in a UNIQUE index, so they neither conflict nor block. Both
archives were empty when this was written; the migration does not depend on it.

**And the delete now checks coverage by matching ids.**
`archive_thb_deposits_for_event` returns `ArchiveCoverage { archived,
unarchived }`, where `unarchived` counts live deposits with no archive row
carrying their id, and the delete runs only when it is 0. A successful write is
not the same as a complete archive, and the next way to lose a row will not be
the duplicate-pair way.

The first version of this gate was `archived >= live`, and the devrel-helper
session was right to push on it: that asks whether the archive is *big enough*,
which is a different question. It passes whenever `archived` is inflated by rows
corresponding to nothing currently live — 0044-era rows carrying a NULL
`source_deposit_id`, or rows kept from an earlier purge of an event that has
since taken new deposits. It was safe in practice, because the `INSERT … SELECT`
is what actually guarantees every live id is archived and the gate was only a
backstop. But a check whose stated meaning is not the one it computes rots the
moment someone changes what else the archive holds, so it now computes the
stated one. Pinned by a test that builds the inflated case: three archive rows
for the event, none of them the live deposit — the old form passes, the new one
reports `unarchived = 1`.

Same reproduction after the fix: `archived 2 rows, 1200 THB`, `complete=true`,
`MONEY LOST 0 THB`.

## Open — the root cause

The duplicate pair is still *creatable*. Closing it properly means:

1. `CREATE UNIQUE INDEX ... ON thb_deposits(event_id, attendee_id)` — safe today
   (0 duplicates in prod and staging) but it will start **rejecting** writes that
   currently succeed silently.
2. Turn `save_thb_deposit` into a single upsert on that constraint, which also
   removes the check-then-act race rather than just failing it loudly.

Both touch the deposit **write** path — the money-in path — and RTM#6 is
2026-09-27. Adding a constraint to a live money table and rewriting its save in
the same week as an event is not a change to slip in beside an archive fix. It
wants its own change, its own staging soak and a quiet week.

Until then the archive is correct regardless of what `thb_deposits` allows,
which is the property that actually protects the money.

## Related

- `.issues/126` — the purge and the archive this came out of.
- `.issues/022` / plan 022 — the state-transition writer sweep; the same
  check-then-act shape, which is why "guards land on one entry point" keeps
  recurring here.
