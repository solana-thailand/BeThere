# 101 — Deposit Confirmation Hardening (c) + THB Card Gate (x)

## What Happened

Implemented action items (c) and (x) from the session summary `cedecc6b`:

### (c) — Harden `verify_tx_on_chain` + Document Devnet Demo Checklist

**Problem:** The USDC deposit confirmation endpoint (`/deposit/usdc/confirm`) could return a 500 (HTML error page) when the RPC subrequest hung or timed out. The frontend couldn't parse the HTML, surfacing "Failed to check deposit confirmation" (status 0). The root cause: `worker::Fetch::send()` had no timeout, so a hanging RPC could exhaust the Worker's 30s wall-clock limit, causing Cloudflare to kill the worker and return an HTML error page.

**Fix — three layers:**

1. **`VerifyOutcome` enum** replaces the boolean return of `verify_tx_on_chain`:
   - `Confirmed` — TX found and confirmed/finalized
   - `Pending` — TX not found yet, or found but not confirmed (normal, keep polling)
   - `RpcError` — RPC infrastructure failure (timeout, network, parse); transient, keep polling

2. **8-second fetch timeout** via `futures_util::select!` racing `worker::Fetch::send()` against `worker::Delay`. Uses `worker::Delay` (worker 0.8.1) which has `PinnedDrop` — the `setTimeout` is automatically cancelled when the fetch branch wins, so no timer is leaked. Both futures are `.fuse()`d and `pin_mut!`'d on the stack.

3. **Single retry** on transient `RpcError` — after a 500ms backoff (`worker::Delay`), the verification is attempted once more. This helps with transient Helius devnet blips.

**Handler change:** `confirm_deposit_handler` now calls `outcome.is_confirmed()`. When the outcome is `RpcError`, it logs at `warn` level and returns `confirmed: false` (pending) — the frontend keeps polling instead of seeing a 500. Same treatment for `verify_and_confirm_deposit` (background task).

**Demo documentation:** Added a prominent **"⚠️ CRITICAL: Phantom Devnet Pre-Flight Checklist"** section to `DEMO.md` explaining the cluster-mismatch issue (Phantom on mainnet → signature not found on devnet → deposit never confirms) and the 5-step checklist to switch Phantom to Testnet Mode before the demo.

### (x) — Gate THB Card on `thb_amount > 0`

**Problem:** The THB payment card was rendered unconditionally ("always shown, recommended") in `choose_payment.rs`. When an admin set a USDC-only event (THB amount = 0), the THB card still appeared showing "฿0 THB" and was clickable.

**Fix:**
- Added `show_thb = thb_amount > 0` flag
- THB card is now conditionally rendered only when `show_thb` is true
- Grid layout: `single_card = show_usdc != show_thb` — single-column layout when exactly one method is available (either USDC-only or THB-only)
- Degenerate case: `no_methods = !show_usdc && !show_thb` — when admin sets neither amount, shows a clear "No payment methods are configured" error message instead of an empty grid

---

## Where Is the Plan / Code / Test

### Files Changed (4 files, +220 / -59 lines)

| File | Change |
|------|--------|
| `worker/src/handlers/deposit/usdc/mod.rs` | `VerifyOutcome` enum, `verify_tx_on_chain` rewrite with timeout + retry, `verify_and_confirm_deposit` updated |
| `worker/src/handlers/deposit/usdc/handlers.rs` | `confirm_deposit_handler` uses `VerifyOutcome`, logs `RpcError` as pending |
| `frontend-leptos/src/pages/deposit/choose_payment.rs` | THB card gated on `show_thb`, single-card grid layout, no-methods error message |
| `DEMO.md` | Phantom devnet pre-flight checklist (35 lines added at top) |

### Tests
- No new tests added (the changes are in Cloudflare Worker wasm code which can't easily be unit-tested for the `select!` timeout behavior)
- All existing tests pass: 87 + 15 + 21 (worker) + 73 (domain) = **196 tests passing**
- `cargo clippy` clean on worker; frontend has only pre-existing warnings (not from our changes)

---

## Reflection — Struggling / Solved

### Struggle: `futures_util::future::select` temporary lifetime issue (Rust 2024 edition)

**The problem:** Using `futures_util::future::select(Box::pin(fetch_fut), Box::pin(timeout_fut)).await` produced "temporary value dropped while borrowed" errors. Multiple attempts failed:
1. `select(...).await` as a match scrutinee → temporary dropped before match arms
2. Binding `.await` result to `let race_result =` → still failed
3. Splitting `Fetch::Request(request)` into a named binding → still failed

**Root cause discovered:** `worker::Fetch::send(&self)` takes `&self` (NOT `self` by value). The returned future borrows from the `Fetch` value. When using `Box::pin(fetch.send())`, the `Fetch` temporary is dropped before the boxed future is used.

**Solution:** Used `futures_util::select!` macro instead of `select` function:
```rust
let fetch = worker::Fetch::Request(request);
let fetch_fut = futures_util::FutureExt::fuse(fetch.send());
let timeout = futures_util::FutureExt::fuse(worker::Delay::from(...));
futures_util::pin_mut!(fetch_fut);
futures_util::pin_mut!(timeout);

let mut response = futures_util::select! {
    result = fetch_fut => match result { ... },
    _ = timeout => { ... }
};
```

The `select!` macro handles pinning internally. `.fuse()` satisfies the `FusedFuture` bound. `pin_mut!` stack-pins the futures. `worker::Delay` has `PinnedDrop` which cancels the timer when the fetch wins.

### Struggle: `js_sys::Promise::new` signature

Initial `delay_ms` helper used `Promise::new(|resolve, _| { ... })` but `Promise::new` takes `&mut dyn FnMut(Function, Function)`, not a direct closure. **Solved** by switching to `worker::Delay` which handles all the JS interop internally — eliminating the need for the custom `delay_ms` function entirely.

---

## Remaining Work

1. **Deploy** — These changes are NOT yet deployed. Run `./deploy.sh` to push to production.
2. **Verify in production** — After deploy, test the USDC deposit flow with Phantom on devnet to confirm the confirmation polling works. Check worker logs for any `RpcError` warnings.
3. **Issue #1 root cause (cluster mismatch)** — The hardening makes the endpoint resilient, but the core issue (Phantom on mainnet while escrow is on devnet) is still unresolved. The deposit will never confirm if networks don't match. A future fix could add cluster detection or a user-facing warning.
4. **Push to `origin/develop`** — Still held pending user decision (standing local-only instruction). 25+ commits now local-only.
5. **Demo Video + Pitch Video** — Still need to be recorded (blocking hackathon submission).
6. **Lower priority:** Remove quiz-progress dual-write (`quiz.rs:293`), finish issue #053, remove EVENTS KV binding entirely.

---

## Issues Ref

- **Issue #035** — "confirm endpoint returned False even when the TX was confirmed" (pre-existing, this hardening makes it more resilient but doesn't fully solve the cluster mismatch)
- **Issue #053** — KV to D1 remaining migration (lower priority)
- **Handover #100** — KV write elimination (deployed in prior session, these changes are on top)
- **Session `cedecc6b`** — Original diagnosis of both bugs

---

## How to Dev / Test

### Build
```bash
# Worker (includes domain)
cargo check -p event-checkin-worker --quiet
cargo clippy -p event-checkin-worker --quiet

# Frontend (separate cargo project)
cd frontend-leptos && cargo check --quiet && cargo clippy --quiet
```

### Run Tests
```bash
cargo test -p event-checkin-worker --quiet    # 123 tests
cargo test -p event-checkin-domain --quiet     # 73 tests
```

### Local Dev
```bash
cd worker && npx wrangler dev --port 8787
cd frontend-leptos && trunk serve
```

### Deploy
```bash
./deploy.sh
```

### Manual Test — USDC Deposit Flow
1. Ensure Phantom is on **devnet** (Settings → Developer Settings → Testnet Mode)
2. Register for an event with USDC deposit
3. Complete the USDC deposit via Phantom
4. Verify the confirmation polling succeeds (page reaches "Deposit Confirmed" state)
5. Check worker logs: `wrangler tail` — should see `[deposit] confirmed on-chain: <sig>`

### Manual Test — THB Card Gate
1. Create an event with `deposit_amount_usdc = 5`, `deposit_amount_thb = 0`
2. Register as attendee → go to deposit page
3. Verify: only the USDC card is shown (THB card hidden)
4. Create another event with `deposit_amount_usdc = 5`, `deposit_amount_thb = 100`
5. Verify: both cards shown in 2-column grid
6. Create event with `deposit_amount_usdc = 0`, `deposit_amount_thb = 100`
7. Verify: only THB card shown in single-column grid
```
[MODIFY]: .handovers/101_deposit_confirmation_harden_and_thb_card_gate.md (added)]