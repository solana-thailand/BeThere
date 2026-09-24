# 141 — The 6-flow preflight suite contradicts itself on one fixture

**Status:** open, verdict only (2026-09-23). No harness change yet. This supersedes
the "short-window fixture" next step in [084](084_preflight_gate_has_never_been_satisfiable.md):
that step cannot produce a 6/6 green run on its own.
**Severity:** high for the deploy gate's meaning (every prod deploy uses `--force`),
none for users.

## Verdict

`flows::register_default` runs all three refund flows in one process, against one
attendee (`FLOW_HARNESS_ATTENDEE_ID`), one wallet, and one on-chain
`AttendeeDeposit`. Each one submits a **live** refund at the wall clock:

| flow | live refund must | needs on-chain |
|---|---|---|
| `refund_pre_event_end` (`refund_pre_event_end.rs:324`) | revert `RefundNotYetAllowed(1)` | `now < event_end` |
| `refund_post_event_end_checked_in` (`:339-341`) | **succeed** | `now >= event_end`, `checked_in = true` |
| `refund_no_show_deadline` (`:480`) | revert `RefundDeadlinePassed(19)` | `now >= refund_deadline`, `checked_in = false`, not yet refunded |

The program rules are from `bethere-escrow/src/instructions/refund.rs:42,74,81-82`:
- `!refunded`, checked first, fails with `AlreadyRefunded`.
- `now < event_end` fails with code 1.
- `!checked_in && now >= refund_deadline` fails with code 19.

Three conflicts, each enough on its own to keep the suite red:
1. Rows 1 and 2 need opposite sides of `event_end` at the same moment.
2. Rows 2 and 3 need opposite `checked_in` values on the same deposit.
3. Row 2 refunds the deposit, so row 3 then gets `AlreadyRefunded`, not 19.

There is also a timing floor. `refund_deadline = event_end + refund_deadline_hours * 3600`
(`worker/src/handlers/deposit/escrow/handlers.rs:100`). The hours value is an integer,
and 0 falls back to 7 days, so the no-show path waits at least **1 h** past
`event_end`.

The verdict clocks the flows pin (`assertion_now_ms`, `refund_deadline_ms ± 1`)
only cover the pure gate assertions. The chain uses its own `Clock`.

## Not the cause

- Funding. The harness attendee has 9.99998 devnet USDC (re-checked 2026-09-22, see 084).
- `seed-staging.sh`'s `now + 4h` end. Shortening it fixes conflict 1 only after
  the fact, and nothing in conflicts 2 and 3.

## Proposed shape (for the fix, not done)

Two phases against two fixtures. Each phase is green on its own, and the sentinel
is written only when both have passed within the hour:

- **Phase A: live, pre-end.** Fixture with a future `event_end`. Flows: `deposit`,
  `refund_pre_event_end`, `auth`, `claim`.
- **Phase B: matured.** Fixture prepared at least 1 h ahead with `refund_deadline_hours = 1`
  and **two** deposits from two harness wallets: attendee 1 checked in on-chain,
  attendee 2 a no-show. Flows: `refund_post_event_end_checked_in` (attendee 1) and
  `refund_no_show_deadline` (attendee 2). This phase uses up its fixture, so each
  gate run needs a fresh matured fixture: two deposits and 1 h of lead time.

This needs:
- a second funded devnet wallet;
- per-flow attendee config (`FLOW_HARNESS_NOSHOW_ATTENDEE_ID` / wallet);
- a seed mode that writes two attendees.

Seeding does not need a person. On 2026-09-23 `scripts/e2e_devnet_test.sh` ran
green end to end on staging with no browser step. It initialized its own escrow
through `POST /api/escrow/init` → organizer-keypair signature →
`POST /api/escrow/confirm-init`, and deposited, checked in and refunded through
the same API ([074](074_attendee_deposit_decode_offsets_stale.md) "Full run").
The "initialize from Manage Events" step in `flow-harness/README.md` is how it
is done today, not a requirement. Both phases can seed their own fixtures that way.

Whether a prod deploy should wait an hour for Phase B is an owner call. The
alternative is to gate on Phase A only and run Phase B nightly.

## Related fix in this change

`scripts/e2e_devnet_test.sh` defaulted `WORKER_URL` to the **production** worker,
and its step 2 writes an event into the target's KV. It now defaults to staging
(same as `flow-harness/src/context.rs:71`) and refuses the production URL
unless `E2E_ALLOW_PROD=1`. Verified: `WORKER_URL=<prod>/ … --cleanup` exits 2
before touching anything.
