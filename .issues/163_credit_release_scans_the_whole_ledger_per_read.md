# 163: Every balance read releases ended credit across the whole ledger

**Status:** open (2026-09-28). Found by session `event-checkin-fa` during the `/simplify` altitude review of `feature/028-w5-credit-release`. It is not a bug and it is not urgent at today's ledger size. W5 stays as built.

## What happens

`db::credit_ledger::release_ended_applies` runs before each balance read. It
is a D1 write that looks at **every** applied lock in the ledger, not just the
reader's. W5 halves the number of calls (`balances` covers both currencies)
and adds migration 0056's index. The migration notes still measure 9,339 VM
steps per read at 3,000 rows. The cost grows with the whole ledger, not with
one person's rows, and the index adds a write cost to every ledger insert.

The release also runs twice on some paths. `hold_admin.rs` calls `balance()`
right after a credit-apply batch that has already released. In signup,
`balances()` releases and then the apply batch releases again.

## Options

1. Scope the per-read release to the reader:
   `AND a.email IN (person_emails_of ?1)`. The cost then follows one person's
   rows.
2. Run the global release from the existing cron (`wrangler.toml` scheduled
   trigger) and keep the scoped release in the read path so a balance is fresh.
3. Add a read-only balance query for the calls that come straight after a
   batch that already released.

Re-measure with the plan 028 D1 step probe before choosing. Credit returns at
check-in OR event end, so the read must not wait for the cron if the reader's
own lock has just ended.
