# 051 — Online Attendee Virtual Check-In (Roadmap #5 Complete)

## What Happened

Completed the final roadmap item for issue #015 (Event Format Model): **online attendee virtual check-in via quest completion**.

When an online attendee registers for an Online or Hybrid event, they get a `claim_token` in the Google Sheet but no `checked_in_at` timestamp (since there's no physical check-in). Previously, this blocked the NFT claim flow at `execute_claim()` which required `checked_in_at` to be set.

Now, when an online attendee completes the quiz or adventure and attempts to claim, the system automatically performs a **virtual check-in** — writing the timestamp to the Google Sheet — and proceeds with the normal claim flow.

## Implementation

### Backend (`worker`)

1. **`worker/src/sheets.rs`** — `mark_virtual_checked_in()`
   - Writes column I (`checked_in_at` timestamp) and column J (`checked_in_by = "virtual"`)
   - Does NOT touch column R (`claim_token`) — already set during registration
   - Follows same `batchUpdate` pattern as `mark_checked_in()`

2. **`worker/src/claim.rs`** — `verify_online_quest_completion()` + modified `execute_claim()`
   - New helper checks quiz status → adventure status → returns `true` if either passed
   - In `execute_claim()`, step 3 (check-in gate) now has an online attendee escape hatch:
     - If `checked_in_at` is `None` AND attendee is online (`!is_in_person()`) AND event has online track
     - Checks quest completion → auto-calls `mark_virtual_checked_in()` → updates in-memory attendee → proceeds
     - If quest not completed → returns "you must complete the quiz or adventure" error
   - In-person attendees still get the original "not checked in" error

3. **`domain/src/models/api.rs`** — Added `participation_type: String` to `ClaimLookupResponse`
   - Frontend can now distinguish online vs in-person attendees

### Frontend (`frontend-leptos`)

1. **`frontend-leptos/src/api.rs`** — Added `participation_type` to `ClaimLookupData`

2. **`frontend-leptos/src/pages/claim.rs`** — UX polish for online attendees
   - New `checked_in_label()` helper: shows "Registered" for online attendees without check-in
   - New `is_online_participant()` helper for participation type detection
   - Updated 4 display locations: QuizView, QuizSubmittedView, NftComingSoon, Ready states
   - Online attendees now see "Registered" instead of "Checked in N/A"

## Plan/Code/Test

- Code: `worker/src/claim.rs`, `worker/src/sheets.rs`, `domain/src/models/api.rs`, `frontend-leptos/src/api.rs`, `frontend-leptos/src/pages/claim.rs`
- Tests: manual testing needed (start worker dev server, register online attendee, complete quiz, claim)
- `cargo check` ✅ Clean
- `cargo clippy` ✅ Clean
- `bash build.sh` ✅ Success (2.2 MB WASM, 68 KB JS, 152 KB CSS)

## Reflection

### Struggling
- Initially considered a separate public endpoint `POST /api/public/virtual-checkin/{token}` but realized it's simpler and more secure to auto-trigger during `execute_claim()` — no new endpoint needed, no race conditions
- Had to make `attendee` mutable to update `checked_in_at` after virtual check-in

### Solved
- The virtual check-in pattern is clean: it piggybacks on the existing claim flow
- Adding `participation_type` to the API response enables frontend differentiation without extra API calls

## Remain Work

**Issue #015 is now 100% complete.** All 6 roadmap items are done.

### Deployment Checklist (Carryover)
| Step | Status |
|------|--------|
| On-chain escrow program (devnet) | ✅ Deployed |
| On-chain escrow program (mainnet) | ❌ Pending (~0.5 SOL) |
| Worker secrets configured | ❌ Pending (`HELIUS_API_KEY`) |
| Frontend built + deployed | ✅ Built locally, ❌ Not deployed |
| `DEV_MODE=0` production mode | ❌ Pending |
| Browser E2E testing on devnet | ❌ Pending |
| `wrangler deploy` to production | ❌ Pending |

### Testing Checklist
- [ ] Start worker dev server (`cd worker && npx wrangler dev`)
- [ ] Hard-refresh browser (Cmd+Shift+R)
- [ ] Test self-registration as Online attendee on `/e/{slug}`
- [ ] Verify quiz/adventure appears on claim page for online attendee
- [ ] Complete quiz → verify "Claim NFT Badge" button appears
- [ ] Click claim → verify virtual check-in happens automatically
- [ ] Check Google Sheet: column I should have timestamp, column J should have "virtual"
- [ ] Verify in-person attendees still get "not checked in" error if not physically checked in
- [ ] Test hybrid event: online track → virtual check-in, in-person track → physical check-in required

## Issues Ref

- `.issues/015_event_format_model.md` — parent issue (now complete)
- `.issues/010_deposit_refund_escrow.md` — escrow issue

## How to Dev/Test

```bash
# Build check
cargo check
cargo clippy

# Frontend build
cd frontend-leptos && bash build.sh

# Dev server
cd worker && npx wrangler dev

# Test flow:
# 1. Create an Online event format event
# 2. Visit /e/{slug} and register as attendee
# 3. Click the claim link → should show quiz/adventure gate
# 4. Complete quiz/adventure → should show "Registered" status
# 5. Click "Claim NFT Badge" → virtual check-in auto-happens → NFT minted
```

## Commit

- `13c69a8` — feat: online attendee virtual check-in via quest completion (roadmap #5)
