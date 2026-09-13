# 084 — The production preflight gate has never been satisfiable

**Status:** open — blocks every production deploy
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
