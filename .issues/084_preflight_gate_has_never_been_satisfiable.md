# 084 — The production preflight gate has never been satisfiable

**Status:** partially fixed 2026-09-13; claim contract fixed in `.issues/088`. A 6/6 green run is still not demonstrated. Blocked on devnet USDC for the harness wallet (Circle faucet captcha needs a human).
**Found:** 2026-09-13, attempting the first prod deploy since the gate landed
**Severity:** high (either prod cannot be deployed, or the gate gets bypassed
routinely and stops meaning anything)

## What

`worker/deploy.sh` refuses a production deploy unless
`flow-harness/results/.last-green` exists and is under an hour old. The sentinel
is written **only** when *every* registered flow passes — `--flow <NAME>`
explicitly calls `without_green_sentinel()` and documents that it "can never
refresh the full-suite production green marker".

All six flows cannot pass, and appear never to have.

**Evidence that it has never been green:**

- `flow-harness/results/.last-green` does not exist and is in no commit.
- `worker/scripts/.preflight-bypass.log` is **empty** — `--force` has never
  been used either.
- The gate landed in `63fd416` on **2026-09-11**. Production was last deployed
  **2026-09-10**. This is the first deploy to meet the gate.

So the gate was introduced, and the next deploy attempt hit a wall.

## Four independent contract drifts

None caused by the release under deploy. Each was reproduced today against
`bethere-staging` with a freshly seeded fixture (`flow-deposit-20260913`), real
devnet USDC, and an escrow initialized on-chain.

### 1. `seed-staging.sh` puts `event_end` on the wrong side of the clock

`seed-staging.sh:62` writes `EVENT_END_MS = NOW_MS + 4h`.
`refund_post_event_end_checked_in` requires `now >= event_end` and its own
failure text says *"The seeded event ends at now-2h at seed time"*.
`refund_pre_event_end` does **not** need a future end — it pins its verdict
clock to `event_end - 1`. So the fixture should seed the end in the **past**,
and the script seeds it in the future.

Moving it to `now-2h` cleared this failure.

### 2. The claim token names do not match, and cannot be overridden

`ClaimFlow`'s default is the literal `flow-test-claim-token`
(`flows/claim.rs:57`). `seed-staging.sh:79` writes
`${EVENT_ID}-claim-token-1`. There is **no `FLOW_HARNESS_CLAIM_TOKEN`** env var
— the value is a code-level config default.

So the claim flow always looked up a token that does not exist. On staging
`PLATFORM_SHEET_ID` is empty, so that D1 miss surfaces as a 500 via the Sheets
fallback — which reads like a server fault and is not one.

### 3. The deposit attendee and the refund attendee are different people

The harness authenticates via SIWS as `FLOW_HARNESS_ATTENDEE_WALLET`, while
`seed-staging.sh` seeds a deposit row for `${EVENT_ID}-attendee-1`. The refund
flows then hit
`validation error: this wallet does not match the deposit on record`.

Correct Worker behaviour; the fixture and the harness simply disagree about who
the attendee is.

### 4. The claim response shape has drifted

With a token that resolves, the flow still fails:
`claim response missing 'status' field`. The Worker does not return `status` on
the claim payload. Harness expectation vs. current API.

## What this release *is* verified against

Recorded so the gate failure is not mistaken for a broken release:

| check | result |
|---|---|
| `deposit` flow (real on-chain USDC deposit) | **passes** |
| `auth` flow (SIWS session) | **passes** |
| claim endpoint resolves a real token | **yes** (failure is #4 above, harness-side) |
| `scripts/verify/claim_token_window_staging.sh` (#071) | **3/3**, both directions |
| #078 error-body redaction on deployed staging | **verified** — returns `internal error` where it previously returned the Sheets URL |
| CI on PR #84 | 6/6 |

## Proposed fix

1. `seed-staging.sh`: seed `event_end` at `now - 2h` (and `event_start` before
   it), matching what the flows document.
2. Add `FLOW_HARNESS_CLAIM_TOKEN`, defaulting to
   `${FLOW_HARNESS_EVENT_ID}-claim-token-1` so the seed script's convention is
   the default rather than a literal from a different era.
3. Make the deposit fixture use the SIWS attendee, or teach the refund flows to
   use the seeded attendee. One identity, not two.
4. Re-check the claim response contract against `flows/claim.rs` and fix
   whichever side is stale.
5. Then run the full suite once and commit the fact that `.last-green` is
   reachable — a gate nobody has ever seen pass is not a gate.

## Interim

Until then every production deploy must use
`deploy.sh --force --reason "..."`, which logs to
`worker/scripts/.preflight-bypass.log`. That is the designed escape hatch, but
if it becomes the normal path the gate is theatre — so this issue should be
fixed, not routed around indefinitely.

## Staging state left behind

`flow-deposit-20260913` on `bethere-db-staging`: escrow initialized on-chain
(`5T9GeoovwRAeNrVieQSUKGTs9UGJLhaopDVXp2QVm7ZB`), `event_end_ms` moved to
`now-2h`, `organizer_wallet` set, and the attendee's `claim_token` changed to
`flow-test-claim-token` while diagnosing #2. A real 10 USDC devnet deposit was
made by the deposit flow. Disposable, but it explains the divergence from what
`seed-staging.sh` would produce.

## Related

- `.issues/072_e2e_script_dead_assertions.md` — same family: assertions that
  could not pass.
- Memory `kv-masks-direct-d1-event-writes` — a D1-only fixture edit was masked
  by staging's KV copy during this investigation; `POST /api/events/reseed-kv`
  cleared it.


---

## Investigation — 2026-09-13

### The root cause is narrower than the four symptoms suggested

Only `DepositFlow::from_env()` and `AuthFlow::from_env()` read the environment.
The three refund flows and the claim flow were registered with `::new()` —
hardcoded `flow-test-event`, `flow-test-attendee-1`, `flow-test-claim-token`.

So `FLOW_HARNESS_EVENT_ID` moved **one** flow. A run against a named fixture
silently exercised **two different events at once**: deposit against the named
one, everything else against a stale `flow-test-event`. Three of the four
"drifts" reported above are downstream of that single fact.

### Fixed

- `fixture_value` moved into `flows/mod.rs` as `pub(crate)`; all five flows now
  have `from_env()`, and `register_default` / `register_named` use it. One run,
  one fixture.
- `DEFAULT_CLAIM_TOKEN` corrected from the never-written literal
  `flow-test-claim-token` to `flow-test-event-claim-token-1`, and
  `ClaimFlow::from_env` derives `${event_id}-claim-token-1` via
  `seeded_claim_token`, matching `seed-staging.sh`. `FLOW_HARNESS_CLAIM_TOKEN`
  overrides it.
- A guard test fails if `register_default` ever constructs a flow with `::new()`
  again. **Negative-controlled** — reverting one call makes it fail.

### Proposed fix #1 above was wrong — do not apply it

It said to seed `event_end = now - 2h`. **That breaks the fixture entirely.**
Tried on staging; escrow initialization failed:

```
{"InstructionError": [1, {"Custom": 13}]}   # EscrowError::EventEndInPast
```

`initialize_event_escrow` rejects an event that has already ended
(`bethere-escrow/src/errors.rs:32`). A fixture **cannot be born past its own
end**, so `seed-staging.sh`'s `now + 4h` is correct and has been restored.

The refund flows still need the wall clock past `event_end`. Both constraints
are satisfiable only by a **two-phase** fixture, now implemented:

```sh
bash worker/scripts/seed-staging.sh --event-id <id>                     # end = now + 4h
#   initialize the escrow on-chain, then deposit
bash worker/scripts/seed-staging.sh --event-id <id> --advance-past-end  # end = now - 2h
```

`--advance-past-end` updates D1 **and resyncs KV**. The resync is load-bearing:
`resolve_event_or_fallback` reads KV first and falls back to D1 only when the
KV read *errors*, so a D1-only update is masked by the cached `EventConfig` and
the Worker keeps serving the old horizon. This was hit for real during the
investigation and reads exactly like the update silently not working.

**Verified end to end on staging** (`flow-084-lifecycle`): seeded, escrow
initialized on-chain (`927NXWLx…`, confirmed by the Worker), advanced, and the
Worker then served an `event_end_ms` with `now >= event_end`.

### Still not green

`deposit` against a *freshly initialized* escrow fails with
`Transaction simulation failed: ... Provided owner is not allowed`.

**Root-caused 2026-09-13: this is `.issues/085`, not a vault problem.** The
event's `on_chain_event_id` is a u64 above `i64::MAX`, so SQLite stored it as a
REAL and lost the low digits. Every tx builder derives the escrow PDA from that
id, so the deposit addresses a PDA that was never created — hence `IllegalOwner`
after 195 compute units, at account validation.

It is not "specific to a newly initialized vault": it depends only on whether
that event's id happened to land above `i64::MAX`, which is roughly a coin
flip. `flow-test-event` passes because its id is `5055890856068877793`, below
the boundary.

So the gate cannot go green until `.issues/085` is fixed and the fixture's id
is repaired. Until then production deploys still need `--force --reason`.

### Noticed, not fixed

`worker/scripts/.preflight-bypass.log` is matched by `*.log` in `.gitignore`,
so the audit trail for production gate bypasses exists **only on the machine
that ran the deploy**. An audit log nobody else can read is weak evidence —
worth committing it or shipping entries somewhere durable.

`flow-harness` is not a workspace member, so CI's `cargo fmt --check` does not
cover it; six files carry pre-existing drift, reverted out of this change to
keep the diff honest. Same blind spot as `frontend-leptos` (`.issues/083`).


---

## After the `.issues/085` fix — 2026-09-13

With the exact-id fix deployed and a clean two-phase fixture
(`flow-085-verify`), the suite went from **1/6 to 3/6**:

```
✓ deposit                              (a real 10 USDC on-chain deposit)
✓ refund_pre_event_end
✗ refund_post_event_end_checked_in     RefundNotYetAllowed(1) from the program
✗ refund_no_show_deadline              expected RefundDeadlinePassed(19), got RefundNotYetAllowed
✓ auth
✗ claim                                claim response missing `status` field
```

`deposit` passing is the headline: that was the `IllegalOwner` failure this
whole issue started from, and it was `.issues/085` all along.

### The remaining refund failures are an on-chain horizon problem

`--advance-past-end` moves `event_end` in **D1 and KV**. It cannot move it
**on-chain**: `EventEscrow` stores its own `event_end`, written at
`initialize_event_escrow` and immutable thereafter. The refund instruction
checks *that* value, not the Worker's — correctly, since on-chain state is the
authority.

So the fixture ends up internally inconsistent: the Worker believes the event is
over, the escrow does not, and the program returns `RefundNotYetAllowed(1)`.

**The two constraints are only satisfiable by making the window short.**
`EventEndInPast` forces `event_end` into the future at init; the refund flows
need the real clock past it. There is no way to advance the chain, so the seed
has to pick an `event_end` the wall clock will cross on its own:

```
event_end = now + ~2 minutes     # future enough to initialize, brief enough to wait out
```

Then: seed → initialize escrow → deposit → **wait out the window** → run the
suite. `--advance-past-end` remains useful for D1/KV-only flows but must not be
relied on for anything the program validates.

This was not visible before, because the deposit never got far enough to expose
it.

### Not attempted

The rework above needs another on-chain deposit, and the harness attendee wallet
is back to **0 USDC** (the passing deposit spent the last 10). Blocked on the
Circle faucet captcha again.

`claim` is unrelated to any of this — the Worker does not return a `status`
field on the claim payload. Harness expectation vs. current API; one of the two
is stale and it needs a decision, not a fixture.


---

## Status check — 2026-09-17

### The interim became the normal path

`worker/scripts/.preflight-bypass.log` holds **29** production deploys from
2026-09-13 to 2026-09-17 (`c94f17d` to `4c5e8f2`). Every one used `--force`, and
`flow-harness/results/.last-green` has never existed. This is the "gate is
theatre" outcome the Interim section warned about. The later entries (22–24, 28, 29) no
longer even cite this issue.

### What is left

| item | state |
|---|---|
| fixture/flow wiring (one run, one fixture) | fixed 2026-09-13 |
| `.issues/085` exact on-chain event id | fixed, deployed |
| claim flow `status` field | fixed in `.issues/088`; `a_payload_without_status_is_accepted` guards it |
| refund flows vs on-chain `event_end` | **open**: needs a short-window fixture (`event_end = now + ~2 min`), then wait it out |
| harness wallet devnet USDC | **blocked**: 0 USDC, and the Circle faucet needs a human to solve its captcha |
| bypass audit log only on one laptop | **awaiting owner decision**: tracking it (a `.gitignore` negation) was proposed 2026-09-17; it would publish deploy reasons and local backup paths to the remote |

### Next step once USDC is available

1. Fund the harness attendee wallet with devnet USDC (at least 10).
2. Seed with a ~2 minute `event_end`, initialize the escrow, and deposit.
3. Wait past `event_end`, then run the full suite so it writes `.last-green`.
4. Record the run here, then deploy the next release **without** `--force`.

