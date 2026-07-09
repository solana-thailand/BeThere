# flow-harness

E2E regression harness for **Plan 005 §3.4** — drives the staging worker over HTTP and asserts the on-chain escrow contract surface (deposit, refund, claim, auth) behaves per `docs/escrow_contract_surface.md`.

The harness is the safety mechanism for plans **006 (SIWS)** and **007 (Dioxus mobile)**: the §3.5 preflight gate refuses production deploys unless a green run exists within the last hour.

---

## Status

| Layer | State |
| --- | --- |
| **Two-path refund predicate** (`assertions.rs`) | ✅ Done + fully unit-tested (offline) |
| **PDA derivation** (`context.rs`) | ✅ Done + unit-tested against real program id + seeds |
| **Typed HTTP client + error parsing** (`client.rs`) | ✅ Done; error-envelope parsing unit-tested |
| **Runner + `summary.json` + `.last-green`** (`runner.rs`) | ✅ Done + unit-tested with no-op flows |
| **CLI entry** (`main.rs`) | ✅ Done |
| **Flow `run` bodies — HTTP execution** | ⏳ Staging-gated (`// TODO(staging-live):`) |
| **Transaction signing/submission** | ⏳ Staging-gated (stubs return `HarnessError::Config`) |
| **On-chain PDA existence assertion** (deposit) | ⏳ Staging-gated |
| **Refund attempt + revert assertion** | ⏳ Staging-gated |
| **Claim mint path** | ⏳ Staging-gated (`attempt_mint=false` default) |
| **§3.5 preflight gate** (`worker/scripts/preflight.sh`) | ❌ Not started (blocked on staging live) |

**What "staging-gated" means:** the flow scaffolding, configuration, preconditions, and the regression assertions are all real and `cargo test`-able offline. Only the HTTP/TX *execution* inside `run` bodies requires staging-live. Each such call-site is marked `// TODO(staging-live):` and fails fast with `HarnessError::Config` until §3.1 is provisioned.

---

## Why this harness exists — divergence #19

This is documented in `docs/escrow_contract_surface.md` §3–§4. Summary:

- **On-chain truth** (`bethere-escrow/src/instructions/refund.rs#L72-85`): a refund succeeds iff `clock >= event_end` AND (`checked_in` OR `clock < refund_deadline`).
- **Legacy frontend gate** (`event_refund_window_open`): checks only `now >= event_end` — ignores `refund_deadline` and `checked_in`.
- **Result:** a no-show past `refund_deadline` sees an enabled "Request Refund" CTA, clicks it, signs, and the TX reverts with `RefundDeadlinePassed` (code 19).

Fix #19 has two parts:
1. **Expose the data** — `DepositStatusResponse.refund_deadline_ms` + `.checked_in`. ✅ Landed in `domain`.
2. **Replace the gate predicate.** ⏳ Pending.

The harness's `refund_no_show_deadline` flow is the regression test for this. It pins the corrected predicate (`refund_cta_enabled`), encodes the legacy predicate (`legacy_gate_verdict_at`), and asserts the two **disagree** at the post-deadline point — i.e. the divergence is detectable. Once part 2 ships, the divergence assertion is relaxed to check the corrected gate alone (the test `divergence_assertion_transitions_when_part_2_ships` documents the transition).

---

## Crate layout

```
flow-harness/
├── Cargo.toml              # STANDALONE (not in root workspace — see below)
├── README.md
├── .gitignore              # ignores results/ except .last-green
├── results/                # run artifacts (gitignored)
│   └── .last-green         # sentinel touched on a green run (gate reads mtime)
└── src/
    ├── lib.rs              # module index + re-exports
    ├── main.rs             # CLI entry (clap)
    ├── context.rs          # StagingContext: URLs, keypairs, derived PDAs
    ├── client.rs           # WorkerClient: typed HTTP over worker endpoints
    ├── assertions.rs       # predict_refund_outcome, refund_cta_enabled (regression core)
    ├── runner.rs           # Runner: orchestrator + summary.json + .last-green
    ├── error.rs            # HarnessError + EscrowCode (mirrors program codes)
    └── flows/
        ├── mod.rs          # register_default (6 flows, dependency order)
        ├── deposit.rs
        ├── refund_pre_event_end.rs          # negative: RefundNotYetAllowed (#1)
        ├── refund_post_event_end_checked_in.rs  # positive: refund succeeds post-end
        ├── refund_no_show_deadline.rs       # #19 divergence detector
        ├── claim.rs
        └── auth.rs                          # plan 006 SIWS baseline
```

---

## Why it's a standalone crate

`flow-harness/` is **intentionally NOT** a member of the root workspace (`event-checkin/Cargo.toml`). Its native deps (`solana-sdk`, `spl-token`, `reqwest` with TLS) would pollute:

- the **worker's** `wasm32-unknown-unknown` build, and
- the **domain** crate's dual-target (`x86_64` + `wasm32`) compilation.

Build it from inside the directory:

```sh
cd flow-harness
cargo test      # offline unit suites (no network, no secrets)
cargo run --    # CLI runner (needs env + staging live)
```

---

## Running

### Offline (every PR)

```sh
cd flow-harness && cargo test
```

Runs the staging-independent suites: the refund-window truth table, PDA derivation, runner orchestration, response-shape parsing. Zero network, zero secrets. Fast.

### Full E2E (needs staging live)

Prerequisites (from Plan 005 §3.1):

1. Staging worker deployed: `bash worker/deploy.sh staging`
2. Staging D1 seeded: `bash worker/scripts/seed-staging.sh`

Then:

```sh
cd flow-harness
export FLOW_HARNESS_PAYER_KEYPAIR=/path/to/devnet-keypair.json
export FLOW_HARNESS_ORGANIZER=<base58>
export FLOW_HARNESS_ATTENDEE_WALLET=<base58>
export FLOW_HARNESS_DEPOSIT_MINT=<base8 devnet USDC>
export FLOW_HARNESS_RPC_URL=https://devnet.helius-rpc.com/?api-key=<key>
# Optional: enables the auth flow's logged-in sub-path
export FLOW_HARNESS_ATTENDEE_SESSION="session=eyJ..."
cargo run --release -- --worker https://bethere-staging.solana-thailand.workers.dev
```

---

## Environment variables

| Var | Required | Purpose |
| --- | --- | --- |
| `FLOW_HARNESS_PAYER_KEYPAIR` | yes (CLI) | Path to a funded Solana keypair JSON (devnet faucet) |
| `FLOW_HARNESS_ORGANIZER` | yes | Base58 escrow organizer pubkey (first `EventEscrow` seed) |
| `FLOW_HARNESS_ATTENDEE_WALLET` | yes | Base58 test attendee wallet pubkey |
| `FLOW_HARNESS_DEPOSIT_MINT` | yes | Base58 USDC mint (devnet) |
| `FLOW_HARNESS_RPC_URL` | yes (live) | Helius devnet RPC for TX submission |
| `FLOW_HARNESS_WORKER_URL` | no | Staging URL (default: `https://bethere-staging.solana-thailand.workers.dev`) |
| `FLOW_HARNESS_EVENT_ID` | no | Worker-side event id (default: `flow-test-event`) |
| `FLOW_HARNESS_EVENT_ID_ON_CHAIN` | no | On-chain `u64` event id (default: `1`) |
| `FLOW_HARNESS_ESCROW_PROGRAM_ID` | no | Override the deployed program id |
| `FLOW_HARNESS_ATTENDEE_EMAIL` | no | Override the seeded attendee email |
| `FLOW_HARNESS_ATTENDEE_SESSION` | no | Session cookie; enables the auth flow's logged-in sub-path |

---

## Results layout

```
flow-harness/results/
├── .last-green                       # touched ONLY when every flow passes
└── 2025-01-15T10-30-00Z/
    └── summary.json                  # the run record (RunSummary)
```

- **`.last-green` mtime** is the §3.5 gate signal. The gate checks `now - mtime < 1h`; `--force` bypasses with an audit-log entry.
- **`summary.json`** records per-flow outcome, duration, error kind, and error message — sufficient to triage a failure without re-running.
- **Exit codes:** `0` all passed; `1` one or more flows failed; `2` misconfiguration (no flow executed).

---

## Escrow error codes mirrored

`flow-harness/src/error.rs::EscrowCode` mirrors the subset of `bethere-escrow/src/errors.rs` codes the harness asserts on (see `docs/escrow_contract_surface.md` §2):

| Code | Variant | Flow that asserts it |
| --- | --- | --- |
| 1 | `RefundNotYetAllowed` | `refund_pre_event_end` (negative) |
| 4 | `AlreadyRefunded` | (future) |
| 19 | `RefundDeadlinePassed` | `refund_no_show_deadline` (negative) |
| 22 | `RefundRequiresClose` | (verify item §5) |

---

## Relationship to the plan

| Plan 005 section | Status |
| --- | --- |
| §3.1 Staging worker env | Scaffolded (wrangler.toml `[env.staging]`, deploy.sh, seed-staging.sh) — pending your provisioning |
| §3.2 Contract surface audit | ✅ Done (`docs/escrow_contract_surface.md`) |
| §3.3 LiteSVM / quasar-svm tests | ✅ Superseded by `bethere-escrow/src/tests/refund.rs` |
| §3.4 E2E harness | **This crate** — skeleton done; staging-live wiring pending |
| §3.5 Preflight gate | Not started (blocked on staging live) |

---

## Adding a flow

1. Create `src/flows/<name>.rs` implementing the `Flow` trait.
2. Add a config struct with defaults aligned to `seed-staging.sh`.
3. Put pure logic (preconditions, outcome prediction, gate verdict) in standalone functions with `#[cfg(test)]` truth-table tests.
4. Mark every HTTP/TX call-site with `// TODO(staging-live):` and fail fast with `HarnessError::Config` until staging is live.
5. Register the flow in `src/flows/mod.rs::register_default` (respect dependency order — the summary records flows in registration order).
6. Add the flow to the table in this README.