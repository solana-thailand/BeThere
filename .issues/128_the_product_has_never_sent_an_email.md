# 128 — The product has never sent an email, and turning it on would send 243 of them

**Status:** open. The §3 trap is now **guarded in code** (see §5, 2026-09-22);
the three remaining blockers are unchanged and the flag stays at `0`.
**Do not wire the caller or flip the flag until the sender domain is onboarded.**
**Found:** 2026-09-21, verifying a claim in
`~/solana-thailand-devrel-helper/reports/phase-2/BETHERE-HACKATHON-BRIEF.md` §P4.1
**Severity:** high — 24 people are registered for RTM#6 on 2026-09-27 and none has
been told anything; and the obvious fix would email ~187 people about events that
ended weeks ago

## 1. Confirmed: nothing has ever been sent

`notification_outbox` in production, 2026-09-21:

| | |
|---|---|
| rows | **487** |
| `attempts = 0` | **487** |
| `attempted_at IS NULL` | **487** |
| `status = 'pending'` | 268 |
| rows belonging to RTM#6 | 62 |

Not one row has ever been attempted, since the queue was created.

## 2. Four blockers, not the two the brief names

The brief says dispatch is *"gated on `NOTIFICATIONS_ENABLED` and has no caller"*.
Both true, and there are two more that matter for the estimate:

1. **`NOTIFICATIONS_ENABLED = "0"`** — `wrangler.toml:68` (prod) and `:291`
   (staging). `dispatch` returns `Ok(())` immediately.
2. **`notifications::dispatch` has no caller.** The cron runs
   `cleanup::run_cleanup`, `credit_ledger::reconcile` and
   `nft_mint_jobs::reconcile`, and never reaches it. Verified by search.
3. **`NOTIFICATION_FROM = ""`** — `transport::Sender::from_env` validates it with
   `is_plausible_email` and returns `Err("NOTIFICATION_FROM invalid")` **before
   any job is claimed**.
4. **There is no `EMAIL` binding.** `wrangler.toml:371-373` has the
   `[[send_email]]` block commented out, with the note *"Enable after onboarding
   a sender domain to Cloudflare Email Sending"*.

So the code change is small and the **critical path is not code**: it is
onboarding a sender domain, which needs DNS records on a domain the owner
controls and has verification lead time. Nothing the worker does can shorten it.
If email is to reach anyone before 2026-09-27, that step starts first, not last.

## 3. The trap: flipping the flag would send 243 emails

`cancel.sql` runs before the claim loop, but it drops only **25** of the 268
pending rows. Modelled against production with the real predicate:

```
pending_now = 268   would_cancel = 25   would_actually_send = 243
```

And what those are:

| kind | event | rows | oldest due (ICT) |
|---|---|---|---|
| survey | RTM#1 | 40 | 2026-09-14 |
| survey | RTM#3 | 28 | 2026-09-13 |
| **registration** | **RTM#6** | **24** | 2026-09-16 |
| **reminder** | **RTM#6** | **24** | 2026-09-26 13:00 |
| survey | RTM#4 | 22 | 2026-09-13 |
| survey | RTM#2 | 21 | 2026-09-14 |
| survey | latent-space 2–5 | 65 | 2026-09-14 |
| **deposit_confirmed** | **RTM#6** | **14** | 2026-09-16 |
| survey | RTM#5 | 11 | 2026-09-13 |

The 62 RTM#6 rows are the ones anybody wants. The ~187 survey rows are for
events that are long over — `cancel.sql` deliberately keeps post-event kinds
alive after the event ends (`.issues/080`), which is correct for a survey sent on
time and wrong for one queued since 13 September.

**The absence of a caller was, until 2026-09-22, the only thing preventing
that** — see §5 for the guard that replaced it. Wiring the caller *before*
deciding what to do with the backlog removes the last safety catch, so the order
still matters:

1. decide the backlog (cancel the stale surveys, or defer them, or scope the
   first enable to RTM#6 kinds only);
2. onboard the sender domain and set `NOTIFICATION_FROM`;
3. wire the caller;
4. enable, on staging first, and watch the first run.

`reminder` timing is already safe: `claim.sql` gates it on
`event_start_ms <= now + 86400`, so RTM#6's 24 reminders cannot fire before the
26th regardless.

### 3.1 Correction: it was never 243 *at once*

The claim loop is `for _ in 0..25` and `[triggers] crons = ["0 3 * * *"]` is
daily, so the true worst case was always **25 a day for ten days**, not 243 in
one burst. That is not a defence — 243 wrong emails delivered over ten days is
still 243 wrong emails, and the first 25 are the same accident — but the
wording in this issue and in `.plans/027` B overstated the mechanism, and the
rate deserved to be a named control rather than a literal nobody could see. It
is now `NOTIFICATIONS_MAX_PER_RUN`.

## 4. Why it is worth doing anyway

A booking product that never confirms a booking is the most visible gap in it,
and the fix is dateable inside the Colosseum window (14 Sep – 12 Oct). Twenty-four
people registered for Sunday have had no confirmation, no deposit receipt and
will get no reminder.

## 5. What was built, 2026-09-22

Two `[vars]` controls, both applying at send time rather than enqueue time, so a
genuinely delayed message still goes out. Neither changes `NOTIFICATIONS_ENABLED`.

### `NOTIFICATIONS_STALENESS` — `off | report | cancel`, default `report`

Per-kind maximum age, in `notifications::policy::max_age_secs`:

| kind | max age past `due_at` | why |
|---|---|---|
| `reminder` | 2 days | says "tomorrow"; `claim.sql` already gates it to the 24 h window and `reminder_timing` cancels it once the event starts. The cap only has to survive a cron outage. |
| `survey` | 3 days | "how was it" stops being a question and becomes an apology. |
| `registration`, `deposit_confirmed`, `deposit_rejected` | 14 days | still literally true until the event ends (which `cancel.sql` already handles), so this is a judgment call, not a correctness one. |

**Why not the flat 72 h `.plans/027` B specified.** It would have taken RTM#6's
own mail with it: the 24 `registration` and 14 `deposit_confirmed` rows were due
2026-09-16, six days old when this was written and eleven on the event day. A
single cutoff cannot fit both clocks — a confirmation is true until its event
ends, months out if need be, while a survey is wrong days after its event ended.
Measured against the §3 table, the per-kind version retires the ~187 surveys and
keeps all 62 RTM#6 rows.

The 14-day cap has a real one-sided cost: someone who registers more than a
fortnight before an event that is still upcoming loses their confirmation.
`report` is the default so that case gets counted on a live queue first.

Modes:

- `off` — no age check. Escape hatch.
- `report` — stale rows are **withheld from the claim loop** and logged per kind
  with the oldest `due_at`; nothing is modified. Withholding in `claim.sql`
  rather than at `prepare` is what makes the mode observable: a stale row never
  spends one of the run's slots, so the backlog does not drain past the guard
  while it is being watched.
- `cancel` — retires them as `cancelled` / `error_code='STALE'`. Not
  `NO_LONGER_ELIGIBLE`: the row *was* eligible, the queue was slow, and the two
  are worth telling apart when reading the outbox back. Only `pending` and
  `failed` are touched; `accepted`, `sending` and `uncertain` keep their outcome.

Unset or unrecognised parses as `report`, mirroring `THB_SLIP_DUPLICATE_MODE`
(`.issues/129`) — a typo can neither disable the guard nor start deleting mail.

### `NOTIFICATIONS_MAX_PER_RUN` — default 25, clamped `1..=100`

Names the rate the loop already ran at (§3.1). A run that spends its whole
budget logs a warning, which is the only outward sign that the queue is behind.

**Why not the stop-the-world burst gate `.plans/027` B specified** ("if the
first pass would exceed 25, send nothing and alert"): RTM#6's legitimate backlog
is 62 rows. That gate would have tripped on exactly the mail the owner wants
sent, and stayed tripped until someone raised the limit by hand. `report` mode
already provides the "a human looks first" step, without wedging the queue.

### Where the numbers live

One place: `policy::max_age_secs`. `policy::max_age_case_sql` emits the `CASE`
from `NotificationKind::ALL` and `staleness::render` splices it into
`claim.sql`, `stale_report.sql` and `stale_cancel.sql`, so a kind added to the
enum cannot be left out of the SQL. `worker/tests/notifications/test_outbox.py`
parses the same function out of `policy.rs` rather than restating the table, and
asserts it found all five kinds — otherwise an empty parse would make every row
stale (`ELSE 0`) and the suite would pass by accident.

### Verified

- `cargo test` in `worker/` — the age table, the mode parser, the SQL/enum
  coupling, and a proof that no flat cutoff separates a 45-day-old registration
  from a 4-day-old survey.
- `python3 -m unittest discover -s worker/tests/notifications` — 36 tests
  against the real migrations and the real statements: both directions (stale
  withheld, fresh delivered), `off` still claiming what `report` withholds,
  `report` provably writing nothing, `cancel` retiring exactly the set `report`
  named, and already-sent rows left alone.

**Not verified:** nothing has been run against production or staging. The guard
has never met the real 268-row backlog, which is the point of `report`.

## Related

- `.issues/080` — why post-event kinds survive the event ending.
- `~/solana-thailand-devrel-helper/reports/phase-2/BETHERE-HACKATHON-BRIEF.md` —
  where this was raised, and the wider 21-day plan.
