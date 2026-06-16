# 102 — Signer Cross-Check + On-Chain Recovery for USDC Deposits

## What Happened

Continued from session summary `e68319d3` (BeThere Record Deletion + Deposit Amount + On-Chain Investigation). The user asked whether the worker could (1) check a deposit TX on-chain, (2) cross-check that the signer matches the attendee's wallet, and (3) flip the deposit to verified when web2 had issues but on-chain was already done.

### Discovery: Existing Verification Had No Signer Check

The deposit verification flow already existed, but `verify_tx_on_chain` used the `getSignatureStatuses` RPC method — which only checks **whether** a TX was confirmed, not **who** signed it. So in theory, anyone who knew a valid TX signature could get "verified". The attendee's `wallet_address` was already captured at deposit initiation (`deposit_usdc_handler`) but never compared against the actual on-chain signer.

### (a) — Signer Cross-Check via `verify_tx_with_signer`

Added a hardened verification function that uses `getTransaction` instead of `getSignatureStatuses`, allowing extraction of the actual signer (`message.accountKeys[0]`) and comparison against the expected attendee wallet. Returns a new `VerifyWithSignerOutcome` enum:

- `Confirmed { signer_matched: true/false, signer }` — TX confirmed, with whether the signer matched
- `Pending` — TX not found or not yet confirmed, keep polling
- `RpcError` — transient RPC failure, retry

The deposit is marked verified **only if `is_confirmed_and_matched()` is true** — TX confirmed AND signer matches the recorded wallet. On mismatch, logs a warning and returns `confirmed: false` (attendee stays pending; mismatch visible in logs).

Wired into both verification paths:
- `confirm_deposit_handler` (`GET /api/deposit/usdc/confirm`) — frontend polling path
- `verify_and_confirm_deposit` — background path triggered by the webhook

**Wallet backfill:** if the deposit record had no `wallet_address` (e.g., older record), it's backfilled from the verified signer so future refunds/check-ins have it.

**Removed dead code:** the original `verify_tx_on_chain`, `verify_tx_on_chain_impl`, and `VerifyOutcome` enum (superseded by `verify_tx_with_signer` / `VerifyWithSignerOutcome`).

### (b) — On-Chain Recovery (`discover_deposit_tx_on_chain`)

During live testing against the `islanddao-v4-demo` deposit, discovered that its `tx_signature` was **empty** in D1 — the webhook that normally records the signature was never called (network drop / worker restart / frontend bug). The signer cross-check above couldn't recover this case because it only runs when a signature exists.

Added a recovery path in `confirm_deposit_handler`'s "no TX signature" branch: when the deposit record has a `wallet_address` and the event has an escrow, the worker:
1. Derives the AttendeeDeposit PDA from `[b"deposit", escrow_pda, attendee_pubkey]`
2. Calls `getSignaturesForAddress` on the PDA (limit 5, newest-first)
3. Picks the most recent error-free signature
4. Records it → next poll verifies through the normal signer cross-check path

### (c) — Pure Parsing Functions for Testability

Extracted two pure functions from the async wrappers so the decision logic is unit-testable without mocking the Cloudflare Workers `Fetch`/`Delay` runtime:

- `parse_get_transaction_response(parsed, signature, expected_wallet) -> VerifyWithSignerOutcome`
- `parse_signatures_for_address_response(parsed) -> Option<String>`

### Live Verification Against `islanddao-v4-demo`

Direct RPC queries confirmed the recovery will work:
- AttendeeDeposit PDA `68XnBdwjuwMQWpCyZSUp5TCM8ESbfUuibzdQtaovqLf5` has signature `5Mc7uojXxdSboUSSyM1PMk6PM9k37qVTgfzy65zgmbkGR3qv9Lx7qriZNo3uahoiKBmCHjHucEhmcSrYnVufjpGP`
- `getTransaction` confirms signer = `AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn` (matches recorded wallet)
- TX finalized, `meta.err: null`

The recovery will fire automatically when the organizer (whose wallet = attendee wallet) next views the deposit page — the frontend polls `/api/deposit/usdc/confirm` with their JWT.

---

## Where Is the Plan / Code / Test

### Files Changed (2 files, +1300 / -200 lines)

| File | Change |
|------|--------|
| `worker/src/handlers/deposit/usdc/mod.rs` | New `verify_tx_with_signer` + `VerifyWithSignerOutcome` enum, `parse_get_transaction_response` (pure), `discover_deposit_tx_on_chain` + `parse_signatures_for_address_response` (pure), removed dead `verify_tx_on_chain`/`VerifyOutcome`, 25 unit tests |
| `worker/src/handlers/deposit/usdc/handlers.rs` | `confirm_deposit_handler` uses signer cross-check + on-chain discovery recovery; `verify_and_confirm_deposit` uses single `verify_tx_with_signer` call with backfill |

### Tests

- **25 new unit tests** in `worker/src/handlers/deposit/usdc/mod.rs::tests` — all passing
- Coverage:
  - `VerifyWithSignerOutcome` methods (4 tests): `is_confirmed`, `is_confirmed_and_matched`, `signer()`
  - `parse_get_transaction_response` (10 tests): confirmed/matched, mismatch (security), finalized, no-wallet backfill, null result, missing result, failed TX, processed-not-confirmed, RPC error, malformed/empty accountKeys, case-insensitive match, realistic devnet self-deposit fixture
  - `parse_signatures_for_address_response` (8 tests): newest-first selection, skips failed TXs, all-failed returns None, empty/missing/null result, RPC error, single entry
- Run: `cd worker && cargo test --lib usdc::tests::`
- `cargo clippy -p event-checkin-worker --all-targets` clean

### Deployment

- **Deployed to production** via `worker/deploy.sh` — startup 14ms, frontend assets verified at 71,963 bytes
- Worker health check passes: D1 connected, 6 events, 80 attendees
- wrangler `/versions` API bug (10013) hit as expected, PUT-API fallback succeeded

---

## Reflection — Struggling / Solved

### Struggle: Double-RPC Redundancy in `verify_and_confirm_deposit`

First attempt added the signer check **inside** the existing `if verify_tx_on_chain().is_confirmed()` block — meaning two RPC calls per verification (getSignatureStatuses + getTransaction). Refactored to a single `verify_tx_with_signer` call by loading the deposit status first (needed for the expected wallet) before the RPC call. Same security, half the RPC calls on the success path.

### Struggle: `find_program_address` Returns `Result`, Not Tuple

Initial `discover_deposit_tx_on_chain` called `find_program_address(...).await` with `let (deposit_pda, _) = ...` — assuming it returns a tuple. The function actually returns `Result<(PubkeyBytes, u8), EscrowError>`. Fixed by `match`ing on the `Result` and returning `None` on `Err`. `cargo check` for the worker (wasm target) caught this; the LSP diagnostics (different target) missed it initially.

### Solved: Public Devnet RPC Returns `confirmationStatus: null` for Old TXs

Live testing against the `islanddao-v4-demo` deposit revealed that the public devnet RPC (`api.devnet.solana.com`) returns `meta.confirmationStatus: null` for finalized historical transactions. Without a default, this would cause every old deposit to be treated as "Pending" forever. The parser defaults to `"confirmed"` when `confirmationStatus` is missing — correct because a TX returned by `getTransaction` with `meta.err == null` is by definition landed on-chain. This default is now covered by `test_parse_realistic_devnet_self_deposit_signer_matches`.

### Solved: Empty `tx_signature` in D1 Required Recovery Layer

Original signer cross-check design assumed the `tx_signature` was always recorded. Live testing showed this is false — the `islanddao-v4-demo` deposit had `tx_signature: ""` in D1, meaning the webhook was never called. Added `discover_deposit_tx_on_chain` to recover this case by querying the AttendeeDeposit PDA's signature history on-chain.

---

## Remaining Work

1. **Verify recovery in production** — The recovery logic is deployed but the `islanddao-v4-demo` organizer needs to view the deposit page (which polls `/confirm` with their JWT) for the auto-recovery to fire. The next page load should flip the pending badge to verified.
2. **Rate limiting on discovery** — `discover_deposit_tx_on_chain` adds an extra `getSignaturesForAddress` RPC call when no signature is recorded. A malicious attendee could trigger this on every poll. Consider adding a short-circuit (e.g., only attempt discovery once per N minutes per attendee, tracked in KV).
3. **Multi-deposit edge case** — `getSignaturesForAddress` returns newest-first, so the discovery picks the **most recent** TX touching the AttendeeDeposit PDA. If an attendee closed and re-created their deposit (via refund + new deposit), the discovery would pick the new signature — which is correct. But if there were failed deposit attempts, the parser correctly skips them (only returns error-free signatures).
4. **Frontend "AwaitingConfirmation" UX** — When the recovery records a signature but doesn't immediately verify (returns `confirmed: false` for the next poll to handle), the frontend should ideally show "Verifying..." rather than "Awaiting Deposit". Low priority.
5. **Push to `origin/develop`** — Held pending user decision (standing local-only instruction).
6. **Commit the changes** — See "How to Dev/Test" below for the suggested conventional commit message.

---

## Issues Ref

- **Handover #101** — `verify_tx_on_chain` hardening (timeout + retry). This session's `verify_tx_with_signer` supersedes it but preserves the same timeout/retry pattern.
- **Handover #083 (VULN-004)** — Moved `/deposit/usdc/confirm` from public → `attendee_authed`. This is why the recovery can't be tested via unauthenticated curl — it requires a valid attendee JWT.
- **Session `e68319d3`** — Original investigation of `islanddao-v4-demo` escrow + USDC display fix. Identified that the "Pending Verification" badge was a worker-side issue despite funds being on-chain.
- **Issue #010** — Deposit/Refund Escrow architecture (AttendeeDeposit PDA seeds: `[b"deposit", escrow, attendee]`).

---

## How to Dev / Test

### Build
```bash
cargo check -p event-checkin-worker --quiet
cargo clippy -p event-checkin-worker --all-targets --quiet
```

### Run Tests
```bash
# All 25 new tests (plus existing 87 worker tests)
cd worker && cargo test --lib usdc::tests::

# Specific test
cd worker && cargo test --lib usdc::tests::test_parse_realistic_devnet_self_deposit_signer_matches
```

### Local Dev
```bash
cd worker && ./deploy.sh dev --remote    # Remote KV, local worker
cd frontend-leptos && trunk serve        # Frontend
```

### Deploy
```bash
cd worker && ./deploy.sh
```

### Manual Test — Signer Cross-Check (Happy Path)
1. Register for a USDC-deposit event
2. Complete the deposit via Phantom (on devnet)
3. The webhook records the TX signature
4. Poll `/api/deposit/usdc/confirm` — should return `confirmed: true` after on-chain confirmation
5. Worker logs: `[deposit] confirmed and signer matches expected wallet`

### Manual Test — Signer Mismatch (Security)
1. Submit someone else's TX signature via the webhook
2. Poll `/confirm` — should return `confirmed: false`
3. Worker logs: `[deposit] Signer mismatch — TX confirmed but does not match expected wallet`
4. Deposit stays pending (impersonation blocked)

### Manual Test — On-Chain Recovery (Empty tx_signature)
1. Use an event with an existing on-chain deposit but empty `tx_signature` in D1 (like `islanddao-v4-demo`)
2. Poll `/confirm` as the attendee
3. Worker derives AttendeeDeposit PDA → queries `getSignaturesForAddress` → records discovered signature
4. Next poll verifies the signature via signer cross-check → `confirmed: true`
5. Worker logs: `[deposit] Recovered deposit TX signature via on-chain PDA discovery`

### Verify PDA Derivation (Python)
```python
from solders.pubkey import Pubkey
program_id = Pubkey.from_string('C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T')
escrow = Pubkey.from_string('H8iHzXcz9Sq3sr5Ny5B5VEvCv6bN77NZ6EhmJc163uQ')
attendee = Pubkey.from_string('AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn')
pda, _ = Pubkey.find_program_address([b'deposit', bytes(escrow), bytes(attendee)], program_id)
print(pda)  # Should print 68XnBdwjuwMQWpCyZSUp5TCM8ESbfUuibzdQtaovqLf5
```

### Suggested Commit
```bash
git checkout -b develop/feature/signer_crosscheck_and_recovery
git add worker/src/handlers/deposit/usdc/mod.rs worker/src/handlers/deposit/usdc/handlers.rs
git commit -m "feat(deposit): signer cross-check + on-chain recovery

- Add verify_tx_with_signer using getTransaction (was getSignatureStatuses)
  to extract actual signer (accountKeys[0]) and cross-check against the
  expected attendee wallet — closes impersonation gap
- Add VerifyWithSignerOutcome enum with is_confirmed_and_matched() gate
- Add discover_deposit_tx_on_chain recovery path: when deposit record has
  no tx_signature, derive AttendeeDeposit PDA and query signature history
- Extract parse_get_transaction_response and parse_signatures_for_address_response
  as pure functions for unit testing
- Backfill wallet_address from verified signer when missing
- 25 unit tests covering all decision paths
- Remove dead verify_tx_on_chain / VerifyOutcome (superseded)

Refs: handover #102, session e68319d3"
```
