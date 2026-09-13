# Goal: Close the deposit double-registration vulnerability

The read-path self-heal shipped in commits `9d23f58` + `ff7e4b0` introduced a
double-registration vector: `recover_and_verify_deposit` trusts the
`(wallet_address, tx_signature)` pair without verifying that pair isn't already
bound to a _different_ `attendee_id`. An organizer deleting an attendee row
(off-chain only — the on-chain `AttendeeDeposit` PDA persists) lets the same
user re-register, initiate a new pending deposit with the same wallet, then
have the recovery logic re-verify the new row against the old on-chain TX —
producing **two verified attendees, two QR codes, one on-chain deposit**.

The same bug is also exploitable by a malicious user (no organizer help
needed): register two emails with the same wallet, initiate a deposit for the
second, and let the ticket-page poller self-heal it.

## Context

- On-chain `AttendeeDeposit` PDA seeds: `[b"deposit", escrow, attendee_wallet]`
  → one PDA per `(event, wallet)`, enforced by `init` on the deposit
  instruction. The chain correctly blocks a second deposit for the same
  wallet; the off-chain DB does not.
- `event_store::find_attendee_by_wallet` already exists (D1 + KV fallback) and
  is used by the escrow-rollover path. There is no equivalent lookup by
  `tx_signature`.
- `deposit_usdc_handler` (`worker/src/handlers/deposit/usdc/handlers.rs` ~L256)
  dedups only by `attendee_id`, never by `wallet_address`.
- `recover_and_verify_deposit` (`worker/src/handlers/deposit/usdc/mod.rs`
  ~L825) sets `verified = true` after the signer cross-check passes, with no
  cross-attendee binding check.
- `deposit_statuses` table (migration `0014`) has indexes on `event_id` and
  `(event_id, attendee_id)` but not on `wallet_address` or `tx_signature` —
  the existing wallet lookup is a within-event scan.

## Non-negotiables

- **Defense in depth, not defense in optionality.** Both guards must be
  implemented; either alone is bypassable (Guard 1 misses legacy pending rows;
  Guard 2 is the actual safety net for the verify write).
- **No regressions on the verified happy path.** A legitimate single-deposit
  attendee loading the deposit page must still self-heal and get their QR.
- **Idempotent and self-skip.** Both checks must compare against the _current_
  `attendee_id` and only refuse when the binding is to a _different_ id. A
  recovery re-run for the same attendee must not false-positive.
- **Production grade.** No `unwrap()`, no `TODO`, no `unimplemented!()`,
  `snake_case`, `format!("{var}")`, `match` over `if` where a choice is
  intended. Best-effort DB failures on the _read_ side must not flip verified
  deposits to unverified; they must skip the guard with a warning.
- **On-chain truth wins.** If the binding check refuses, the deposit_status is
  returned unchanged (still pending, still unverified) — we never destroy
  on-chain facts, and we never fabricate them.
- **Additive migration.** New D1 migration must use `CREATE INDEX IF NOT
EXISTS` so it is safe to apply on already-deployed DBs.

## Tasks

### Phase 1 — Add `tx_signature` lookup helper (DRY mirror of wallet lookup)

- [x] **1.1 D1 query `db::deposit_statuses::find_attendee_by_tx_signature`**
      Mirror of `find_attendee_by_wallet`: parameterised SQL
      `SELECT attendee_id FROM deposit_statuses WHERE event_id = ?1 AND tx_signature = ?2 LIMIT 1`,
      same JsValue/JSON.stringify deserialisation pattern, returns
      `Result<Option<String>, String>`. Located in
      `worker/src/db/deposit_statuses.rs`.
- [x] **1.2 KV-fallback `event_store::find_attendee_by_tx_signature`**
      Mirror of `find_attendee_by_wallet` in `worker/src/event_store/read.rs`:
      D1-first, then KV scan over `list_deposit_statuses`, match on
      `tx_signature` (skip empty). Empty `tx_signature` short-circuits to
      `Ok(None)`.
- [x] **1.3 Re-export from `event_store::mod`**
      Add `find_attendee_by_tx_signature` to the `pub use read::{ ... }` block.

### Phase 2 — Guard 1: dedup by wallet at deposit initiation

- [x] **2.1 Insert wallet-binding check in `deposit_usdc_handler`**
      In `worker/src/handlers/deposit/usdc/handlers.rs`, immediately _after_
      the existing `attendee_id` dedup check (~L256) and _before_
      `increment_deposit_counter_with_fallback`, call
      `event_store::find_attendee_by_wallet(kv, &event.id, &body.wallet_address, d1)`.
      If it returns `Some(other_id)` where `other_id != body.attendee_id`,
      return `AppError::Validation` with message naming the conflict
      (`"wallet is already bound to another registration for this event"`).
      Map the underlying `Result` error to `AppError::Internal`.
- [x] **2.2 Add structured tracing on conflict**
      `tracing::warn!` with `event_id`, `new_attendee_id`, `existing_attendee_id`,
      `wallet` (full pubkey — it's already user-visible input, not PII) so the
      abuse pattern is visible in logs.

### Phase 3 — Guard 2: claim-binding in `recover_and_verify_deposit`

- [x] **3.1 Extract pure decision helper `binding_conflict`**
      In `worker/src/handlers/deposit/usdc/mod.rs`, add a `#[cfg(test)]`-visible
      pure function:
      `fn binding_conflict(current: &str, wallet_owner: Option<&str>, tx_owner: Option<&str>) -> bool`
      returns `true` iff either owner is `Some(id)` with `id != current`.
      This is the unit-testable core.
- [x] **3.2 Insert guard before `verified = true` write**
      After the `verify_tx_with_signer` `is_confirmed_and_matched()` gate
      passes and _before_ mutating `status.verified`, call both: - `event_store::find_attendee_by_wallet_with_fallback(kv, d1, &event.id, wallet)` (use
      the post-backfill wallet — the signer if backfilled, else the stored
      wallet) - `event_store::find_attendee_by_tx_signature_with_fallback(kv, d1, &event.id, sig)`

      If `binding_conflict(&status.attendee_id, wallet_owner, tx_owner)` is
      `true`, log a `tracing::warn!` with both bound ids, emit an audit entry
      (action `DepositConfirmed`? no — use a clearer narrative in meta), and
      `return status` *unchanged* (still unverified, still carrying the
      discovered `tx_signature` so a human can investigate).

- [x] **3.3 Make DB read failures non-fatal**
      If either lookup returns `Err`, log `tracing::warn!` and treat that
      signal as `None` (do **not** block verification on a transient DB hiccup
      — the signer cross-check already proved the TX is real). This keeps the
      happy path resilient and avoids flipping verified deposits to
      unverified.

### Phase 4 — D1 lookup indexes (additive migration)

- [x] **4.1 Create `worker/migrations/0018_deposit_statuses_lookup_indexes.sql`**

      ```sql
      CREATE INDEX IF NOT EXISTS idx_deposit_statuses_wallet
          ON deposit_statuses(event_id, wallet_address);
      CREATE INDEX IF NOT EXISTS idx_deposit_statuses_tx
          ON deposit_statuses(event_id, tx_signature);
      ```

      Both new queries become index range scans instead of within-event table
      scans. `IF NOT EXISTS` makes it idempotent on already-deployed DBs.

### Phase 5 — Tests

- [x] **5.1 Unit tests for `binding_conflict`**
      Cover all four truth-table rows for each input plus the
      `Some(current_id)` self-match case (must return `false` for both
      wallet and tx). Located in the existing `#[cfg(test)] mod tests` in
      `worker/src/handlers/deposit/usdc/mod.rs`.
- [x] **5.2 Regression assertion**
      Add a test that documents the _invariant_: when `wallet_owner ==
  Some(current_id)` and `tx_owner == Some(current_id)`, the helper returns
      `false`. This locks in the idempotent re-recovery behaviour.

### Phase 6 — Verify

- [x] **6.1 `cargo test -p event-checkin-worker --lib`** — 118 tests pass
      (was 112 + 6 new `binding_conflict` tests). Existing tests unaffected.
- [x] **6.2 `cargo clippy -p event-checkin-worker --all-targets`** — clean,
      no new warnings.
- [x] **6.3 `cargo fmt --check`** — all new code in `event_store/` is
      fmt-clean. Pre-existing fmt diffs in untouched files
      (`attendee.rs`, `escrow_index.rs`, `solana_escrow/tx_builders/*`, and
      pre-existing lines inside `handlers/deposit/usdc/{mod,handlers}.rs`) are
      out of scope for this security fix.

## Out of scope (deliberately)

- ~~**Refactoring `confirm_deposit_handler` / `verify_and_confirm_deposit` to
  share the new guard.** Those paths take the signature from a request body or
  webhook and already enforce the wallet→attendee link via
  `get_deposit_status_with_fallback` + signer cross-check. Folding them in is
  a separate, riskier change and not required to close this hole.~~
  **Wrong — reversed 2026-09-04 in `bd33825`. The signer cross-check does not
  enforce the wallet→attendee link. See §7.**
- **Frontend changes.** The backend rejects with a clear `Validation` error;
  the existing error toast path is sufficient.
- **Removing the orphaned prior-session files** (`DEMO.md`,
  `already_deposited.rs`, etc.) — separate narrative, separate commit.
- **Pushing `develop` to `origin`** — still held pending explicit user
  instruction.

## Risk & rollback

- **Risk: transient D1 unavailability.** Mitigated by 3.3 — read errors skip
  the guard rather than blocking verification.
- **Risk: false positive on legitimate re-recovery.** Mitigated by the
  `other_id != current_attendee_id` self-skip in `binding_conflict` and
  locked in by test 5.2.
- **Rollback:** revert the two code commits (handler + mod) and the migration.
  The migration is additive (no data change), so reverting it leaves orphan
  indexes that are harmless and can be dropped later.


---

## §7 — The webhook path never got the guards (2026-09-04, `bd33825`)

**The deferral above was the defect — and it named both paths that were
missing the guards.** Three entry points could set `DepositStatus::verified = true`:

| | read path | webhook path | polling endpoint |
|---|---|---|---|
| entry | `recover_and_verify_deposit` (`recover.rs`) | `verify_and_confirm_deposit` (`confirm.rs`) | `confirm_deposit_handler` (`handlers/confirm.rs`) |
| reached from | `/public/ticket` and friends | `POST /api/deposit/usdc/webhook`, detached via `wait_until` | `GET /api/deposit/usdc/confirm`, polled by the frontend |
| signer cross-check | ✅ | ✅ | ✅ |
| **F1** — on-chain `AttendeeDeposit` PDA | ✅ | ❌ | ❌ |
| **Guard 2** — `binding_conflict` | ✅ | ❌ | ❌ |
| discovery rate-limit cooldown | ✅ | n/a | ❌ |

The deferral rested on "already enforce the wallet→attendee link via
`get_deposit_status_with_fallback` + signer cross-check." Both halves are false:

1. **The signer cross-check does not prove a deposit happened.** `recover.rs`'s
   own F1 comment says so: *"the signer cross-check proves the wallet signed *a*
   confirmed tx — NOT that it was a real deposit … a confirmed-but-unrelated tx
   by the right fee-payer (e.g. a self-transfer) could earn a free verified
   ticket."* F1 was added to `recover.rs` afterwards and never mirrored here.
   `parse_get_transaction_response` checks confirmation and `accountKeys[0]` —
   never the destination, the mint, or the amount.
2. **When the record has no `wallet_address` there is no cross-check at all.**
   `verify_tx_with_signer(_, _, None)` returns `signer_matched: true` — "no
   expected wallet" is treated as a match (pinned by the pre-existing
   `test_parse_confirmed_no_expected_wallet_backfills_signer`). Such records
   demonstrably exist: `confirm.rs` and `recover.rs` both carry a
   `wallet_address` backfill for "older record or web2 hiccup at deposit
   creation time." On that record, **any** confirmed signature on the cluster
   flipped the deposit to verified and backfilled a stranger's pubkey.

Reachable: the route is mounted at `handlers/mod.rs:152` and the JWT branch
accepts *any* valid token — it never binds the caller to `body.attendee_id`.

**Fix — delete the copy, don't patch it.** `recover_and_verify_deposit` was
already a strict superset of `verify_and_confirm_deposit`: same
`verified = true` transition, same D1 dual-write, same audit entry, same sheet
write and QR auto-gen (and it detaches those via `wait_until` rather than
blocking). `confirm.rs` is now a ~100-line adapter that loads the `EventConfig`
and the `DepositStatus` and hands both over. One implementation is what stops
the two from drifting apart a third time.

It **fails closed** on an unloadable event config: F1 needs `organizer_wallet`,
`on_chain_event_id` and `deposit_amount_usdc` to check the PDA, so no config
means no provable deposit and no verification. Four bail-outs total (config
missing / unreadable, status missing / unreadable).

**Two further defects in the same handler**, both fixed in the same commit:

- **The rejection log leaked the credential.** `tracing::warn!(auth = %auth_header, …)`
  wrote the full `Authorization` header into Cloudflare request logs on every
  failed attempt — a near-miss `WEBHOOK_SECRET`, or a JWT valid on another
  route. Now logs only its shape (`auth_present`, `auth_is_bearer`, `auth_len`).
- **A verified deposit's signature could be clobbered.** The handler wrote
  `deposit_status.tx_signature = Some(body.tx_signature)` unconditionally. The
  JWT branch authenticates the caller but does not bind them to
  `body.attendee_id`, so any token holder could replace a settled money
  record's proof-of-payment with an arbitrary string — `verified` would survive
  while the signature backing it, and every refund/audit path reading it, would
  not. Now returns the recorded signature unchanged once `verified`; re-sending
  the same signature stays a no-op success, so Helius retries are safe.

**Guard: `worker/tests/deposit_verify_guard.rs`** (5 tests), mutation-tested
three ways — restoring the pre-fix `confirm.rs` verbatim, deleting the
verified-clobber guard, and restoring the raw-header log each turn it red.
Source restored, all gates green (`fmt`, `clippy -D warnings`, 530 workspace
tests, wasm32 release build).

### The polling endpoint — the third copy (`e8fc217`)

`confirm_deposit_handler` was ~270 lines duplicating `recover.rs` end to end:
PDA discovery in the no-signature arm, signer cross-check, `verified = true`,
D1 dual-write, detached sheet write, QR auto-gen, and a blocking fallback for
tests. Same two guards missing. The route requires identity (the VULN-004 fix)
but never binds the caller to `query.attendee_id`, so any signed-in attendee
could drive it on anyone's behalf — and it is the endpoint the frontend
actually polls in a loop, while it was the only one of the three *without* the
read path's `DISCOVERY_COOLDOWN_SECS` rate limit on `getSignaturesForAddress`.

Now ~100 lines: load the config and the record, hand both to
`recover_and_verify_deposit`, map the returned status onto
`ConfirmDepositResponse`. Every observable response is preserved — verified,
pending-with-signature, and the Solana Pay retry URL when nothing is recorded or
discoverable. "Not yet confirmed", "RPC error" and "a guard refused" all collapse
to the same pending response, which is what the endpoint already did and the only
thing the frontend can act on. **Net −331 lines across the two fixes.**

**The re-exports were narrowed to match.** `usdc/mod.rs` no longer exports
`verify_tx_with_signer`, `VerifyWithSignerOutcome` or
`discover_deposit_tx_on_chain` — clippy flagged all three as unused the moment
the last duplicate went away, which is the collapse proving itself. They are now
private to the module, so **restoring either pre-fix handler verbatim no longer
compiles**. That is a stronger guard than a test; `deposit_verify_guard.rs` (7
tests) covers the shapes that still would, and was mutation-tested against one.

### Still open

- **The JWT branch does not bind the caller to `body.attendee_id`.** Any valid
  token may report a signature for any attendee in any event. The money impact
  is now closed by F1 + Guard 2 + the verified-clobber guard, but the authz
  model is still wider than the endpoint needs. Narrowing it means deciding
  whether legitimate callers include organiser/admin tokens — an owner call,
  not a mechanical fix.
- **`auth_header == expected` for the webhook secret is not constant-time.**
  Remote timing analysis across the Cloudflare edge is impractical, and the
  comparison is on a `String` of fixed expected length; left alone, recorded
  here so it is a decision rather than an oversight.
- **Not deployed.** This branch is unpushed; the vulnerable code is what runs
  in production today.
