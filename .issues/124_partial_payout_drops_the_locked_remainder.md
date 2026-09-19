# 124 — A partial credit payout clears the request and drops the locked remainder

**Status:** Open — found while building #120 §3, not fixed
**Found:** 2026-09-19, self-review of the #120 diff
**Severity:** Low today (no open requests in prod; ฿3,500 is locked across 7
people until RTM #6 ends 2026-09-27 16:00 +07), Medium once it happens

## The case

#120 §3 closed the case where a payout request has **nothing** payable and
everything locked: the clear now returns 409 and the request stays in the queue.

The **partial** case is still open. A holder with ฿800 who registers for an
event with a ฿500 deposit has ฿300 payable and ฿500 locked. If they request a
return and the organizer clears it:

1. `reverse_held_credit` reverses the ฿300 — correct, that is what was paid out;
2. the flag clears;
3. when the event ends, `release_ended_applies` returns the ฿500 to the ledger —
   with no open request, and no record that one was ever made.

The attendee has to notice and ask again. Nothing is lost or double-paid, but
the request silently covers less than it asked for.

## Why it was not fixed in the same pass

The obvious fix — reverse what is payable but keep the flag set while anything
is locked — collides with the reversal's idempotency key:

    refund:{email}:{requested_at}:{organization_id}:{currency}

`requested_at` only changes when the attendee re-requests, so a second payout
under the *same* open request reuses the key and the `record` insert no-ops.
The ฿500 would never leave the ledger: the organizer pays the cash and the
attendee keeps spendable credit — the double payout the whole fail-closed
design of that path exists to prevent.

Making the key per-payout (adding the amount, or a sequence) is a ledger change
with its own trap: two payouts of the same amount under one request would
collide again, in the direction that costs the organizer. That is a design
decision about the payout path, not a display fix, so it does not belong in a
pass whose scope was "make the exit reachable".

## Options

1. **Sequence the reversal key** — `refund:{email}:{requested_at}:{seq}:{org}:{currency}`,
   `seq` from `COUNT(*)` of existing refund rows for that request. Needs a
   single-statement insert to stay race-free (same rule as `try_spend`).
2. **Re-stamp on partial clear** — treat a partial payout as closing the old
   request and opening a new one (`credit_refund_requested_at = now`). Cheap,
   keys stay unique, the queue row stays visible, and "requested at" honestly
   becomes "waiting since".
3. **Refuse any clear while credit is locked** — simplest and safest, but it
   blocks a legitimate partial payout for up to a whole event cycle.

Option 2 looks right: it needs no ledger change and no new column, and it makes
the queue row's meaning ("this person is still owed something") exact.

## Related

- `.issues/120_credit_withdrawal_path_hard_to_reach.md` — the ฿0 case, fixed.
- `.issues/118_no_show_forfeits_rolling_credit.md` — why locked credit always
  comes back rather than being forfeited.
