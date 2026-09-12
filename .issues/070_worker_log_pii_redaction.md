# 070 — Worker log PII redaction

**Status:** Open  
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

1. Generalize the existing SHA-256 helper in `worker/src/crypto.rs` without
   changing the claim-token fingerprint wire/log shape.
2. Convert direct attendee identifiers first: registration, walk-in, claim,
   deposits/refunds/credits, profile/social linking, privacy, check-in, and
   attendee management.
3. Convert staff/organizer identifiers in request logs. The event audit entry
   continues to carry the authorized actor under its existing access controls.
4. Convert wallet and signature fields to a dedicated fingerprint where they
   are not required to diagnose an on-chain instruction. Event IDs and an
   internal correlation ID must be enough to find the record through controlled
   data stores.
5. Add a source guard that fails tests on tracing fields named `email`,
   `staff_email`, `attendee_email`, `wallet`, `wallet_address`, `signature`, or
   `tx_signature` when rendered with `%` or `?`. Allow only a reviewed,
   narrow list of non-PII names if a false positive is demonstrated.
6. Re-run the full Worker suite and inspect a staging request's logs manually
   using only a disposable fixture.

## Acceptance criteria

- [ ] The source guard passes and raw PII identifiers no longer appear in
      Worker tracing fields.
- [ ] Claim-token fingerprint behavior remains stable.
- [ ] Administrative audit history still records the authorized actor and
      action correctly.
- [ ] Staging walk-in, profile, deposit, and claim error paths produce useful
      redacted diagnostics.
- [ ] No production data mutation or provider call is needed for validation.

## References

- [Issue 064](064_claim_token_log_redaction.md)
- [Core readiness audit](066_core_services_readiness_audit.md)
- [Operator handover](../docs/operator-handover.md)
