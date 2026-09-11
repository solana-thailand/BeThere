# 066 — Core services production-readiness audit

**Status:** Open  
**Priority:** P0  
**Audit date:** 2026-09-10

## Why this is open

The core paths have strong unit/invariant coverage, but production data and the
live integration boundary still expose gaps that can block attendees or leave
money/NFT state inconsistent. This issue is the release checklist for NFT,
quiz/adventure, registration, deposits, attendee UX, landing, and public slug
pages.

## Production evidence (aggregate only)

- 16 events total; 13 active.
- 15 events have `quiz_enabled`; only 3 have a `quiz_configs` row.
- 10 active events have quiz enabled without quiz configuration.
- 37 checked-in, unclaimed attendees belong to events where quiz is enabled but
  no quiz configuration exists. Claim correctly fails closed, so these attendees
  can be blocked until the event is repaired.
- 8 events have deposits enabled; one active event has an escrow address.
- No deposits were pending at audit time.
- 85 attendees were checked in and unclaimed across all events.

No attendee PII was queried for this audit.

## P0 — complete before the next core production release

### Quiz/adventure configuration integrity

- [x] Add one shared readiness validator for event activation and admin quiz update.
- [x] Reject transitions to `active + quiz_enabled` unless an enabled, structurally valid quiz
      config with at least one enabled question exists.
- [x] Show quiz readiness failures in the admin Events page.
- [x] Add an authorization-filtered aggregate admin repair report for existing invalid events.
- [ ] For each affected production event, explicitly configure the quiz or
      disable the gate; do not bulk-disable without organizer review.
- [ ] Re-check the 37 affected attendee records after repair and verify claim
      eligibility without minting.

### NFT consistency and network readiness

- [ ] Replace the current mint-then-best-effort-D1 flow with a durable mint
      intent/outbox state machine (`pending`, `submitted`, `confirmed`,
      `persisted`, `needs_reconciliation`).
- [x] Persist provider request/idempotency metadata before calling Crossmint.
- [x] Reconcile submitted mints by provider idempotency key after timeouts or D1
      write failures; never make a second mint while outcome is ambiguous.
- [x] Apply the same implementation to pre-registered and walk-in claims.
- [x] Make a failed post-mint D1 write observable and recoverable instead of
      returning an untracked success.
  Implemented by migration `0033_nft_mint_jobs.sql` and the shared mint journal.
  Crossmint receives a SHA-256 custom mint ID through its idempotent PUT route;
  D1 records that ID before external I/O, while KV retains confirmed results as
  a seven-day recovery copy until the attendee projection succeeds. The broader
  state-machine item remains open for explicit terminal failure/reconciliation
  states and provider-side pending reconciliation. A daily idempotent sweeper
  now repairs missing attendee projections from confirmed journal rows, marks
  exact matches persisted, and alerts on inconsistent confirmed jobs or pending
  jobs older than one hour.
- [ ] Validate Crossmint host, collection, and collection network at startup or
      event activation. Production currently uses `www.crossmint.com` while the
      config comment identifies the collection as devnet.
- [x] Report separate NFT/RPC and escrow clusters in health/readiness. The current
      health cluster inferred from the Helius URL can say mainnet-beta while
      `SOLANA_CLUSTER` for escrow is devnet.
- [x] Add a read-only NFT readiness check. Do not use a real mint as a health
      check because it has external side effects and may cost money.
  `/api/health` now reports each network role, whether RPC/NFT credentials are
  present, and stable mismatch warning codes. The legacy `cluster` field follows
  the escrow network used for wallet signing. The check performs no provider
  request and has no external side effect.

### Live release gate

- [x] Finish the staging flow harness HTTP/TX implementations. Deposit,
      refund, claim, and auth seams are wired; stale `TODO(staging-live)` prose
      no longer represents executable stubs.
- [ ] Cover registration -> deposit -> verification -> ticket -> check-in ->
      quiz -> NFT claim, including retry/idempotency and refund/forfeit branches.
- [x] Make the live staging preflight gate default-on for production deploys,
      with an explicit audited emergency bypass. `worker/deploy.sh` now always
      requires a fresh green sentinel for production; `--force` is accepted
      only with a non-empty `--reason` and appends the existing audit record.
- [ ] Use dedicated staging fixtures and wallets with strict balance limits.
      The harness now rejects production-looking Worker hosts, non-devnet RPC
      URLs, and payer/attendee signer mismatches before any transaction; wallet
      provisioning and the first live green run remain.

## P1 — core correctness and attendee experience

### Deposit and credit flow

- [x] Make credit spend + verified deposit creation one recoverable workflow.
      Registration and admin recovery now share one idempotent D1 transaction
      covering the ledger spend, credit projection, and verified ticket status.
      Retries repair legacy credit projections without spending twice; cash
      deposits are never overwritten. Daily reconciliation safely repairs the
      known legacy THB status mismatch and reports anything still incomplete.
- [ ] Consolidate THB verification writes so deposit status, attendee projection,
      and QR readiness cannot partially succeed without a reconciliation job.
- [ ] Add a reconciliation view/job for ledger, THB deposit, deposit status,
      attendee deposit fields, refund/credit settlement, and Sheets mirrors.
- [ ] Show the exact currency and network before wallet signing; remove every
      remaining `SOL` reference where the product actually takes USDC.
- [ ] Keep escrow on devnet until program deployment/audit and live rollover E2E
      items in Issues 001, 013, and 040 are completed.

### Registration and attendee UX

- [ ] Add browser E2E for new registration, duplicate registration, capacity
      race, online/in-person choice, credit-covered registration, and expired
      deposit recovery.
- [ ] Preserve server-side D1 unique enforcement and D1-before-credit ordering.
- [ ] Add resumable “My registration” recovery for users who clear local storage
      or change devices, with email ownership checks retained.
- [ ] Test mobile and accessible states for pending/rejected deposit, sold out,
      waiting, checked in, quiz blocked, mint pending, mint retry, refund, and
      held-as-credit.
- [ ] Give every long external operation a clear pending state and safe retry
      instruction tied to the same idempotency key.

### Quiz/lesson flow

- [ ] Add a preview/readiness indicator showing the exact attendee-visible gate.
- [ ] Define versioning behavior when organizers edit questions after attendees
      have started or passed.
- [ ] Add browser E2E for pass, retry, malformed/no config, already passed, and
      event-ended behavior.
- [ ] Decide and document whether walk-ins intentionally skip quiz/adventure;
      the current implementation always skips both.

## P2 — product truth, performance, and maintenance

- [ ] Correct landing copy that says `0.01 SOL` / `SOL/USDC`; event payment is
      configured in THB and USDC and refunds are not universally on-chain.
- [ ] Ensure public slug/detail pages derive deposit/refund/NFT wording from the
      actual event configuration and network.
- [ ] Run a Chrome DevTools Core Web Vitals trace for landing and a representative
      public slug page. The audit attempt was blocked by the shared Chrome profile
      lock, so no trustworthy LCP/INP/CLS result is recorded yet.
- [ ] Add performance budgets for initial WASM/JS, hero image, and public-event
      API latency after measuring a cold and warm production load.
- [ ] Sync stale plan/issue deployment statuses (Plans 021/023/024 and related
      issue checklists) with the 2026-09-10 production release.

## Existing strengths to preserve

- Registration writes the attendee to D1 before spending rolling credit.
- D1 enforces event/email uniqueness against concurrent duplicate registration.
- Deposit/credit settlement has atomic ledger and CAS protections in core paths.
- Claim wallet provenance, claim locks, deposit verification, capacity, and
  pagination have meaningful invariant tests.
- Sheets is generally a detached display mirror after the D1 source-of-truth
  migration.

## Exit criteria

This issue closes when all P0 items are complete, production-invalid quiz events
are reconciled, the live staging gate passes twice consecutively, and there are
no unresolved ambiguous mint or deposit/credit records. P1/P2 work may move into
separate linked issues once each has an owner and acceptance tests.
