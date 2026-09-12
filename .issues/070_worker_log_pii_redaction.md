# 070 — Worker log PII redaction

**Status:** Code complete — every personal or durable identifier in the Worker
log stream is a keyed fingerprint, and the source guard is enabled. Only the
staging log inspection (step 6) remains.  
**Priority:** P1 security/privacy  
**Created:** 2026-09-12

## Problem

Issue 064 removed raw claim-token capability values from Worker logs. A follow-up
source scan found direct structured logging of other personal or durable
identifiers, including attendee/staff email addresses, wallet addresses, and
transaction signatures. These values are useful for support correlation but do
not belong in the general Worker log stream.

The affected paths include walk-in registration, profile and social linking,
THB deposit/credit handling, privacy requests, event administration, Sheets
mirrors, and escrow indexing. Do not paste a raw scan output into tickets: it
can contain real field names and encourages copying PII into another system.

## Decision

- Keep the D1 audit log as the access-controlled, actor-attributable record for
  authorized operational actions.
- Replace Worker-log identifiers with a stable, irreversible, fixed-length
  SHA-256 fingerprint. Use a distinct field name such as
  `identity_fingerprint`; do not overload `claim_token_fingerprint`.
- Retain non-sensitive event IDs, aggregate counts, operation kind, error class,
  and correlation ID so production debugging remains useful.
- Never log a full wallet, email, claim token, OAuth value, API key, or raw
  provider transaction signature. Explorer URLs remain UI/API behavior, not
  structured log fields.

## Implementation plan

- [x] 1. Generalize the existing SHA-256 helper in `worker/src/crypto.rs`
      without changing the claim-token fingerprint wire/log shape.
- [x] 2. Convert direct attendee identifiers first: registration, walk-in,
      claim, deposits/refunds/credits, profile/social linking, privacy,
      check-in, and attendee management.
- [x] 3. Convert staff/organizer identifiers in request logs. The event audit
      entry continues to carry the authorized actor under its existing access
      controls.
- [x] 4. Convert wallet and signature fields to a dedicated fingerprint where
      they are not required to diagnose an on-chain instruction. Event IDs and
      an internal correlation ID must be enough to find the record through
      controlled data stores.
- [x] 5. Add a source guard that fails tests on tracing fields named `email`,
      `staff_email`, `attendee_email`, `wallet`, `wallet_address`, `signature`,
      or `tx_signature` when rendered with `%` or `?`. Allow only a reviewed,
      narrow list of non-PII names if a false positive is demonstrated.
      *Enabled as `worker/tests/log_pii_guard.rs` now that the migration is
      complete.*
- [ ] 6. Re-run the full Worker suite and inspect a staging request's logs
      manually using only a disposable fixture.
      *The five-gate suite is green. Staging log inspection is still pending
      and needs a disposable fixture — the only open item on this issue.*

### Completed slice — 2026-09-12

- Added a keyed `identity_fingerprint` helper. It uses the deployment JWT
  secret and a domain separator, so public hash-table lookups cannot recover
  low-entropy identifiers from Worker logs. Secret rotation intentionally
  begins a new diagnostic correlation window.
- Converted all attendee and staff tracing fields in `handlers/walkin.rs`.
  The protected D1/KV audit entry and the authenticated CSV/API response retain
  their existing authorized operational data; only the general Worker log
  stream changed.
- This does **not** close the issue: registration, claim, deposits, profile,
  privacy, admin, wallet, and transaction-signature paths still require the
  same migration and then the source guard.

### Completed slice 2 — 2026-09-12

First, a defect from slice 1 was repaired. The helper extraction had left
`claim_token_fingerprint` calling `short_fingerprint(token.as_bytes())` — the
raw token bytes, with no digest — so it emitted the first eight characters of
every claim token as plaintext hex and reintroduced exactly what Issue 064
closed. The hash is restored and the existing two-token test covers it.

Then 38 further tracing sites were converted across four files, all now at zero
raw identifiers:

| File | Sites | Identifiers removed from the log stream |
|---|---|---|
| `handlers/deposit/usdc/rpc.rs` | 16 | TX signature, extracted signer, expected wallet |
| `handlers/deposit/thb/handlers/hold_refund_request.rs` | 13 | attendee email, admin email, target contact email |
| `handlers/deposit/usdc/discovery.rs` | 5 | TX signature, AttendeeDeposit PDA, attendee wallet |
| `handlers/deposit/usdc/recover.rs` | 4 | TX signature |

Three design points worth carrying forward:

- **`LogRedactor` (new, in `crypto.rs`).** `rpc.rs` and `discovery.rs` are
  stateless and hold no `AppState`, so they could not reach the deployment
  secret. Rather than thread a raw secret through a pure parser's signature —
  and into its unit tests — the handler that already holds `AppState` builds a
  `LogRedactor` and passes it down. `LogRedactor::fingerprint` is exactly
  `identity_fingerprint` under the same key, so lines emitted through either
  style correlate with each other; a test asserts that equivalence.
- **Signatures and wallets are keyed, not plain-hashed.** Both are public
  on-chain values, so an unkeyed digest of either is reversible by anyone who
  indexes the chain. They use the same keyed helper as emails for that reason.
- **The AttendeeDeposit PDA is treated as personal.** It is derived from the
  attendee wallet, so logging it raw re-exposes the wallet. The escrow address
  is event-level and stays readable, because without it these lines are not
  diagnosable at all.

Deliberately left raw: `attendee_id` and `event_id` (internal identifiers the
Decision section keeps), and the `tx_signature` inside `append_event_audit`'s
meta payload in `recover.rs` — that is the access-controlled D1 audit record,
not the general log stream.

### Completed slice 3 — 2026-09-12

The migration is finished and the guard is on.

**The remaining-site count in slice 2 was wrong.** The line-anchored regex it
reported (83 sites / 43 files) under-counted by more than 2x: it could not see
fields on macro continuation lines, and its field list omitted `sig`,
`claims_email`, `recorded_tx_signature`, `incoming_tx_signature`, `telegram_id`,
`github`, and attendee display `name`. A paren-depth scanner that walks whole
`tracing!` bodies with string literals blanked found **177 sites across 50
files**. All 177 are now converted. That scanner is what the source guard runs.

**DRY foundation.** Slice 2 spelled out
`crate::crypto::identity_fingerprint(&x, &state.config.jwt_secret)` at each
call site — ten times in `walkin.rs` alone, re-hashing per log line. Two methods
on `AppState` now hold the secret access in one place:

- `AppState::log_fingerprint(&self, identifier) -> String` for handlers.
- `AppState::log_redactor(&self) -> LogRedactor<'_>` for handing down into
  stateless helpers.

Hot paths bind the fingerprint once per request (`signup.rs`, `privacy.rs`,
`hold_refund_request.rs`) rather than re-deriving it on every line.

**Threading into stateless code.** Five pure/background functions could not
reach `AppState`, so they take a `LogRedactor` parameter rather than a raw
secret: `poll_escrow_events`, `index_helius_transactions`,
`parse_helius_transaction`, `apply_rollover_deposit_status`, and
`run_backfill_workflow`. `DeveloperData` carries one as a field for
`write_developer_data`. `parse_helius_transaction`'s four unit tests construct
`LogRedactor::new("test-log-secret")` — accepting a small test-surface change
was better than losing signature correlation on instruction-decode failures.

**Scope calls made during this slice:**

- **Attendee display names are redacted** (`name_fingerprint`). A full human
  name is personal data of the same class as the email, and leaving it readable
  beside a redacted email defeats the redaction.
- **Event, organization, and escrow names/addresses stay readable.** They are
  not personal, and without them these lines are not diagnosable. The guard's
  forbidden-field list therefore excludes the bare name `name`; a separate
  assertion covers the attendee-name expressions specifically.
- **GitHub logins and Telegram IDs are redacted.** Both are durable account
  identifiers obtained via OAuth, which the Decision section already covers.
- **`waitlist.rs` fields were mislabeled `staff_email`.** The waitlist is a
  public signup form, so they are now `subscriber_fingerprint`.
- **`DEV_MODE`'s warning in `state.rs` is fingerprinted.** It runs inside
  `build_config` before `AppConfig` exists, so it reads `JWT_SECRET` directly
  using the same fallback `jwt_secret` uses — the value still correlates with
  the rest of the stream.

**The guard is self-tested.** `the_guard_detects_a_reintroduced_identifier`
asserts five known-bad snippets are flagged and five known-good ones are not,
so a bug in the scanner cannot make the guard silently pass on everything.

**Gates:** fmt, `check --workspace --all-targets`, `clippy -D warnings`,
`test --workspace` (0 failed), and the wasm32 release build all pass.

## Acceptance criteria

- [x] The source guard passes and raw PII identifiers no longer appear in
      Worker tracing fields. *(`log_pii_guard`, 3 tests including a
      self-test that the guard is not vacuous.)*
- [x] Claim-token fingerprint behavior remains stable. *(Regression found and
      fixed in slice 2; covered by `claim_token_fingerprint_is_short_stable_and_one_way`.)*
- [x] Administrative audit history still records the authorized actor and
      action correctly. *(No D1 audit payload was touched; only the general
      log stream changed.)*
- [ ] Staging walk-in, profile, deposit, and claim error paths produce useful
      redacted diagnostics.
- [ ] No production data mutation or provider call is needed for validation.

## References

- [Issue 064](064_claim_token_log_redaction.md)
- [Core readiness audit](066_core_services_readiness_audit.md)
- [Operator handover](../docs/operator-handover.md)
