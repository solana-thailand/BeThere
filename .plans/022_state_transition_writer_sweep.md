# 022 — Sweep every writer of a state transition before trusting a guard

**Status:** in progress. Five transitions swept (four needed a fix, `claimed_at` was already clean); the rest listed below are unswept.

## Why this plan exists

Three separate defects on this branch shared one shape:

1. **claim-lock cleanup ordering** — the `?`-before-KV-write bug was fixed on one
   cleanup path; two sibling paths performing the same cleanup kept it (plan 020 §10).
2. **USDC deposit verification** — three entry points could set `verified = true`;
   only the read path carried the F1 and `binding_conflict` guards (plan 003 §7).
3. **Virtual check-in approval** — three paths set `checked_in_at`; the adventure
   quest-complete endpoint never gated on `approval_status` (this plan, §2).
4. **THB settlement** — a CAS owns `refunded` / `held_as_credit`, but a blanket
   read-modify-write reachable from seven handlers could retract its result
   (this plan, §3).

In the first three, a guard was added at the entry point where the bug was *found*, and
the siblings performing the same transition were never revisited. In two of the
three, a plan or a code comment asserted the sibling path was "already safe" —
both assertions were false. **Treat such a note as a lead, not a fact.**

## The sweep

For a state transition `T` (a field flipping to a meaningful value):

1. `rg` for every write of the field — SQL `SET`, struct-field assignment, insert
   column lists. Include the Durable Object copies and the Sheets writers.
2. For each writer, walk up to the HTTP handler(s) that reach it and tabulate the
   gates each one runs. Divergence between columns is the defect.
3. **Prefer collapsing over copying.** If one path is a superset, make the others
   delegate to it and delete the duplicate. A guard that is copied is a guard that
   will diverge again.
4. Express the shared rule in `domain/` where the check is on entity state, so it
   has one home and a behavioural test.
5. Narrow the re-exports afterwards. If the collapsed-away helpers become private,
   restoring the pre-fix path no longer compiles — stronger than any test.
6. Add a source-text guard in `tests/` (strip comment lines, so prose cannot
   satisfy a rule about code) and mutation-test it: revert the fix, confirm red.

## §2 — `checked_in_at` (swept, fixed in `e628759`)

Writers of the transition:

| path | entry | gates |
|---|---|---|
| staff scan, on-site | `handlers/checkin.rs` → `db::attendees::check_in_attendee` | `Attendee::can_check_in` (not-checked-in, approved, in-person) |
| staff scan, `online=true` | same handler, online branch | ~~open-coded not-checked-in + approved~~ → `can_check_in_virtually` |
| self-serve quest completion | `handlers/adventure.rs::quest_complete_checkin` | event has online track, adventure enabled, idempotent — **no approval check** |
| claim mint auto check-in | `claim/mint/execute.rs:145` | online attendee, event has online track, quest passed, event has ended |
| walk-in creation | `db/attendees/walkin.rs` | inserts already-checked-in; approval does not apply by design |
| `EventDurableObject::handle_check_in` | DO RPC | not reachable — DO bindings are disabled (see CLAUDE.md `10021`) |

**The defect.** `claim/mint/execute.rs` never checks `approval_status`. It does not
have to, *provided* every path that sets `checked_in_at` is approval-gated — it
reads a set `checked_in_at` as proof that an approval-gated path produced it. That
is the same transitive trust the deposit paths had. `quest_complete_checkin` broke
it: a `PendingApproval` or `Invited` registrant on an adventure-enabled hybrid
event could `POST /api/adventure/quest-complete` to set their own `checked_in_at`,
and thereby become claim-eligible without an organiser ever approving them. They
still had to complete the adventure — the gate at `execute.rs:252` runs
unconditionally, outside the `checked_in_at.is_none()` branch, so the endpoint's
*other* missing check (it verifies no quest progress despite its doc comment
saying it does) is genuinely defended downstream, as its `SECURITY NOTE` claims.
The approval gate was not.

Secondary effect: an unapproved registrant counts as checked in everywhere
attendance is read — dashboards, `db/event_summaries.rs`, the plan 008 recap.

**The fix.** `Attendee::can_check_in_virtually` in `domain/` — `can_check_in`
minus the in-person rule, which the virtual path inverts. Both the online branch
of the staff scan and `quest_complete_checkin` now call it; the open-coded copy in
`checkin.rs` is gone. Behaviour pinned by `domain/tests/virtual_checkin_gate.rs`
(5 tests, including one that asserts the *relationship* — anything `can_check_in`
accepts, `can_check_in_virtually` must accept, so a rule added to one forces a
decision about the other). Shape pinned by `worker/tests/virtual_checkin_guard.rs`
(4 tests), mutation-tested four ways: gate deleted, gate moved after the write,
`checkin.rs` restored to the hand-rolled copy, and the domain gate stripped of its
approval check. Each turned it red.

The fourth guard test asserts `claim/mint/execute.rs` still *lacks* an approval
check — the premise the other three protect. If that changes, the guards should be
re-derived deliberately rather than left asserting a stale assumption.

### Still open on this transition

- `quest_complete_checkin`'s doc comment claims it "verifies … the required levels
  are completed in D1". It does not — it fetches the config and checks only
  `enabled`. Harmless today (the claim gate catches it) but the comment is wrong
  and the endpoint hands out a `checked_in_at` for an unstarted adventure, which
  inflates attendance. Fixing it means calling `get_adventure_status` here too;
  left out of `e628759` to keep that commit to the security defect.
- `claim/mint/execute.rs`'s auto check-in writes the in-memory struct and Sheets
  but **not D1**, unlike the other two writers, which write D1 first because
  my-registration reads D1-first. Suspected dual-write gap; not yet traced.

## §3 — THB `refunded` / `held_as_credit` (swept, fixed in `186d057`)

`refund_status` on an attendee is *derived* (`db/attendees/reads.rs`) — the real
transition is the `thb_deposits` settlement pair: `refunded` (cash paid out) and
`held_as_credit` (converted to rolling credit), mutually exclusive.

Writers of the transition:

| writer | shape |
|---|---|
| `try_settle_refund` | CAS — `SET refunded = 1 ... WHERE verified = 1 AND refunded = 0 AND held_as_credit = 0` |
| `try_settle_hold_credit` | CAS — mirror image; whichever lands first wins |
| `insert_thb_deposit` | fresh row, nothing to retract |
| ~~`update_thb_deposit`~~ | **blanket read-modify-write of all 14 columns, including all five settlement columns** |

`update_thb_deposit` is reached from `event_store::save_thb_deposit` by seven
handlers. Including the settlement columns in that UPDATE reopened exactly the
hole the CAS closes — not by racing the CAS, but by *retracting* its result:

- `slip_verify` loads the deposit, guards on `refunded` / `held_as_credit` /
  `is_non_cash`, then blanket-writes. A refund settling between the load and the
  write is reset to `refunded = 0`, with `refund_proof_url` blanked.
- `slip_upload` / `slip_admin_upload` construct a fresh record with
  `refunded: false`. Their "already deposited" guard consults the *other* store
  (`deposit_status`), so a `thb_deposits` row surviving without its
  `deposit_status` sibling is overwritten rather than rejected.
- `hold_admin`'s rolling-credit auto-apply builds a `covered` record with
  `refunded: false, held_as_credit: false` and saves it through the same
  existence branch.

In each case the cash has already gone out, the row now says otherwise, and
`try_settle_refund` will settle it a **second time** — its three preconditions
are all satisfied again.

**The fix.** The five settlement columns (`refunded`, `refunded_at`,
`refund_proof_url`, `held_as_credit`, `held_as_credit_at`) are gone from
`update_thb_deposit`'s `SET` list. Every caller that sets one in memory
(`refund.rs` ×2, `hold_credit.rs`, `hold_admin.rs`) does so *after* its own CAS
has already written D1 — their comments say as much — so this is behaviour-
preserving on the intended paths and removes only the clobber. The KV fallback
in `save_thb_deposit` still serialises the whole struct, which is correct: there
is no CAS there.

One legitimate non-CAS writer remained: the `data:`-URL → R2 migration in
`thb/handlers/mod.rs` rewrites `refund_proof_url` in place to replace a
multi-MB inline base64 blob with a compact serving path for the *same* proof. It
now uses `set_refund_proof_url`, which touches exactly that one column and so
cannot retract a settlement.

Guarded by `worker/tests/thb_settlement_ownership_guard.rs` (5 tests), including
one that counts every `SET <settlement column> =` in the file and fails if any
appears outside the three licensed functions — so a *fourth* writer is caught,
not just a regression of this one. Mutation-tested three ways (column restored
to the blanket update, refund CAS stripped of `held_as_credit = 0`, proof setter
given a second column); each turned it red.

### Still open on this transition

- **`save_thb_deposit` never writes KV when D1 is configured** — it returns after
  the D1 branch. Four call sites carry a comment saying they "mirror the settled
  state into KV", which in production does not happen. The comments are corrected
  in `186d057`; whether KV should be a real mirror (it is the documented fallback
  when D1 is unavailable) is a separate design question. If a D1 read ever falls
  back to KV, a stale KV blob would report a settled deposit as unrefunded.
- The USDC deposit refund path was not part of this sweep; `refunded` there lives
  on-chain (`AttendeeDeposit.refunded`, `solana_escrow/wire.rs:344`) and is read,
  not written, by the worker.

## §4 — `claimed_at` (swept, clean, no change)

Two live writers: `claim/mint/execute.rs:411` (the gated mint) and
`claim/mint/walkin.rs:107` (the walk-in claim, which legitimately skips the
quiz/adventure gates — a walk-in never registered online). The
`EventDurableObject` copy (`event_do/checkin.rs:113`) is not reachable; DO
bindings are disabled.

Both live paths are structurally parallel: check `claimed_at.is_some()` first,
`acquire_claim_lock`, mint, `finalize_claim_lock` on success,
`release_claim_lock` on failure. No divergence — this transition was already
brought into line by the claim-lock work (plan 020 §10). **No change made.**

## Transitions not yet swept
- `approval_status` itself — who may set it to `Approved`.
- credit-balance mutations (hold → balance → auto-apply; see
  `credit_ledger_guards.rs` for what is already pinned).
- `escrow` state transitions (`escrow_transition_contract.rs` covers the wire
  shape, not the set of writers).

## DoD

- [x] `checked_in_at` swept; divergence fixed and guarded.
- [x] THB `refunded` / `held_as_credit` swept; blanket writer defanged and guarded.
- [x] `claimed_at` swept — clean, both writers already share the claim lock.
- [x] `verified` (USDC deposit) swept — plan 003 §7.
- [x] claim-lock cleanup swept — plan 020 §10.
- [ ] The three transitions still listed above swept.
- [ ] Nothing here is deployed; this branch is unpushed.
