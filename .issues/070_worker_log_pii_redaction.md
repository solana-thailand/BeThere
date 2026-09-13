# 070 — Worker log PII redaction

**Status:** Verified — every personal or durable identifier in the Worker log
stream is a keyed fingerprint, the source guard is enabled, and a runtime probe
against a locally running Worker confirms the live log stream is clean. One
residual is documented below and is outside the Worker's control.  
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
- [x] 6. Re-run the full Worker suite and inspect a request's logs manually
      using only a disposable fixture.
      *Done as a local runtime probe rather than a staging deploy — see
      "Completed slice 4". `scripts/verify/pii_log_probe.sh` drives 55 requests
      with sentinel identifiers through `wrangler dev --local` and greps the
      captured log; it found three classes of leak across 18 sites that the
      source guard could not see, plus the request-path class. A follow-up
      review of what the probe itself could not reach found a fourth class —
      see "Completed slice 5". All fixed.*

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

### Completed slice 4 — 2026-09-12 (runtime probe)

**The runtime inspection was done locally, not on staging.** A local
`wrangler dev --local` worker with a disposable `.dev.vars` (fake Google
credentials, `GOOGLE_SERVICE_ACCOUNT_TOKEN_URI` pointed at a closed local port)
is strictly stronger for this check than a staging deploy: it involves no real
attendee data, mutates no deployed store, and lets every identifier in the run
be a unique sentinel, so *any* hit in the captured log is a leak by
construction. `scripts/verify/pii_log_probe.sh` is that harness — seeded local
D1 fixture, 55 requests across the public, webhook, attendee, and staff/admin
surfaces, then `--grep` over the captured log. The one outbound call that left
the machine was a Helius DAS lookup with a fake API key for a fake wallet, which
returned 401.

**It found leaks that the source guard structurally could not.** The guard
inspects tracing *field names* and blanks string literals, so three whole
classes were invisible to it:

| Class | Sites | Example |
|---|---|---|
| Identifier interpolated into the *message* | 8 | `"listing attendees (requested by: {})", claims.email` |
| Field name absent from the forbidden list | 9 | `verifier = %claims.email`, `marker = %claims.email`, `developer_email = %developer_email`, `actor = %claims.email`, `uploader_email = %claims.email`, `%bg_email` |
| Recorded with no sigil, so the `= %`/`= ?` check missed it | 1 | `claim_token = d1_attendee.claim_token.as_deref()…` — a raw capability token, the exact regression Issue 064 closed |

All 18 are converted to keyed fingerprints, in `handlers/auth.rs` (x2),
`handlers/adventure.rs` (x3), `handlers/attendee/{admin,list,read}.rs`,
`handlers/ext.rs`, `db/campaigns/checkin.rs` (x2), `handlers/notifications.rs`,
`handlers/register/signup.rs`, and the THB `slip_verify` / `slip_upload` /
`refund` (x3) handlers. `on_event_checkin` now takes a precomputed
`developer_fingerprint` rather than a `LogRedactor`, because it runs inside
`ctx.wait_until` and cannot hold a borrow of the secret.

**The request path was logging capability tokens.** `middleware/correlation.rs`
recorded `path = %req.uri().path()` on every request, which put claim, quiz and
adventure tokens plus wallet addresses back into the log stream that the rest of
this issue had cleared. It now logs a redacted path. Redaction is *shape*-driven
(UUID-shaped or base58-key-shaped segments become `{id}`) rather than a
per-route table, so a new route that takes an identifier segment is covered the
day it is added, while slugs and literal segments stay readable —
`/api/claim/{id}`, `/api/wallet/{id}/nfts`, but
`/api/public/event/solana-bangkok-deep-dive` unchanged. Five unit tests cover
it. Nothing is lost diagnostically: the handler behind each of those routes
already emits a keyed fingerprint of the value under the same `correlation_id`.

**The guard was hardened so each class fails the suite from now on**
(`worker/tests/log_pii_guard.rs`, now 5 tests):

- `identifiers_are_not_interpolated_into_log_messages` scans tracing bodies with
  literals *kept*, after stripping every `…fingerprint(<balanced>)` call, so an
  identifier reaching the stream through a message or a plain argument is
  flagged wherever it sits. This is the check that found the nine
  unusually-named fields, and it does not need the field-name list to be
  complete.
- `request_paths_are_redacted_before_logging` forbids `uri().path()` inside any
  tracing body and asserts the middleware still calls `redact_path`.
- `renders_field` now accepts **any** `field = …`, not just `= %` / `= ?`, and
  `claim_token` / `token` joined the forbidden names. The self-test gained the
  sigil-less and message-interpolation cases, so a regression in either
  direction fails.

**Residual, and it is not fixable in this codebase.** The *platform* request log
records the raw URL — locally the `[wrangler:info] GET /api/claim/<token>` lines,
in production Cloudflare's request metadata. A capability token that travels in
a path is therefore visible to anyone with access to Workers Logs, regardless of
what the Worker itself logs. Options and a recommendation are in
[Issue 071](071_capability_token_in_url_platform_logs.md); this issue does not
close it and must not be reopened for it.

**Verification result.** 55 requests, 54 of which reached the Worker; zero
sentinel hits in the tracing output, with fingerprints present and stable across
lines (`staff_fingerprint`, `claim_token_fingerprint`, `identity_fingerprint`,
`subject_fingerprint`, `attendee_fingerprint`, `subscriber_fingerprint`,
`name_fingerprint`, `wallet_fingerprint`). Gates: fmt, `check`, `clippy -D
warnings`, `test --workspace` (631 passing, 0 failed), wasm32 release build.

### Completed slice 5 — 2026-09-12 (the paths the probe could not reach)

The slice 4 probe deliberately made Google credentials unusable, so every Sheets
helper failed at the token fetch and **nothing past that point ever executed**.
A review of what the probe was structurally blind to found a fourth leak class
and two structural gaps in the guard itself.

**An identifier written into an error message reaches the log stream from a
different function.** `sheets::contacts::{increment_credit, decrement_credit,
set_credit_refund_requested}` returned `"contact not found: {email_lower}"`, and
four deposit-credit call sites log a failed mirror as `error = %e`
(`hold_credit.rs`, `hold_admin.rs`, `hold_refund_request.rs` x2). The realistic
trigger — an attendee who was never synced to the Contacts sheet holds a deposit
— therefore put a raw address in the log next to the fingerprint that was
supposed to replace it. No guard could see it: the field is `error`, the
expression is `%e`, and the identifier is added ~300 lines away. The message is
now `"contact not found in contacts sheet"`; the caller's fingerprint field
already carries the identity.

Two `AppError::NotFound(format!("no registration found for {} …", claims.email))`
messages (`handlers/adventure.rs`, `handlers/register/my_registration.rs`) are
fixed for the same reason even though they are 404s and `WorkerError` only logs
5xx bodies: the address tells the requester nothing they do not know, and the
latent leak is one variant reclassification away.

**Two structural gaps in the guard, both closed:**

- Every scan in `log_pii_guard.rs` extracts `tracing::<level>!(` bodies, so a
  bare `info!(email = %claims.email, …)` reached through `use tracing::info`
  would have been invisible to *all* of them. All 870 call sites are
  path-qualified today; `tracing_macros_stay_path_qualified` keeps it that way,
  which is what makes the other scans exhaustive rather than best-effort.
- The scans cover `worker/src` only. `the_domain_crate_stays_log_free` asserts
  the reason that scope is complete — `domain` has no logging and no `tracing`
  dependency — so the day domain starts logging, the suite says to extend the
  guards instead of silently leaving a crate uncovered.

`error_messages_do_not_embed_identifiers` is the new leak-class guard. It flags
an identifier in a `format!` that sits **inside** an error-constructing call
(`map_err`, `ok_or_else`, `Err(`, `AppError::<Variant>(`, `.context`, `anyhow!`),
matching capture names and argument expressions but never the literal prose, so
`"D1 find_attendee_by_wallet bind: {e:?}"` stays legal. Span containment — not a
backward text window — is what keeps `Err(e) => { … }` match arms and a
`map_err(AppError::Internal)?` on the previous line out of scope. `mask_wallet`
/ `mask_email` join `…fingerprint(` as recognized sanitizers, since a reviewed
partial rendering in a user-facing message is a deliberate product decision.

Both new guards were mutation-checked against a real file placed under
`worker/src`: each flags its class, and `identifiers_are_fingerprinted_before_logging`
demonstrably does **not** — which is the gap being closed.

**One hazard found and left in place, deliberately.** `attendee_id = %api_id`
appears at eight log sites on the assumption that an `api_id` is an internal
identifier — and it is, except at `claim/mint/lookup.rs`, where a walk-in has no
attendee row and its claim *response* is keyed `walkin:<email>`. That value is
never logged today (walk-ins return from `lookup_claim` before any of the eight
sites, and `virtual_checkin`/`execute_claim` take a store `Attendee`), and
changing the field would change the claim API contract for no log-stream gain.
A comment at the construction site records the invariant instead.

Gates: fmt, `clippy --workspace --all-targets -D warnings`, `test --workspace`
(634 passing, 0 failed), wasm32 release build.

## Acceptance criteria

- [x] The source guard passes and raw PII identifiers no longer appear in
      Worker tracing fields. *(`log_pii_guard`, 8 tests: fields, display names,
      message interpolation, request paths, error messages, macro qualification,
      domain scope, plus a self-test that the guard is not vacuous.)*
- [x] Claim-token fingerprint behavior remains stable. *(Regression found and
      fixed in slice 2; covered by `claim_token_fingerprint_is_short_stable_and_one_way`.)*
- [x] Administrative audit history still records the authorized actor and
      action correctly. *(No D1 audit payload was touched; only the general
      log stream changed.)*
- [x] Walk-in, profile, deposit, and claim error paths produce useful redacted
      diagnostics. *(Verified on a local runtime probe rather than staging —
      slice 4. Both success and error paths were exercised; every line still
      carries a correlation ID plus a stable fingerprint.)*
- [x] No production data mutation or provider call is needed for validation.
      *(The probe runs entirely against local D1/KV with fake credentials. The
      one outbound call was a Helius DAS 401 for a fake wallet.)*

Not covered by a local probe, and deliberately left as an operator check on the
next deploy: that Cloudflare's own observability pipeline renders these lines the
same way `tracing-wasm` does locally, and the platform URL residual above.

## References

- [Issue 064](064_claim_token_log_redaction.md)
- [Issue 071](071_capability_token_in_url_platform_logs.md) — the URL/platform-log residual
- [Core readiness audit](066_core_services_readiness_audit.md)
- [Operator handover](../docs/operator-handover.md)
