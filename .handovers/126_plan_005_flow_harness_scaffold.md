# Handover 126 — Plan 005 §3.4 `flow-harness` scaffold

## What happened

Scaffolded the `flow-harness/` crate — the E2E regression harness for Plan 005 §3.4. This is the staging-independent skeleton: every assertion, PDA derivation, response-shape check, and the runner/results-writer is real and `cargo test`-able offline. Only the flow `run` bodies' HTTP/TX execution is gated behind `// TODO(staging-live):` markers (each fails fast with `HarnessError::Config` until §3.1 is provisioned).

Decision: **scaffold §3.4 now** rather than pause, because the skeleton is strictly staging-independent, it's a brand-new crate (zero risk to existing code), and it unblocks §3.5 the moment staging goes live.

### Built
- `flow-harness/Cargo.toml` — standalone crate (`[workspace]` table keeps it out of the root workspace so its native deps never pollute the worker's wasm32 build or domain's dual-target).
- `src/assertions.rs` — **the regression core**. `predict_refund_outcome` + `refund_cta_enabled` are literal transcriptions of the on-chain guard (`bethere-escrow/src/instructions/refund.rs#L72-85`). Full truth-table unit tests. This is the safety net for divergence #19.
- `src/context.rs` — `StagingContext`: worker URL, payer keypair (wrapped in `Arc<Keypair>` because `Keypair` isn't `Clone`), derived `EventEscrow` + `AttendeeDeposit` PDAs against the real program id (`C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`) and seed layout (`["escrow", organizer, event_id]` / `["deposit", event, attendee]`).
- `src/client.rs` — typed HTTP client mirroring the 12-row endpoint surface (`docs/escrow_contract_surface.md` §6). Lenient error-envelope parsing (3 known shapes) with `EscrowCode` extraction for negative tests.
- `src/runner.rs` — orchestrator + `summary.json` writer + `.last-green` sentinel (the §3.5 gate signal). Empty-run-is-not-green semantics; records per-flow outcome/duration/error-kind.
- `src/error.rs` — `HarnessError` + `EscrowCode` (mirrors program codes 1/4/19/22 + `Other(n)` round-trip).
- `src/flows/{deposit,refund_pre_event_end,refund_post_event_end_checked_in,refund_no_show_deadline,claim,auth}.rs` — six flows, each with config defaults aligned to `seed-staging.sh`, pure precondition/outcome/gate helpers, and staging-gated execution stubs.
- `src/main.rs` — clap CLI. Exit codes: `0` all passed, `1` flow failure, `2` misconfiguration (no flow executed).
- `README.md`, `.gitignore` (`/results/*` except `.last-green`).

### Verification
- `cargo test --manifest-path flow-harness/Cargo.toml` → **114 passed, 0 failed, 0 ignored**.
- `cargo clippy --all-targets` → exit 0; only 11 pedantic doc-comment indentation lints remain (`doc_lazy_continuation` / `doc_overindented_list_items`), all in markdown prose.
- `cargo run` with no env → exit 2 with actionable config error (proves fail-fast).
- Root workspace isolation confirmed: `cargo metadata` shows root members are exactly `[event-checkin-domain, event-checkin-worker]`; `flow-harness` is its own workspace root.

## Where is the plan / code / test
- **Plan:** `.plans/005_flow_verification_and_staging.md` §3.4 (lines 130–145).
- **Code:** `flow-harness/` (~5,400 lines, every file < 1024 lines; largest is `refund_no_show_deadline.rs` at 720 — the #19 detector).
- **Tests:** inline `#[cfg(test)]` modules per file; 114 tests total. Run with `cd flow-harness && cargo test`.
- **Contract reference:** `docs/escrow_contract_surface.md` §3 (refund truth) + §6 (endpoint surface).

## Reflection — struggling / solved
- **`lib.rs` corruption:** my internal reasoning text leaked into the file content during creation. Solved by overwriting the file completely with the clean 47-line index. Verified clean post-overwrite.
- **`Keypair: Clone` not satisfied:** `solana-sdk`'s `Keypair` doesn't impl `Clone`, but `StagingContext` derives `Clone` (the runner clones it). Solved by wrapping `payer` in `Arc<Keypair>`; `payer_pubkey()` derefs explicitly (`self.payer.as_ref().pubkey()`) to avoid relying on a `Signer for Arc<T>` impl that varies across SDK versions.
- **`Value::is_object` closure signature:** `.filter(Value::is_object)` failed (expected `for<'a> fn(&'a &Value)`). Fixed with `.filter(|v| v.is_object())`.
- **`url` trailing-slash normalization:** `Url::parse("https://host")` round-trips to `https://host/`. The `strip_auth_cookie` test compared against the non-slash string. Fixed the assertion to compare `base_url()` on both sides (robust to normalization) + assert `host_str()`.
- **Auth flow double-registration:** `register_default` included `AuthFlow::new()` and `main.rs` added `AuthFlow::from_env()` → 7 flows with auth duplicated. Fixed by making `register_default` use `AuthFlow::from_env()` and removing the duplicate in `main.rs`.
- **Deprecated `Keypair::from_bytes`:** switched to `Keypair::try_from(seed.as_slice())`.
- **Standalone-crate discovery:** cargo auto-attached `flow-harness` to the root workspace until I added an empty `[workspace]` table to its `Cargo.toml`.

## Remaining work
- **§3.1 staging provisioning (USER MANUAL STEPS):**
  1. `npx wrangler d1 create bethere-db-staging`
  2. `npx wrangler kv namespace create EVENTS_STAGING`
  3. `npx wrangler r2 bucket create bethere-assets-staging`
  4. `npx wrangler d1 migrations apply bethere-db-staging --remote`
  5. Paste returned IDs over `TODO_PROVISION_*` in `worker/wrangler.toml`.
  6. Register staging OAuth redirect URI (`https://bethere-staging.solana-thailand.workers.dev/api/auth/callback`) in Google Cloud.
  7. `bash worker/deploy.sh staging` → `bash worker/scripts/seed-staging.sh`.
- **§3.4 staging-live wiring (every `// TODO(staging-live):` marker):**
  - Deposit: decode/sign/submit TX, poll verification, assert on-chain PDA fields.
  - Refund flows: `attempt_refund_and_assert_revert` (simulate TX, assert escrow code 1/19).
  - Refund checked-in: sign+submit, poll PDA closed.
  - Claim: `perform_claim_mint` (separate NFT program).
  - Need `FLOW_HARNESS_RPC_URL` (Helius devnet) consumed by the submit stubs.
- **§3.5 preflight gate:** `worker/scripts/preflight.sh` (not started; blocked on staging live). Reads `.last-green` mtime (`now - mtime < 1h`); `--force` escape hatch with audit-log entry. Update `worker/deploy.sh` production path to require a green preflight within the last hour.
- **One-time baseline:** run read-only flows against production to confirm current behaviour (Plan 005 §5; GET endpoints only).
- **Mutation check (Plan 005 §4):** intentionally break one staging response, confirm the harness catches it.

## Issues ref
- Plan 005 (`005_flow_verification_and_staging.md`) §3.4 + acceptance criteria §7.
- Divergence #19 (`docs/escrow_contract_surface.md` §4) — the `refund_no_show_deadline` flow is its regression test. Fix #19 part 1 (expose `refund_deadline_ms` + `checked_in` on `DepositStatusResponse`) ✅ landed in `domain`; part 2 (replace the client gate predicate) ⏳ pending. The harness's `assert_divergence_observable` pins the current (detectable) state and its test `divergence_assertion_transitions_when_part_2_ships` documents the hand-off.

## How to dev / test
- **Offline tests (every PR):** `cd flow-harness && cargo test` — 114 tests, <1s, no network/secrets.
- **Compile check:** `cargo check --manifest-path flow-harness/Cargo.toml` (from repo root).
- **CLI (needs env + staging live):**
  - `export FLOW_HARNESS_PAYER_KEYPAIR=/path/to/devnet-keypair.json`
  - `export FLOW_HARNESS_ORGANIZER=<base58>`
  - `export FLOW_HARNESS_ATTENDEE_WALLET=<base58>`
  - `export FLOW_HARNESS_DEPOSIT_MINT=<base58 devnet USDC>`
  - `export FLOW_HARNESS_RPC_URL=https://devnet.helius-rpc.com/?api-key=<key>`
  - `cd flow-harness && cargo run --release -- --worker https://bethere-staging.solana-thailand.workers.dev`
- **Adding a flow:** see `flow-harness/README.md` §"Adding a flow" (6-step checklist).

## Status summary (Plan 005)
- §3.1 staging env: **scaffolded** (wrangler `[env.staging]`, deploy.sh `staging` arg, seed-staging.sh) — pending user provisioning.
- §3.2 contract surface audit: **done** (23/23 variants mapped; the one divergence #19 fixed).
- §3.3 LiteSVM / quasar-svm tests: **superseded** by `bethere-escrow/src/tests/refund.rs`.
- §3.4 E2E harness: **skeleton done this turn**; staging-live wiring pending.
- §3.5 preflight gate: **not started** (blocked on staging live).

---

The `flow-harness/` scaffold is complete and verifiably clean: **114 tests pass, zero clippy errors, the crate is isolated from the root workspace, and the CLI fails fast (exit 2) on missing env** — exactly the §3.4 regression core, ready to wire the moment staging goes live.