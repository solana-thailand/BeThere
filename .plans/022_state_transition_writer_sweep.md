# 022 — Sweep every writer of a state transition before trusting a guard

**Status:** every transition identified for this plan has been swept — eight in
total. Five needed a fix; `claimed_at`, `approval_status` and `escrow_status` were
already clean (the last two picked up a writer-set guard anyway). `checked_in_at`
was swept twice: the second pass (§2b) collapsed the copied gate into a single
writer and closed two more defects the copy had left behind.

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

### §2b — the follow-up: one writer instead of two (`e33b2c5`)

`e628759` copied the gate into `quest_complete_checkin`. Step 3 of the sweep says
to prefer collapsing over copying, and the two remaining "still open" items on
this transition were both consequences of the copy, so the two self-serve paths
were collapsed into one writer:

**`worker/src/virtual_checkin.rs` — `commit_virtual_check_in`.** It owns both
event/attendee gates (online track, `can_check_in_virtually`), writes D1
synchronously, then mirrors to Sheets via `wait_until`. `handlers/adventure.rs`
and `claim/mint/execute.rs` call it and nothing else. The staff scan is
deliberately *not* a caller: it writes `checked_in_by = <staff email>` plus a
claim token, a different transition, and runs the same domain gate directly.

Three defects closed by the collapse:

1. **The claim path now runs the approval gate.** It previously did not — it read
   a set `checked_in_at` as proof one had run. That transitive trust is what
   `quest_complete_checkin` broke, and it is now unnecessary: the gate is in the
   writer, so it cannot be reached without it. The guard test that pinned the old
   premise ("`execute.rs` still lacks an approval check") was rewritten rather
   than left asserting a stale assumption, as it asked to be.
2. **The claim path now writes D1.** It set the in-memory struct and the Sheet
   only; `my-registration` reads D1-first, so an online attendee auto-checked-in
   by the claim flow still saw "not checked in" until the next sync.
3. **`quest_complete_checkin` now verifies the adventure.** It checked only
   `config.enabled` while its doc comment promised the required levels were
   verified in D1. It now requires `AdventureStatus::Passed` (progress is keyed by
   claim token, so an unresolvable token denies). Minting was never at risk — the
   claim gate re-verifies — but `checked_in_at` is the attendance signal the
   dashboards, `db/event_summaries.rs` and the plan 008 recap count, so an
   unstarted adventure must not set it.

`worker/tests/virtual_checkin_guard.rs` (5 tests) pins the new shape: the shared
writer gates before it writes; both self-serve paths delegate and call
`check_in_attendee` nowhere themselves; the adventure status is read before the
commit; the staff branch still shares the domain gate. The fifth **walks
`worker/src` and fails on any file outside the three licensed ones that writes the
virtual check-in columns** — it catches a *fourth* writer, not just a regression
of the two known ones. Mutation-tested four ways: the write inlined again in
`execute.rs`, the gate moved after the write in `virtual_checkin.rs`, the
adventure status check deleted, and a new unlicensed writer added. Each red.

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

- ~~**`save_thb_deposit` never writes KV when D1 is configured**~~ — settled.
  Making KV a real mirror would be *wrong*: the function serialises the caller's
  whole in-memory struct, so the mirror would re-create the retraction the CAS
  prevents, in a store with no CAS to lose to. The D1 branch now **deletes** any
  KV copy instead, so a blob left over from a D1-less deployment cannot diverge
  for good. It cannot orphan data: the branch has just written the row to D1,
  where the fallback looks first.

  The exposure was narrower than first recorded — `get_thb_deposit_with_fallback`
  is read only by the ticket page, and a D1 *error* propagates rather than falling
  back, so only a D1 *miss* reaches KV. The payout path (`try_settle_refund`) is
  D1-only, so this was a stale-display risk, not a second double-payout route.
  Pinned by a sixth test in `thb_settlement_ownership_guard.rs` (no `kv.put` on
  the D1 branch, and the delete is still there); mutation-tested both ways.
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

## §5 — `approval_status` (swept, clean, no change)

Writers of `approval_status = 'approved'`:

| writer | entry | value | scope |
|---|---|---|---|
| `upsert_attendee` | `register/signup.rs` | `"approved"` | self-registration is auto-approved by design |
| `upsert_attendee_full` | `events/sync.rs::sync_sheet_to_d1` | normalized Sheets cell | organiser-gated (`auth::check_event_access`) |
| `insert_walkin_attendee` | `walkin::register_walkin` | `'approved'` literal | staff-gated router |
| `upsert_post_event_attendee` | post-event lead form | `'post_event_registered'`, and the `DO UPDATE` deliberately omits the column | cannot promote |
| `EventDurableObject` | DO RPC | — | not reachable; DO bindings disabled |

There is **no** "approve attendee" endpoint: approval is edited in the Google
Sheet and flows one way, Sheets → D1, through the sync. No divergence found, and
every unhandled value fails *closed*:

- `CheckInStatus::from_str` is total and maps anything unrecognised (including
  `""` and `rejected`, which is not a variant) to `PendingApproval`;
  `normalize_approval_status` passes unknown values through verbatim, so D1 can
  hold a status no reader recognises — but every reader either parses it through
  `from_str` or filters `approval_status = 'approved'` literally. An unknown
  status therefore denies, never grants.
- `upsert_attendee`'s `ON CONFLICT (id)` can never fire from `signup.rs` (the
  caller mints a fresh `Uuid::now_v7()`); a repeat registration hits the partial
  unique index and is handled as already-registered. So the auto-approve is
  insert-only and cannot promote a registrant an organiser has demoted.
- `upsert_attendee_full` blanket-overwrites `approval_status` from the sheet while
  COALESCE-preserving `checked_in_at`. That asymmetry is the same shape as §3's
  defect, but here it is the correct direction: the Sheet is the organiser's
  editing surface for approval and the worker never writes approval to D1 outside
  the signup insert. Checked that the registration append writes `"Approved"` into
  the approval column (`sheets/write/append.rs:71`) — without it, every
  self-registration would be demoted to `pending_approval` by the next sync.

**No change made.** Worst case is a *denial*: an organiser clearing or typo-ing
the approval cell demotes the attendee on the next sync, recoverable by fixing
the cell.

## §6 — credit balance (swept, fixed in `9870709`)

Balance is `SUM(delta)` over `(email, organization_id, currency)` in the
append-only `credit_ledger`. Writers:

| writer | scope used |
|---|---|
| `record(+hold)` — `hold_credit.rs` (attendee elects), `hold_admin.rs` (organiser) | `event.organization_id` |
| `record(+return)` — `handlers/checkin.rs` Model B | `event.organization_id` |
| `remove_return` — undo check-in | keyed by `(event_id, email)`, org-independent |
| `try_spend(-apply)` — `signup.rs`, `hold_admin.rs` | `event.organization_id` |
| ~~`record(-refund)`~~ — `clear_credit_refund_request_handler` | **hard-coded `""`** |

**The defect.** Two paths hang off the *contact*, not an event, so they have no
org context: the attendee's balance chip (`credit_balance_handler`) and the
payout reversal that fires when an organiser clears a "return my held credit"
request. Both guessed `organization_id = ""`. The ledger's own migration says
"events carry an empty organization_id today" and the handler carried the comment
"Single-org scope today" — the same *assertion that a sibling path is safe* this
plan warns about. The Events tab has an Org ID column (`sheets/events_tab.rs`
column Q, documented example `solana-thailand`) and `org_store.rs` reads a per-org
config whenever it is non-empty, so the assertion holds only until an organiser
types into that cell.

Once it is set, holds land under the real org and:

- the balance chip reads `("", …)` → **0**, so the attendee is told they hold no
  credit; and
- the reversal reads the same empty bucket → `bal > 0` is false → **no reversal is
  written**, while the organiser has already paid the cash out. The attendee keeps
  the full spendable balance. That is the double payout the reversal exists to
  prevent, reintroduced by a scope mismatch rather than a missing guard.

**The fix.** `credit_ledger::positive_balances(email)` enumerates every
`(organization_id, currency)` bucket the email still holds — one `GROUP BY …
HAVING SUM(delta) > 0` query. The reversal writes one entry per bucket, with the
bucket in the idempotency key (`refund:{email}:{requested_at}:{org}:{currency}`),
so a double-clear finds each bucket already at zero. The balance chip sums the
buckets per currency; what is *spendable* at a given event is still resolved
per-org at registration, which is unchanged.

Guarded by a fourth test in `worker/tests/credit_ledger_guards.rs`: it walks
`worker/src` and fails if **any** caller passes an empty-string org to
`credit_ledger::{balance, record, try_spend}` — so a new contact-scoped path
cannot repeat the guess. Mutation-tested by restoring the hard-coded org at one
call site; red.

### Still open on this transition

- The Sheets `contacts` mirror (`increment_credit`) is inherently org-blind — one
  contacts sheet for all orgs. It is display-only (the D1 ledger is authoritative
  and the module doc says so), but a multi-org deployment will show a merged
  number there.
- ~~`reconcile` (`lib.rs:195`, the scheduled job) was not re-read as part of this
  sweep.~~ **Swept — §6b below.**

## §6b — the payout reversal's failure path, and reconcile's blind side

Re-reading the reversal for the first time end-to-end surfaced that §6 fixed
*which* buckets get reversed but not *whether the reversal happened at all*.

`clear_credit_refund_request_handler` read the buckets with `unwrap_or_default()`
and logged each failed `record` with "reconcile manually" — then **cleared the
flag regardless**. So any transient D1 failure during the reversal produced
exactly the outcome the reversal exists to prevent: the organizer has paid the
cash out, the flag is gone from the queue, and the attendee keeps the full
spendable balance. The "reconcile manually" instruction had nowhere to land —
`reconcile` only checked the *loss* direction (held deposit with no ledger
credit), so credit standing against a cleared request was invisible forever.

Two changes, one for each half:

1. **The reversal fails closed.** The bucket enumeration and the per-bucket write
   moved into `reverse_held_credit(db, email, requested_at)`, which propagates
   with `?`; the handler `map_err(AppError::Internal)?`s it *before* either flag
   clear. A failure now 500s and the request stays in the organizer's queue to
   retry — recoverable, where an unreversed clear is not. The per-bucket
   idempotency key means the retry re-writes only the buckets that did not land.
   A missing D1 binding is treated the same way (fail closed) rather than
   clearing a flag whose ledger side can't be written.
2. **`reconcile` now checks both directions of the money.** Two more COUNT
   queries alongside the existing two:
   - `double_settled` — deposits with `held_as_credit = 1 AND refunded = 1`. The
     two settle paths are mutually exclusive by CAS, so a nonzero count means the
     CAS was bypassed and money left twice (cash back *and* credit kept).
   - `phantom_holds` — ledger `hold` entries whose deposit is no longer held as
     credit. The mirror of `orphan_holds`: credit that exists and should not.

   Both feed `is_clean()` and the Slack alert text, so either surfaces within a
   day instead of never.

Guarded by two more tests in `worker/tests/credit_ledger_guards.rs`
(6 total): one pins the reversal's fail-closed shape (no `unwrap_or_default` in
the helper, `?` on both the read and the writes, reversal ordered before the
clear, `map_err(...)?` between them); one pins all four reconcile fields in both
`ReconcileReport` and `is_clean()`, the `double_settled`/`phantom_holds` SQL
shapes, and a `count_query` per check so a field stubbed to a literal is caught.
Mutation-tested four ways (swallow the bucket read; swallow the handler's error;
drop `double_settled` from `is_clean`; stub `phantom_holds = 0`) — each red, and
the last one is why the `count_query` count assertion exists.

## §7 — `escrow_status` (swept, clean, guard added in the same commit)

The event-level escrow lifecycle already had a behavioural contract test
(`worker/tests/escrow_transition_contract.rs`, plan 014 §2.4) pinning the five
legal transitions, the twenty illegal ones and the exact error format. What it did
**not** pin is the set of writers — and every event write persists a whole
`EventConfig` through `save_event_config` / the D1 `upsert_event`, so a handler
that loads a config, assigns the field and saves would bypass the allowlist while
keeping all thirteen existing tests green.

Swept anyway:

| writer | verdict |
|---|---|
| `event_store::apply_update` | the allowlist itself — the only assignment in `worker/src` |
| `event_store::update_event` | loads → `apply_update` → persists; the two copies were already collapsed |
| escrow-init confirmation (`escrow/status.rs`) | builds an `UpdateEventRequest`, goes through `update_event` |
| `PUT /events/{id}` (`handlers/events/update.rs`) | same; its two `escrow_status` mentions are `==` comparisons for an audit-log entry |
| `write/create.rs`, `write/seed.rs`, `duplicate.rs` | hard-code `EscrowStatus::None` in a struct literal — not a transition |
| Events tab column K (`sheets/events_tab.rs`) | **write-only** from the worker; the only reader is a super-admin listing endpoint, and no path imports the cell back into a config |
| `db/events.rs` upsert | `escrow_status = excluded.escrow_status` — persistence of whatever the caller already validated |

**No divergence found; no behaviour changed.** A third layer was added to the
contract test: it walks `worker/src` and fails on any `.escrow_status =`
assignment outside `event_store/write/update.rs`. Mutation-tested by adding one in
`handlers/events/recap.rs`; red.

### Follow-ups (both since closed)

- ~~Re-initialising an escrow to a *different* address while the event is still
  `Initialized` returns a 500 "failed to persist escrow state".~~ **Fixed.**
  The refusal itself is correct — repointing a live escrow would strand every
  deposit held at the old address, which is exactly why the allowlist makes
  `Initialized → Initialized` illegal. What was wrong is that the refusal
  arrived as an opaque server error. `confirm_escrow_init` now catches the case
  before it reaches `update_event` and returns a `Validation` error naming the
  current address, the derived one, and the wind-down path
  (`Initialized → Deactivated → Closed → None`). The empty-`escrow_address`
  variant of the same state takes the same branch. A fourteenth test in the
  contract file pins the guard's existence, its position before `update_event`,
  its error class and the recovery text; mutation-tested three ways.
- ~~The contract test's module header still describes "two copies in write.rs"
  and the pre-#052 file path.~~ **Fixed** — the header now documents the actual
  three layers (behavioural matrix, single-copy source scan, writer-set walk),
  the post-split path, and the corrected Layer 2 counts (5 arms, 1 error string,
  not 10 and 2).

## Transitions not yet swept

None. Every transition identified at the start of this plan has been swept.
New ones should be added here as they appear — the procedure above is the
deliverable, not the list.

## DoD

- [x] `checked_in_at` swept; divergence fixed and guarded.
- [x] `checked_in_at` re-swept — the two self-serve writers collapsed into
      `virtual_checkin::commit_virtual_check_in`; no "still open" items remain
      on this transition.
- [x] THB `refunded` / `held_as_credit` swept; blanket writer defanged and guarded.
- [x] `claimed_at` swept — clean, both writers already share the claim lock.
- [x] `verified` (USDC deposit) swept — plan 003 §7.
- [x] claim-lock cleanup swept — plan 020 §10.
- [x] `approval_status` swept — clean; Sheets is the only editing surface and
      every unrecognised value fails closed.
- [x] credit balance swept; the two contact-scoped paths no longer guess the org.
- [x] `escrow` state transitions swept — clean; a writer-set guard now backs the
      existing allowlist contract.
- [ ] Nothing here is deployed; this branch is unpushed.
