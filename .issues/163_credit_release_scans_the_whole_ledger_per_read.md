# 163: Every balance read releases ended credit across the whole ledger

**Status:** in progress (2026-10-09, session `event-checkin-9c`): option 1 is rebased onto `develop` and has a PR. The PR also covers `try_refund`, which was added later. RTM #6 (4 Oct) has passed; the merge still waits on a staging credit-registration rehearsal, the same gate plan 028 set for W5. Built 2026-09-30 by session `event-checkin-dd`. No migration is needed. Found 2026-09-28 by session `event-checkin-fa` during the `/simplify` altitude review of `feature/028-w5-credit-release`.

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

## Built (2026-09-30, `feature/163-scoped-credit-release`)

I measured first, as asked above. SQLite 3.54 ran every migration on disk (the
0056 index included; also the uncommitted 0057 sponsor table, which has no
ledger columns). Each person has one `hold` and one `apply`, spread over
20 events, 18 of them ended. Steady state: every release is already written.
Figures are VM steps per release (`.stats on`):

| ledger rows | global release | `a.email IN person` | same, `+a.reason` |
|---:|---:|---:|---:|
| 290 | 1,917 | 1,187 | 107 |
| 2,900 | 18,747 | 11,087 | 107 |
| 29,000 | 187,047 | 110,087 | 107 |

Adding only the email filter is not enough. The planner still searches the
0056 `reason` index, walks every `apply` row, and filters by email with a
bloom filter, so the cost still follows the whole ledger. The unary `+` on
`a.reason` takes that index off the table. SQLite then seeks
`idx_credit_ledger_bal (email=?)`, and the plan and the 107 steps stay the
same after `ANALYZE` at 29,000 rows.

- **`credit_ledger.rs`:** one `release_ended_applies_sql!` template and two
  constants built from it. `RELEASE_ENDED_APPLIES_SQL` is the global release,
  unchanged. `RELEASE_PERSON_ENDED_APPLIES_SQL` binds `?1` to the person, over
  `person_emails_of!("?1")`, the same set the spend guard and balance reads
  sum.
- **Per-person readers:** `try_spend`, `balances` (and so `balance`),
  `positive_balances`, `locked_applies`, and the atomic apply batch in
  `credit_coverage.rs` now call `release_person_ended_applies`.
- **Whole-ledger readers keep the global release:** `liability`,
  `thb_balances_by_email`, `reconcile` (the daily cron is the backstop) and
  the payout queue in `contacts.rs`.
- **Why a scoped release is enough:** a person's balance sums only their own
  emails' rows, and both releases write the same
  `return:{event}:{email}` key, so they can never double-return.
- **Tests:**
  - `worker/tests/security/test_credit_balances.py`, three new tests on
    migrated SQLite:
    - the plan seeks by email, not by reason;
    - the scoped release returns only the person's (linked) ended applies,
      and a second run changes nothing;
    - for every email, the balance after the scoped release equals the
      balance after the global one.
  - `credit_release_and_staff_comp.rs`: the source guards now accept either
    release. A reader that uses the scoped one must read and bind the same
    `email_lc`.
  - I removed the `+` and the plan test went red.
- **Not done:** option 3, the second release right after a batch that
  already released (`hold_admin.rs`, signup). At 107 steps it is not worth
  another read path.

## 2026-10-09: rebase onto develop (session `event-checkin-9c`)

- The branch was rebased 201 commits forward with no conflicts.
- `try_refund` (the payout write from `.issues/190`) arrived after this
  branch was cut and still ran the global release. `TRY_REFUND_SQL` reads
  only `positive_buckets_of!("?1")`, so it now calls
  `release_person_ended_applies` too.
- This keeps the payout consistent with the check before it: both the
  organizer's `payable_of` check (`positive_balances`) and the write release
  the same person.
- `credit_payout_guards` now requires the person release before
  `TRY_REFUND_SQL`, bound to the same `email_lc`. The reader guard in
  `credit_release_and_staff_comp.rs` accepts `TRY_REFUND_SQL` as a
  person-scoped read; that constant's `?1` scope is pinned by
  `try_refund_is_one_guarded_statement`.
- The payout queue (`contacts::credit_refund_requests`) is whole-queue and
  keeps the global release.
