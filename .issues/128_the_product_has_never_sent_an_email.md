# 128 — The product has never sent an email, and turning it on would send 243 of them

**Status:** open. **Do not wire the caller or flip the flag until §3 is decided.**
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

**The absence of a caller is currently the only thing preventing that.** Wiring
it *before* deciding what to do with the backlog removes the last safety catch,
so the order matters:

1. decide the backlog (cancel the stale surveys, or defer them, or scope the
   first enable to RTM#6 kinds only);
2. onboard the sender domain and set `NOTIFICATION_FROM`;
3. wire the caller;
4. enable, on staging first, and watch the first run.

`reminder` timing is already safe: `claim.sql` gates it on
`event_start_ms <= now + 86400`, so RTM#6's 24 reminders cannot fire before the
26th regardless.

## 4. Why it is worth doing anyway

A booking product that never confirms a booking is the most visible gap in it,
and the fix is dateable inside the Colosseum window (14 Sep – 12 Oct). Twenty-four
people registered for Sunday have had no confirmation, no deposit receipt and
will get no reminder.

## Related

- `.issues/080` — why post-event kinds survive the event ending.
- `~/solana-thailand-devrel-helper/reports/phase-2/BETHERE-HACKATHON-BRIEF.md` —
  where this was raised, and the wider 21-day plan.
