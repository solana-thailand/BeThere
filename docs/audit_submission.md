# BeThere Escrow — Solana Audit Arena Submission Package

**Document type**: Audit brief / submission package
**Prepared**: 2026-07-21
**Submitter**: BeThere internal team
**Audit target**: BeThere escrow on-chain program (`bethere-escrow/`)
**Framework**: Quasar (Anchor-compatible Solana framework)

---

## 1. Audit Target Summary

| Item | Value |
|------|-------|
| Program name | `bethere-escrow` |
| Framework | [Quasar](https://github.com/blueshift-gg/quasar) (`quasar-lang` + `quasar-spl`) |
| Crate | `bethere-escrow` v0.1.0 (edition 2021, `cdylib` + `lib`) |
| Devnet program ID | `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` |
| Pinned git commit | `83b47e22cdcdb721c059cddebe43366598233e20` |
| Pinned branch | `develop` |
| Commit date | 2026-07-21 11:32:03 +0700 |
| Deployed `.so` size | 89,856 bytes (`bethere-escrow/target/deploy/bethere_escrow.so`) |
| Verified Devnet SHA256 | `26380992e22a4784e40857dec77b708bdc0c1899b65cef2ce562c57e11900d80` (Phase 0.3 verified) |
| Build size (last Quasar record) | 89,856 bytes (`bethere-escrow/target/.quasar-last-size`) |
| Source root | `bethere-escrow/src/` |
| Existing internal audit | `docs/security_audit.md` (725 lines, SEC-001..SEC-015) |

> **Note on size**: prior internal notes referenced 99,104 bytes; the actual deployed artifact and the on-disk `.quasar-last-size` record both show **89,856 bytes**. Treat 89,856 as authoritative.

The program declares its ID at `bethere-escrow/src/lib.rs#L17`:
```event-checkin/bethere-escrow/src/lib.rs#L17
declare_id!("C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T");
```

---

## 2. Scope of Review

The audit target is the **on-chain escrow program only**. There are 9 user-facing instructions plus one introspection helper module. Discriminators are 1-byte instruction selectors assigned by Quasar in declaration order in `lib.rs`; account discriminators are explicit (`EventEscrow = 1`, `AttendeeDeposit = 2`).

### 2.1 Instructions in scope

| # | Discriminator | Instruction | Description | Signer / Authority |
|---|---|---|---|---|
| 0 | `0x00` | `create_event` | Initialize `EventEscrow` PDA + vault token account. Sets organizer, deposit mint/amount, event_end, refund_deadline. Validates `deposit_amount != 0`, `event_end > now`, `refund_deadline > event_end`. | Organizer (signer) |
| 1 | `0x01` | `deposit` | Attendee deposits `deposit_amount` of `deposit_mint` into vault via `transfer_checked`. Creates `AttendeeDeposit` PDA. Requires `event_escrow.is_active`. | Attendee (signer) |
| 2 | `0x02` | `mark_checked_in` | Sets `attendee_deposit.checked_in = true`. Time-guarded: rejects if `now > event_end` (SEC-011 fix). | Organizer (signer, `has_one`) |
| 3 | `0x03` | `refund` | Refunds deposit from vault → attendee via `transfer_checked` (PDA-signed). Requires `now >= event_end`. For no-shows, also requires `now < refund_deadline`. Requires `close_deposit` to be paired in the same transaction (introspection). | Attendee (signer) |
| 4 | `0x04` | `claim_forfeited` | Organizer claims a single no-show's deposit. Requires `now >= refund_deadline`, `!checked_in`, `!refunded`. Transfer via `transfer_checked`. | Organizer (signer, `has_one`) |
| 5 | `0x05` | `close_event` | Closes `EventEscrow` PDA + vault token account, drains lamports to organizer. Requires `!is_active`, accounting invariant holds, AND `vault.amount() == 0` (SEC-013 fix). | Organizer (signer, `has_one`) |
| 6 | `0x06` | `deactivate_event` | Sets `event_escrow.is_active = false`. One-way transition. Refunds still allowed. | Organizer (signer, `has_one`) |
| 7 | `0x07` | `close_deposit` | Closes `AttendeeDeposit` PDA, drains lamports to closer. Two paths: (a) self-close by attendee after `refunded == true`; (b) GC close by anyone after parent `EventEscrow` is closed (`data_len == 0`). | Any signer (with path-dependent guards) |
| 8 | `0x08` | `rollover_deposit` | Atomically moves attendee's deposit from source `EventEscrow` vault → target `EventEscrow` vault. Requires same organizer, same mint, same deposit_amount on both. Source attendee must be `checked_in && !refunded`. Target event must be `is_active`. | Attendee (signer) |

### 2.2 Helper module (not a dispatchable instruction)

| File | Purpose |
|------|---------|
| `bethere-escrow/src/instructions/introspection.rs` | Helpers for scanning the Instructions sysvar: `validate_instruction_sysvar`, `has_close_deposit_for`, `has_rollover_deposit_for`, `require_close_deposit_follows_refund`. ⚠️ **Auditor note**: as of the pinned commit, these public functions are **not called** by any instruction handler — `refund.rs` inlines its own equivalent `require_close_deposit_pair`. `has_rollover_deposit_for` is unused. This is dead code in the current build and should be either wired up or removed. |

### 2.3 Account discriminators

| Type | Discriminator | Seeds | Source |
|------|---|---|---|
| `EventEscrow` | `1` | `["escrow", organizer, event_id(u64 LE)]` | `state.rs` `#[account(discriminator = 1)]` |
| `AttendeeDeposit` | `2` | `["deposit", event, attendee]` | `state.rs` `#[account(discriminator = 2)]` |

### 2.4 Event discriminators (emitted via `emit!`)

| Discriminator | Event |
|---|---|
| 0 | `EventCreated` |
| 1 | `Deposited` |
| 2 | `CheckedIn` |
| 3 | `Refunded` |
| 4 | `ForfeitedClaimed` |
| 5 | `EventClosed` |
| 6 | `EventDeactivated` |
| 7 | `DepositClosed` |
| 8 | `DepositRolledOver` |

Source: `bethere-escrow/src/events.rs`.

---

## 3. Trust Model & Invariants

### 3.1 Trust model (from `docs/security_audit.md` Appendix)

Originally the program encoded a trust assumption that **the organizer fairly checks in attendees** (SEC-001: refund required `checked_in == true`). This trust was removed in Phase 3 — refunds no longer depend on `checked_in`. The escrow now functions as a **pure no-show deterrent**: any attendee can reclaim their deposit after `event_end` (within the no-show refund window `[event_end, refund_deadline)`), regardless of organizer behavior. The organizer cannot rug-pull attendees.

The `checked_in` field is now purely an analytics / NFT-eligibility signal; it does **not** gate fund access. The only organizer-controlled fund flow is `claim_forfeited`, which is restricted to (a) post-`refund_deadline`, (b) `checked_in == false`, (c) `refunded == false` attendees.

### 3.2 Core security invariants the auditor must verify

The following invariants are claimed by the implementer and must be checked against the code at the pinned commit:

| # | Invariant | Where enforced (claim) |
|---|---|---|
| I1 | **No organizer fund theft.** Attendees can always refund after `event_end` regardless of `checked_in`. | `refund.rs` `validate_and_update()` |
| I2 | **Refund window is well-ordered.** `event_end < refund_deadline` (enforced at `create_event`). For no-shows, refund window is `[event_end, refund_deadline)`. For checked-in attendees, window is `[event_end, ∞)`. | `create_event.rs`, `refund.rs` |
| I3 | **No refund / claim race.** Once `now >= refund_deadline`, `refund` is blocked for no-shows (returns `RefundDeadlinePassed`) and `claim_forfeited` becomes eligible. Windows are non-overlapping for no-show deposits. | `refund.rs`, `claim_forfeited.rs` |
| I4 | **Checked-in attendees cannot be forfeited.** `claim_forfeited` requires `!attendee_deposit.checked_in()`. | `claim_forfeited.rs` |
| I5 | **Accounting conservation.** `total_deposited == total_refunded + total_forfeited + vault_balance` at all times. `close_event` enforces both the accounting identity AND `vault.amount() == 0`. | `deposit.rs`, `refund.rs`, `claim_forfeited.rs`, `rollover_deposit.rs`, `close_event.rs` |
| I6 | **No double-spend.** `refunded` flag is set before transfer and re-checked via `!attendee_deposit.refunded()` constraint. | `refund.rs`, `claim_forfeited.rs`, `rollover_deposit.rs` |
| I7 | **Escrow-critical fields immutable after `create_event`.** Only `total_deposited`, `total_refunded`, `total_forfeited`, `is_active` (one-way `true→false`) are mutable post-init. Organizer, deposit_mint, deposit_amount, event_end, refund_deadline, vault, event_id are write-once. | No `set_inner` calls in mutating instructions; enforced by absence |
| I8 | **PDA uniqueness.** `EventEscrow` unique per `(organizer, event_id)`; `AttendeeDeposit` unique per `(event, attendee)`. No cross-user PDA sharing. | `state.rs` seed declarations |
| I9 | **No duplicate mutable accounts (8/9).** Defense-in-depth helper `require_distinct` invoked at the top of 8 of 9 instruction handlers. ⚠️ **`refund.rs` omits it** (comment: "removed to free BPF call depth frames for instruction introspection"). Distinctness in `refund` is enforced indirectly via PDA seeds rather than explicit check. | `instructions/mod.rs`, all instruction files except `refund.rs` |
| I10 | **Atomic rent reclamation.** `refund` must be paired with `close_deposit` in the same transaction (introspection-enforced), preventing deposit-PDA rent leaks. | `refund.rs::require_close_deposit_pair` |
| I11 | **Token-2022 compatibility.** All 3 token transfer sites use `transfer_checked(mint, decimals)`, not `transfer`. | `deposit.rs`, `refund.rs`, `claim_forfeited.rs`, `rollover_deposit.rs` |
| I12 | **Close atomicity.** `close_event` and `close_deposit` use Quasar's `close(dest)` which zeroes data + drains lamports atomically; no in-TX revival possible. | `close_event.rs`, `close_deposit.rs` |

---

## 4. Known Findings Status Table

For each SEC-001..SEC-015, the status below is **verified by reading the actual code at commit `83b47e22`**, not by trusting the audit doc. "On-chain scope" = the audit target. Several findings are out of scope for this on-chain audit (backend-only or frontend-only) and are listed as such.

| ID | Severity | Title | Scope | Status (verified) | Evidence |
|----|----------|-------|-------|---|---|
| SEC-001 | 🔴 Critical | Check-in gate enables fund theft | On-chain | ✅ **Remediated** | `refund.rs` accounts block has **no** `checked_in` constraint. Only timing checks (`event_end`, `refund_deadline`) gate refund. |
| SEC-002 | 🟠 High | Escrow-critical fields mutable after on-chain init | **Backend only (out of scope)** | 🟢 N/A for on-chain audit | Affects `worker/src/event_store.rs::update_event`. On-chain immutability is independently confirmed (see SEC-008). |
| SEC-003 | 🟡 Medium | No maximum deposit cap | **Worker layer (defense-in-depth)** | ✅ **Remediated** | No on-chain cap exists (intentional — see note below). Cap is enforced in the worker at **3 independent sites** in `worker/src/event_store/write.rs`: L166-170 (`create_event`), L386-393 (`update_event`), L657-664 (`apply_update`) — all check `deposit_amount_usdc > MAX_DEPOSIT_USDC` where `MAX_DEPOSIT_USDC = 1_000_000_000` (= $1,000 USDC at 6 decimals). **On-chain cap is unnecessary as defense** because `deposit.rs::create_deposit` L88 reads `amount = self.event_escrow.deposit_amount()` — the attendee never supplies the amount; only the organizer-configured (and now worker-capped) value can ever be transferred. An attendee therefore cannot exploit an uncapped `deposit_amount`. Worker cap exists to prevent organizer mis-configuration (self-inflicted). |
| SEC-004 | 🟡 Medium | Archive doesn't deactivate on-chain escrow | **Backend only (out of scope)** | 🟢 N/A for on-chain audit | Affects `worker/src/event_store.rs::archive_event`. On-chain deactivation is independent (`deactivate_event` instruction). |
| SEC-005 | 🟡 Medium | Explorer links hardcoded to devnet | **Frontend only (out of scope)** | 🟢 N/A | `frontend-leptos/` only; no on-chain impact. |
| SEC-006 | 🟢 Low | Duplicate Merkle Tree field in form | **Frontend only (out of scope)** | 🟢 N/A | `frontend-leptos/` only. |
| SEC-007 | 🟢 Info | Worker cannot manipulate funds | Architectural | ✅ **Confirmed safe** | Non-custodial design: worker builds unsigned TXs, all signing via wallet adapter. Verified by inspecting `worker/src/solana_escrow/tx_builders/*` — they construct instructions, do not sign. |
| SEC-008 | 🟢 Info | On-chain escrow fields immutable after creation | On-chain | ✅ **Confirmed safe** | `set_inner` only called in `create_event.rs` and `deposit.rs`/`rollover_deposit.rs` (for `AttendeeDeposit` init). No mutating instruction calls `set_inner`. Counter updates use field assignment only. |
| SEC-009 | 🟡 Medium | `transfer()` vs `transfer_checked()` | On-chain | ✅ **Remediated** | All 4 transfer sites use `transfer_checked(...)`: `deposit.rs` L65-70 (attendee→vault), `refund.rs` `transfer_usdc` (vault→attendee), `claim_forfeited.rs` `validate_and_claim` (vault→organizer), `rollover_deposit.rs` (source_vault→target_vault). |
| SEC-010 | 🟡 Medium | AttendeeDeposit PDAs never closed (rent leak) | On-chain | ✅ **Remediated** | `close_deposit` instruction exists (discriminator 7) with self-close + GC paths. TX builder at `worker/src/solana_escrow/tx_builders/close.rs`. Additionally, `refund.rs` enforces pairing with `close_deposit` in same TX via sysvar introspection. |
| SEC-011 | 🟡 Medium | No `event_end` guard on `mark_checked_in` | On-chain | ✅ **Remediated** | `mark_checked_in.rs` reads `Clock` sysvar and returns `EscrowError::EventEnded` if `clock.unix_timestamp > event_end`. |
| SEC-012 | 🟠 High | Refund has no `refund_deadline` upper bound (race with `claim_forfeited`) | On-chain | ✅ **Remediated** | `refund.rs::validate_and_update` returns `RefundDeadlinePassed` if `!checked_in && now >= refund_deadline`. Kani proof `refund_window_exclusive` formally verifies the window is `[event_end, refund_deadline)` for no-shows. |
| SEC-013 | 🟡 Medium | Vault griefing via external USDC airdrop blocks `close_event` | On-chain | ✅ **Remediated** | `close_event.rs::close_event` checks both the accounting invariant AND `if self.vault.amount() != 0 { return Err(VaultNotEmpty) }` before invoking `close_account`. Kani proof `vault_griefing_detected` covers this. |
| SEC-014 | 🟡 Medium | No wallet network detection (wrong cluster TX signing) | **Frontend only (out of scope)** | 🟢 N/A | JS/Rust frontend logic; no on-chain impact. |
| SEC-015 | ℹ️ Info | Stranded lamports on token accounts | On-chain | ✅ **Confirmed safe** | All close paths use `close(dest)` (PDA) or `close_account` CPI (vault), draining all lamports. No account is left rent-exempt-alive with excess. |

### 4.1 Verified status tally

**On-chain scope findings (10):**
- ✅ Remediated: **7** (SEC-001, SEC-003 [worker layer], SEC-009, SEC-010, SEC-011, SEC-012, SEC-013)
- ✅ Confirmed safe: **3** (SEC-007 architectural, SEC-008, SEC-015)
- ❌ Open: **0**

**Out of scope (5):** SEC-002, SEC-004 (backend), SEC-005, SEC-006, SEC-014 (frontend).

### 4.2 Discrepancies between `docs/security_audit.md` and the actual code

The auditor should be aware of the following honest discrepancies found while preparing this brief:

1. **SEC-003 (deposit cap) — RESOLVED, was a documentation/scoping ambiguity, not a missing fix.** Initial review (above table v1) reported this as OPEN based on searches of `bethere-escrow/src/` and `worker/src/handlers/`. On deeper review, the cap **IS implemented in the worker layer** at `worker/src/event_store/write.rs` in three independent sites: `create_event` (L166), `update_event` (L386), and `apply_update` (L657) — all enforcing `MAX_DEPOSIT_USDC = 1_000_000_000` ($1,000 USDC). The on-chain program intentionally has no cap, which is safe because `deposit.rs::create_deposit` (L88) derives the transfer amount from `event_escrow.deposit_amount()` rather than from any attendee-supplied input — an attendee cannot deposit more than the worker-capped, organizer-configured value. The original audit doc's "✅ Fixed (Phase 1)" claim is therefore accurate in substance (the cap exists and is enforced) but ambiguous in scope (it is a worker-layer guard, not an on-chain one). No remediation work is required; the worker-layer placement is the correct architectural choice (config validation belongs at the API boundary, not in the program).

2. **Deploy size** — audit doc / prior notes reference 99,104 bytes; the actual deployed `.so` and `.quasar-last-size` both show **89,856 bytes**. Treat 89,856 as the authoritative figure.

3. **`introspection.rs` dead code** — the public helpers `require_close_deposit_follows_refund`, `has_close_deposit_for`, and `has_rollover_deposit_for` are defined in `instructions/introspection.rs` but **not called** from any instruction. `refund.rs` has its own inlined `require_close_deposit_pair` that duplicates the logic of `require_close_deposit_follows_refund`. `has_rollover_deposit_for` is fully unused — i.e., no introspection-based guard on `rollover_deposit`. This is not a vulnerability per se, but it is misleading code that the auditor should flag.

4. **Audit doc text inconsistencies** — `docs/security_audit.md` Finding Details section lists several findings (SEC-002, SEC-003, SEC-004, SEC-005, SEC-006, SEC-009, SEC-011) with body text "Status: Open" while the summary table at the top marks them "✅ Fixed". The summary table reflects later remediation; the body sections were not updated. The status column in §4 above supersedes both.

5. **`rollover_deposit` not in original audit** — `rollover_deposit` (discriminator 8) was added after the original audit and is **not covered** by SEC-001..SEC-015. The auditor should review it as a net-new instruction (atomic vault-to-vault transfer with `checked_in` precondition; see `rollover_deposit.rs`).

6. **`refund.rs` omits `require_distinct` defense-in-depth check — verified ACCEPTABLE, document as a justified design decision.** `docs/security_audit.md` (SF checklist cross-ref, program-side row 8; security-question #6) claims `require_distinct` is called in "all 9 handlers". This is **factually false**: `refund.rs::validate_and_update` explicitly does NOT call it. The code comment reads: *"require_distinct and validate_version removed to free BPF call depth frames for instruction introspection. PDA seeds guarantee distinct addresses and correct account discriminators."* Verified: 8/9 instructions call it (`create_event`, `deposit`, `mark_checked_in`, `claim_forfeited`, `close_event`, `deactivate_event`, `close_deposit`, `rollover_deposit`); `refund` is the sole exception. **Independent verification of the PDA-seed argument (this review):** the `Refund` accounts struct uses Quasar `address =` constraints that resolve and verify PDA addresses *before* `validate_and_update` runs — `event_escrow` is `address = EventEscrow::seeds(organizer, event_id)`, `attendee_deposit` is `address = AttendeeDeposit::seeds(event_escrow, attendee)` with `attendee` being the `Signer`. Combined with the `Signer` uniqueness guarantee and the `constraints(*vault.address() == *event_escrow.vault())` / `constraints(*attendee_deposit.attendee() == *attendee.address())` framework checks, no two of the five mutable accounts (`attendee`, `event_escrow`, `attendee_deposit`, `attendee_ta`, `vault`) can collide at runtime. The defense-in-depth claim in the audit doc is therefore **inaccurate as stated but the underlying security property holds**. Recommended remediation: update `docs/security_audit.md` to read "8/9 handlers" with the BPF-call-depth rationale, so the auditor is not surprised. No code change is needed.

---

## 5. Out of Scope

The auditor should **NOT** spend time on the following; they are not part of the on-chain audit target:

| Area | Path | Reason |
|------|------|--------|
| Cloudflare Worker backend | `worker/src/` (incl. `worker/src/solana_escrow/tx_builders/`, `worker/src/handlers/deposit.rs`, `worker/src/event_store.rs`) | Off-chain. TX builders construct unsigned instructions; they are non-custodial and do not affect on-chain security. (Mentioned only as evidence for SEC-007.) |
| Frontend (Leptos) | `frontend-leptos/` | Off-chain UI. SEC-005, SEC-006, SEC-014 are frontend-only. |
| D1 / Cloudflare infra | `worker/`, `wrangler.toml` | Off-chain storage / infra. |
| Domain models | `domain/` | Off-chain types. |
| E2E / flow harness | `e2e/`, `flow-harness/`, `tests/` (root level) | Integration harnesses outside the program crate. |
| Backend SQL / D1 raw_sql pattern | `worker/` | Off-chain; documented as accepted risk in `docs/security_audit.md`. |
| Program upgrade authority / multisig | — | Operational concern, not code. |

**In scope**: every `.rs` file under `bethere-escrow/src/`. Specifically: `lib.rs`, `state.rs`, `errors.rs`, `events.rs`, `kani.rs`, and `instructions/{create_event,deposit,mark_checked_in,refund,claim_forfeited,close_event,deactivate_event,close_deposit,rollover_deposit,introspection}.rs`.

---

## 6. Build & Reproduction Instructions

### 6.1 Environment

- Rust toolchain: per `bethere-escrow/Cargo.toml` (`edition = "2021"`).
- Quasar framework: pulled from git (`quasar-lang`, `quasar-spl`, `quasar-svm` for tests).
- `Cargo.toml` declares `check-cfg` for `target_os = "solana"` and `kani`.
- IDL generator and Solana bytecode emitter are bundled into the Quasar CLI / build scripts.

### 6.2 Build the program

From the repo root:

```sh
cd bethere-escrow
cargo build --release
```

Quasar's build config (`Quasar.toml`) declares `toolchain.type = "solana"` and `testing.framework = "quasarsvm-rust"`, so the Quasar build pipeline emits the SBF `.so` to `bethere-escrow/target/deploy/bethere_escrow.so`. The deploy artifact and keypair already exist on disk at the pinned commit (`bethere_escrow.so` = 89,856 bytes, `bethere_escrow-keypair.json`).

### 6.3 Regenerate the IDL

The IDL is emitted as part of the Quasar build to:

```
bethere-escrow/target/idl/bethere_escrow.idl.json
```

A Rust client crate is also generated to `bethere-escrow/target/client/rust/bethere-escrow-client/` (declared as a dev-dependency in `Cargo.toml`). To force regeneration, run a clean build:

```sh
cd bethere-escrow
cargo clean
cargo build
```

### 6.4 Run the SVM test suite

Tests live in `bethere-escrow/src/tests/`:

| Test file | Covers |
|---|---|
| `create_event.rs` | `create_event` validation, PDA derivation |
| `deposit.rs` | `deposit` happy path + edge cases |
| `checkin.rs` | `mark_checked_in` + SEC-011 `event_end` guard |
| `refund.rs` | `refund` window logic + SEC-001 (no check-in gate) + SEC-012 (refund_deadline) |
| `close.rs` | `close_event` + `close_deposit` + SEC-013 (vault grief) |
| `introspection.rs` | `refund` ↔ `close_deposit` TX pairing |
| `rollover.rs`, `rollover_flow.rs` | `rollover_deposit` end-to-end |
| `reference_oracle.rs` | State reference helpers |
| `mod.rs` | Test harness entrypoint |

Run the full suite:

```sh
cd bethere-escrow
cargo test --quiet
```

Per project conventions, set `RUST_LOG=info` for logs.

### 6.5 Run Kani formal verification

Kani harnesses live in `bethere-escrow/src/kani.rs` (16 proofs, ~490 lines). They verify pure arithmetic properties of `create_event`, `deposit`, `refund`, `claim_forfeited`, `close_event` (conservation, monotonicity, refund-window exclusivity, vault-grief detection, no overflow).

Requires Kani 0.67.0+:

```sh
cargo install kani-verifier --version 0.67.0
cargo kani setup
cd bethere-escrow
cargo kani --quiet
```

Expected result: all 16 harnesses pass (729 total checks).

### 6.6 Inspect the live devnet program

The program is deployed to devnet at `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`. The auditor can verify bytecode / read accounts via:

```sh
solana config set --url devnet
solana program show C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T
solana account <PDA>   # for any EventEscrow / AttendeeDeposit / vault
```

---

## 7. Submission Checklist (Artifacts to Upload)

| # | Artifact | Path / Source | Notes |
|---|---|---|---|
| 1 | Source tarball at pinned commit | `git archive --format=tar.gz 83b47e22cdcdb721c059cddebe43366598233e20 -o bethere-escrow-83b47e2.tar.gz` (run from repo root, scope to `bethere-escrow/`) | Pinned to commit on `develop` branch |
| 2 | Compiled `.so` | `bethere-escrow/target/deploy/bethere_escrow.so` | 89,856 bytes, SHA-256 should match devnet deployment |
| 3 | IDL JSON | `bethere-escrow/target/idl/bethere_escrow.idl.json` | 15 KB |
| 4 | This submission brief | `docs/audit_submission.md` | You are reading it |
| 5 | (Reference) Internal audit doc | `docs/security_audit.md` | Optional reading; includes Kani + SF checklist cross-ref |
| 6 | (Reference) Kani proofs | `bethere-escrow/src/kani.rs` | 16 harnesses |
| 7 | (Reference) Test suite | `bethere-escrow/src/tests/` | 10 files |

### 7.1 Pre-submission self-check (run before submitting)

```sh
cd /Users/ozone/event-checkin

# 1. Confirm pinned commit + branch
git rev-parse HEAD
# expected: 83b47e22cdcdb721c059cddebe43366598233e20
git rev-parse --abbrev-ref HEAD
# expected: develop

# 2. Confirm program ID is declared in source
rg 'declare_id!' bethere-escrow/src/lib.rs
# expected: C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T

# 3. Confirm deployed size matches
wc -c < bethere-escrow/target/deploy/bethere_escrow.so
# expected: 89856

# 4. Confirm IDL exists
test -f bethere-escrow/target/idl/bethere_escrow.idl.json && echo OK

# 5. Confirm all 9 instructions are present in lib.rs dispatch
rg '#\[instruction\(discriminator' bethere-escrow/src/lib.rs | wc -l
# expected: 9
```

### 7.2 Devnet program ID for live inspection

```
C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T
```

Cluster: **devnet** (`https://api.devnet.solana.com`).

---

## 8. Suggested Auditor Focus Areas

Based on the verified state above, the highest-value audit targets are:

1. **`refund.rs` sysvar introspection (`require_close_deposit_pair`)** — non-trivial byte-level parsing of the Instructions sysvar. Verify: (a) bounds checks are sound, (b) cannot be bypassed by crafted TX structure, (c) correctly handles the `current_idx` boundary, (d) the duplicated logic in `introspection.rs` doesn't drift.
2. **`rollover_deposit.rs`** — net-new instruction not covered by SEC-001..SEC-015. Atomic vault-to-vault transfer; verify source/target escrow invariants, `checked_in` precondition, conservation across the two escrow accounts.
3. **`claim_forfeited.rs`** — verify the `!checked_in` guard is airtight and that the `refunded` flag flip is correctly ordered before the CPI.
4. **`close_event.rs`** — verify the dual check (accounting + `vault.amount()`) cannot be TOCTOU'd within the TX and that `close_account` CPI seeds are canonical.
5. **SEC-003 (deposit cap)** — confirm the worker-layer cap (`worker/src/event_store/write.rs:166/386/657`, `MAX_DEPOSIT_USDC = 1_000_000_000`) is reachable on every code path that sets `deposit_amount_usdc` (create, update, and any bulk-import / seed / duplicate paths). Verify that no admin-only or sync endpoint bypasses `apply_update` / `create_event`. The on-chain absence of a cap is acceptable given `deposit.rs` L88 reads the amount from escrow state (not attendee input).
6. **Dead code in `introspection.rs`** — determine whether the unused helpers were intended to be wired up (especially `has_rollover_deposit_for`).
7. **`require_distinct` defense-in-depth helper** — verify coverage. As of the pinned commit: 8/9 instruction handlers call it; `refund.rs` is the explicit exception. Confirm whether the PDA-seed-only distinctness guarantee in `refund` is sufficient, especially given that `refund` has the most mutable accounts of any instruction (attendee, event_escrow, attendee_deposit, attendee_ta, vault, instruction_sysvar).

---

*End of submission package.*