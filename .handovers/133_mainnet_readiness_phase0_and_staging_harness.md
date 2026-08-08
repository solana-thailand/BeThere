# Handover 133 — Mainnet Readiness Phase 0 & Staging Flow Harness Hardening

## 0. TL;DR

Successfully executed Phase 0 of the [Mainnet Readiness Runbook](file:///Users/ozone/event-checkin/docs/mainnet_readiness_runbook.md). Proved on-chain escrow bytecode on Devnet matches local source byte-for-byte (`sha256: 26380992e22a4784e40857dec77b708bdc0c1899b65cef2ce562c57e11900d80`). Restored `wasm-bindgen-cli v0.2.118` toolchain alignment, provisioned Cloudflare Workers Staging secrets, deployed `bethere-staging` (`https://bethere-staging.solana-thailand.workers.dev`), and updated `flow-harness` to forward session tokens.

---

## 1. Changes Made

### 1. On-Chain Devnet Verification (Phase 0.3 Gate)
- Ran [scripts/verify_devnet_binary.sh](file:///Users/ozone/event-checkin/scripts/verify_devnet_binary.sh) against deployed Devnet program `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`.
- **Verdict**: `✓ MATCH — on-chain bytecode == pinned source.`
- Updated [docs/audit_submission.md](file:///Users/ozone/event-checkin/docs/audit_submission.md) with the verified SHA256 string.
- Checked off Phase 0.3 & 0.4 in [docs/mainnet_readiness_runbook.md](file:///Users/ozone/event-checkin/docs/mainnet_readiness_runbook.md).

### 2. Staging Infrastructure & Deployment
- Reinstalled pinned `wasm-bindgen-cli v0.2.118` to resolve schema mismatch (`0.2.126` vs `0.2.118` locked in `Cargo.lock`).
- Provisioned all 10 Cloudflare secrets (`GOOGLE_CLIENT_ID`, `JWT_SECRET`, etc.) for `[env.staging]` via `wrangler secret put --env staging`.
- Deployed `bethere-staging` (`https://bethere-staging.solana-thailand.workers.dev`) with `DEV_MODE=1`. Verified `/api/health` returns `status: ok` and D1 database connection (`events: 1`, `attendees: 1`).

### 3. `flow-harness` & Staging Seed Hardening
- Updated [flow-harness/src/main.rs](file:///Users/ozone/event-checkin/flow-harness/src/main.rs) to read `FLOW_HARNESS_ATTENDEE_SESSION` from environment and attach session cookies (`client.with_auth_cookie(...)`).
- Updated [worker/scripts/seed-staging.sh](file:///Users/ozone/event-checkin/worker/scripts/seed-staging.sh) to include `--env staging` on wrangler D1 commands and configure future `EVENT_END_MS` for deposit testing.

---

## 2. Verification

```bash
# 1. Devnet binary verification
bash scripts/verify_devnet_binary.sh
# Result: ✓ MATCH (sha256: 26380992e22a4784e40857dec77b708bdc0c1899b65cef2ce562c57e11900d80)

# 2. Flow harness unit test suite
cargo test -p flow-harness
# Result: 126 passed; 0 failed

# 3. Staging health check
curl -s https://bethere-staging.solana-thailand.workers.dev/api/health
# Result: {"status":"ok","cluster":"devnet","dev_mode":true,"d1":{"connected":true...}}
```

---

## 3. Reflections & Struggles / Solved

- **Staging 503 Outage**: `bethere-staging` returned 503 because Cloudflare Worker secrets are non-inheritable across environments in Wrangler. Piping `.dev.vars` to `wrangler secret put --env staging` inside `worker/` resolved it immediately.
- **Wasm Bindgen Drift**: The local `wasm-bindgen-cli` had drifted to `0.2.126`. Following Handover 132's rule, reinstalling `0.2.118` restored clean reproducible builds.
- **Session Auth in Harness**: `flow-harness` required session cookies for identity-gated endpoints (`/api/deposit/usdc`). Wiring `FLOW_HARNESS_ATTENDEE_SESSION` into `main.rs` enabled authenticated test calls.

---

## 4. Remaining Work

1. **Phase 1 Lifecycle Execution**: Run complete on-chain lifecycle test (`test_escrow_devnet.sh`) on Devnet to capture signatures.
2. **Multi-Repo Quiz Pipeline**: Connect `viral` (Whisper ASR audio transcripts) + `solana-learn` (curriculum taxonomy) to auto-generate post-event quizzes for BeThere D1 and sync cNFT badges to `solana-thailand-genesis` reputation ranks.
3. **Phase 4 Mainnet Gates**: Submit external security audit package ([docs/audit_submission.md](file:///Users/ozone/event-checkin/docs/audit_submission.md)) and transition upgrade authority key to Squads Multisig.

---

## 5. How to Dev / Test

```bash
# Run flow harness against staging
FLOW_HARNESS_ORGANIZER="$(solana address)" \
FLOW_HARNESS_ATTENDEE_WALLET="$(solana address --keypair /tmp/bethere-escrow-e2e-attendee.json)" \
FLOW_HARNESS_PAYER_KEYPAIR="$HOME/.config/solana/id.json" \
FLOW_HARNESS_DEPOSIT_MINT="4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU" \
FLOW_HARNESS_WORKER_URL="https://bethere-staging.solana-thailand.workers.dev" \
FLOW_HARNESS_ATTENDEE_SESSION="event_checkin_token=dev-token" \
cargo run --package flow-harness
```
