# Handover 111 — Plan 014 Phase 5.3 Deterministic-Not-Stochastic Guard

> Branch: `feature/014_deterministic_lint` → `develop`
> Date: 2026-06-27
> Plan ref: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 5.3

## 1. What Happened

Plan 014 Phase 5.3 — the **deterministic-not-stochastic discipline for monetary
code** — is complete. The plan asked for the discipline to be "encoded as a
clippy lint or `#[deny]`-style test where feasible." Custom clippy lints require
nightly (via `dylint`), so I shipped it as a multi-layer `#[test]`-style guard
that runs on stable and is part of `cargo test --workspace`.

The audit-first pass confirmed the codebase is **already compliant** — no
monetary decision path uses any form of RNG. This is another "audit finds
already-compliant state" result (like Phase 4.3.1 / 4.3.3). The value of the
work is therefore the **regression guard**, not the remediation.

### The discipline

katgpt-rs has a hard "sigmoid, never softmax" rule. The equivalent hard rule
for monetary code: **deterministic, never stochastic**. No RNG in policy
decisions, no probabilistic gates on refunds / claims / deposits.

### Audit findings (what already exists)

Comprehensive scan of `worker/`, `domain/`, and `frontend-leptos/` for any RNG
surface:

| Surface | Finding |
|---|---|
| `rand` crate | **Absent** from every `Cargo.toml` in the workspace |
| `fastrand` crate | Absent |
| `getrandom` crate | Absent |
| `rand_core` / `rand_chacha` / `rand_pcg` / `rand_xorshift` | All absent |
| Direct RNG patterns in worker source | None — `thread_rng`, `OsRng`, `StdRng`, `ChaCha*Rng`, `RngCore`, `from_entropy`, `Math::random`, `getRandomValues`, `gen_range`, `fill_bytes` all absent |
| `Uuid::now_v7()` (7 sites in worker) | **Used only for identifier generation** — claim tokens, lock IDs, API IDs, correlation IDs. Never for decisions. v7 is timestamp-prefixed and monotonic; its random tail is collision-avoidance, not a policy input. Explicitly allowed by the guard. |
| `chrono::Utc::now()` | Wall-clock time, not randomness. Allowed. |
| Deterministic hashes (FNV-1a, BLAKE3, SHA-256, HMAC) | Pure functions of input, not RNG. Allowed. |
| Frontend `init_arrange_puzzle` / `init_right_shuffle` | Already deterministic (reverse + swap if still sorted). Out of worker scope; the frontend has no monetary decision path anyway. |
| `bethere-escrow` `RANDOM_USER` | A constant `Pubkey` in a test, not actual randomness. |

Monetary decision paths spot-checked for deterministic logic:
- `worker/src/handlers/deposit/thb/handlers/refund.rs::mark_refund_handler` — refund decision based on verified status, prior refund state, proof URL presence. Fully deterministic.
- `worker/src/claim/lock.rs::acquire_claim_lock` — lock acquisition via compare-and-set on `lock_id`. Deterministic.
- `worker/src/handlers/deposit/escrow/status.rs` — escrow status from KV/D1 lookup. Deterministic.

### Initial hypothesis (proven wrong)

I started the audit expecting to find at least one stochastic site — perhaps a
"random delay before retry" in a deposit poller, or a " probabilistic jitter"
in a cache TTL. The audit found none. The codebase was written from day one
with the deterministic discipline, even though it was not formally encoded
until this phase.

## 2. Changes (1 new file, 1 doc edit)

### New: `worker/tests/deterministic_monetary_code.rs` (~620 lines, 23 tests)

Three layers of defense, plus 20 self-tests that prove the guard logic works:

**Layer 1 — Dependency guard (`worker_cargo_manifest_has_no_direct_rng_dependency`)**
Asserts `worker/Cargo.toml` declares none of: `rand`, `fastrand`, `getrandom`,
`rand_core`, `rand_chacha`, `rand_pcg`, `rand_xorshift`. Without the
dependency, no monetary module can call into the RNG ecosystem even if a
developer tries. This is the strongest layer because it blocks the most common
regression vector (adding `rand = "0.8"` to Cargo.toml).

The `is_dep_declaration` helper trims defensively and rejects comment lines,
feature-flag arrays, and substring false-positives (`rand_extended` must not
match the `rand` rule; `rand_core` must match `rand_core`, not `rand`).

**Layer 2 — Source-scan guard (`monetary_modules_contain_no_direct_rng_calls`)**
Recursively scans every `.rs` file under the monetary module tree for forbidden
direct RNG patterns. 17 patterns in `FORBIDDEN_RNG_PATTERNS`:

- Crate prefixes: `rand::`, `fastrand::`
- RNG structs: `thread_rng`, `OsRng`, `StdRng`, `SmallRng`, `ChaCha8Rng`,
  `ChaCha12Rng`, `ChaCha20Rng`
- RNG traits: `RngCore`, `CryptoRng`
- RNG seeding: `from_entropy`, `seed_from`
- JS bridge: `Math::random`
- Web Crypto: `getRandomValues` (camelCase WebIDL name), `get_random_values`
  (snake_case Rust binding)
- RNG calls: `gen_range`, `fill_bytes`

Scope (recursively scanned, new files auto-covered):
- `worker/src/claim/`
- `worker/src/solana_escrow/`
- `worker/src/escrow_indexer/`
- `worker/src/handlers/deposit/` (includes `escrow/`, `thb/handlers/`,
  `usdc/`)
- Explicit files: `handlers/claim.rs`, `handlers/escrow_index.rs`,
  `handlers/checkin.rs`, `handlers/register.rs`, `handlers/walkin.rs`,
  `handlers/wallet.rs`

**Layer 3 — Scope sanity (`monetary_module_scope_is_nonempty_and_existing`)**
Catches the silent-regression case where a refactor moves monetary code out of
the scanned directories and the source-scan guard becomes a no-op. Asserts the
scan returns non-empty, contains `claim` / `deposit` / `escrow` files, and has
at least ~10 files. If the module tree is ever reorganised, this test forces a
conscious edit of the scope constants rather than a silent drop in coverage.

**Self-tests (20 tests under `self_tests` module)**
A regression guard that never fires is worthless. The self-tests prove the
guard's pattern logic catches real violations while rejecting deterministic
lookalikes:

- *Positive cases* (must catch): `rand::thread_rng().gen_range(0..100)`,
  `OsRng`, `ChaCha20Rng::from_entropy()`, `js_sys::Math::random()`,
  `web_sys::crypto()?.get_random_values_with_buffer(...)` (snake_case),
  `crypto.getRandomValues` (camelCase WebIDL), `RngCore::Error`,
  Cargo.toml `rand = "0.8"` and `rand = { version = "0.8" }` forms.
- *Negative cases* (must NOT catch): `Uuid::now_v7()`,
  `chrono::Utc::now()`, `blake3::hash(...)`, `sha2::Sha256::digest(...)`,
  FNV-1a offset basis constant `0xcbf29ce484222325`, deterministic-shuffle
  comments, Cargo.toml comment lines, substring false-positives
  (`rand_extended`, `my_rand`, feature-flag arrays).

### Live injection test (manual verification, not committed)

Before committing, I verified the guard actually fires on a real violation:

1. Injected `// TODO: use rand::thread_rng().gen_range(0..100) for fee
   calculation` into `worker/src/handlers/deposit/thb/handlers/refund.rs`.
2. Ran the guard. It failed loudly, catching all three patterns (`rand::`,
   `thread_rng`, `gen_range`) with the exact file:line and the offending line
   contents.
3. Restored the file via backup copy. Confirmed clean via `git diff`.
4. Re-ran the guard. All 23 tests passed.

This is the critical confidence check: a guard that never fires in practice is
untested. The injection test proves the source-scan layer actually walks the
configured directory tree and surfaces violations with actionable diagnostics.

### Edited: `.plans/014_katgpt_rs_paradigm_migration.md`

Phase 5.3 marked `[x]` DONE with the audit outcome and the three-layer guard
design recorded inline.

## 3. Validation Done

| Check | Method | Result |
|---|---|---|
| Guard tests pass | `cargo test -p event-checkin-worker --test deterministic_monetary_code` | ✅ 23/23 (3 guard + 20 self-tests) |
| Workspace check | `cargo check --workspace --all-targets` | ✅ EXIT 0, zero warnings (only pre-existing profile warning) |
| Workspace clippy | `cargo clippy --workspace --all-targets` | ✅ Zero warnings |
| Workspace tests | `cargo test --workspace` | ✅ 308 tests pass (was 285; +23 from this work), 0 failed |
| Live injection | Manual: inject comment with `rand::thread_rng().gen_range`, run, revert | ✅ Guard fires with clear message; restored cleanly |
| Frontend native | `cargo check --all-targets` in `frontend-leptos/` | ✅ EXIT 0 |
| Frontend wasm32 | `cargo check --target wasm32-unknown-unknown` in `frontend-leptos/` | ✅ EXIT 0 |

### Test count growth

- Before: 285 tests (84 domain unit + 9 pod + 144 worker + 15 + 33 misc)
- After: **308 tests** (+23 from this work: 3 guard + 20 self-tests)

## 4. Plan / Code / Test Locations

- **Plan**: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 5.3 (now marked
  DONE with audit outcome recorded inline).
- **Guard test**: `worker/tests/deterministic_monetary_code.rs` (~620 lines,
  23 tests across 4 modules: dependency guard, source-scan guard, scope-sanity
  guard, self-tests).
- **Forbidden RNG patterns**: `FORBIDDEN_RNG_PATTERNS` constant in the test
  file — 17 entries.
- **Forbidden deps**: `FORBIDDEN_DEPS` constant in the test file — 7 entries.
- **Monetary module scope**: `MONETARY_DIRS` (4 directory roots) and
  `MONETARY_FILES` (6 explicit files) constants in the test file.

## 5. Reflections

### What went well

- **The live injection test was the highest-value check.** I almost shipped
  the guard on the strength of the self-tests alone. The injection proved the
  source-scan layer actually walks the tree and fires. The first attempt
  (injecting `rand::thread_rng()` as code) failed at compile time because
  layer 1 (the dependency guard) had already done its job indirectly — `rand`
  isn't in Cargo.toml, so the code can't even compile. This was itself a
  useful confirmation: layer 1 is the strongest defense. The second attempt
  (injecting as a comment) reached layer 2 and fired correctly.
- **The self-tests caught a real gap.** The first version of the guard had
  `getRandomValues` (camelCase WebIDL) in the forbidden list but missed
  `get_random_values` (snake_case Rust binding). The self-test
  `forbidden_patterns_catch_web_crypto_getrandomvalues` originally had an
  unused-variable warning because the test wasn't actually exercising the
  Rust binding form. Fixing the warning surfaced the gap; I added
  `get_random_values` to the forbidden list and tightened the test to verify
  both forms are caught. This is exactly why self-tests on guard logic matter.
- **Audit-first discipline paid off again.** The audit confirmed the
  codebase was already compliant, so the work focused on the regression guard
  rather than remediation. No code was changed in monetary modules — the
  value is purely forward-looking.

### What was harder than expected

- **Designing the scope of "monetary modules".** The codebase doesn't have a
  formal `monetary/` directory; monetary logic is spread across `claim/`,
  `solana_escrow/`, `escrow_indexer/`, `handlers/deposit/`, plus individual
  handlers that generate claim tokens (`checkin.rs`, `register.rs`,
  `walkin.rs`). I had to enumerate these explicitly. The scope-sanity test
  (layer 3) exists to catch the case where this enumeration drifts.
- **Avoiding false positives without dylint.** A true clippy lint could
  inspect the AST and reject only actual RNG call expressions. Source
  scanning is text-based, so I had to be careful with pattern specificity.
  The self-tests document the contract: `Uuid::now_v7`, `chrono::Utc::now`,
  BLAKE3, SHA-256, FNV-1a, and deterministic-shuffle comments must all be
  rejected (i.e. NOT matched) by the forbidden patterns. A future dylint
  migration would be cleaner, but the source-scan approach runs on stable
  and covers the realistic regression vectors.

### Where the result differs from the plan

The plan said "Encode as a clippy lint or `#[deny]`-style test where
feasible." The literal interpretation is a clippy lint. Custom clippy lints
require nightly via `dylint`, which is incompatible with the project's stable
toolchain. I shipped the `#[deny]`-style test interpretation instead — a
plain `cargo test` regression guard. This is feasible on stable, runs in CI,
and covers the same regression vectors a clippy lint would catch.

## 6. Remaining Work

### Plan 014 status after this handover

- **Phase 1** (wire format): ✅ CONCLUDED
- **Phase 2.1** (SSOT audit): 🟡 Open
- **Phase 2.3** (CI dup-check): 🟡 Open
- **Phase 3** (policy traits): ✅ CONCLUDED (trait demoted, entry #7)
- **Phase 4.1** (profile): 🟡 Blocked on infra
- **Phase 4.3** (I/O wins): ✅ CONCLUDED
- **Phase 4.4** (no-SIMD doc): 🟡 Blocked on 4.1
- **Phase 5.3** (deterministic-not-stochastic lint): ✅ **CONCLUDED (this handover)**
- **Phase 5.4** (zero-alloc audit): ✅ CONCLUDED
- **Phase 5.5** (feature-flag discipline): 🟡 Open (mostly satisfied; 5.3 and
  5.4 both ship opt-in test infrastructure)

### What's next (priority order)

1. **Deploy** the commits now sitting on `develop` (5 from handover 109 + 3
   from handover 110 + this handover's commits) via `develop → main → deploy.sh`.
   Operator action.
2. **Phase 2.3** — forward-looking CI dup-check. Buildable now, no infra
   dependency. The Phase 5.3 guard pattern (source-scan + self-tests) is a
   useful template.
3. **Phase 2.1** — SSOT audit. High audit-miss risk per Plan 014 track record
   (5 of 5 audits so far found the plan's premises wrong or already-satisfied).

### Guard re-open preconditions for Phase 5.3

The guard will fire on any of these:

- A developer adds `rand`, `fastrand`, `getrandom`, `rand_core`,
  `rand_chacha`, `rand_pcg`, or `rand_xorshift` to `worker/Cargo.toml`.
- A developer writes any of the 17 forbidden RNG patterns in any file under
  the configured monetary module tree (including comments — by design, even
  a `// TODO: use rand::thread_rng()` comment warrants review).
- The monetary module tree is reorganised without updating the scope
  constants (caught by layer 3, the scope-sanity test).

If a legitimate need ever arises (e.g. a non-deterministic fraud-detection
model is added), the right response is to document the exception in
`.plans/014_negative_results.md` with the reason, then update either
`FORBIDDEN_RNG_PATTERNS` (narrowing the list) or `MONETARY_DIRS` (excluding
the new non-monetary module). Silent removal of the guard is not the right
response.

## 7. Issues Ref

- Plan 014: `.plans/014_katgpt_rs_paradigm_migration.md`
- Plan 014 negative results log: `.plans/014_negative_results.md` (9 entries;
  Phase 5.3 is a positive result, so no entry added)
- Previous handover (Phase 5.4 zero-alloc audit): `.handovers/110_plan_014_phase5_4_zero_alloc_audit.md`
- Phase 4.3 I/O wins handover: `.handovers/109_plan_014_phase4_3_io_wins.md`
- Phase 1 wire format handover: `.handovers/108_plan_014_phase1_wire_format.md`

## 8. How to Dev / Test

### Run the guard

```sh
cargo test -p event-checkin-worker --test deterministic_monetary_code
```

No feature flags required. Runs as part of `cargo test --workspace` by
default. The 23 tests break down as:

- 3 guard tests (dependency, source-scan, scope-sanity)
- 20 self-tests under the `self_tests` module (10 positive cases that must
  catch, 10 negative cases that must NOT catch)

### Add a new forbidden pattern

1. Open `worker/tests/deterministic_monetary_code.rs`.
2. Add the pattern to `FORBIDDEN_RNG_PATTERNS`.
3. Update the doc comment above the constant to explain what the pattern
   matches and why it's forbidden.
4. Add a positive self-test in `self_tests` that proves the pattern catches
   its target.
5. Add a negative self-test that proves the pattern does NOT catch any
   deterministic lookalike.
6. Run the guard. All tests must still pass.

### Add a new monetary module to the scan

1. Open `worker/tests/deterministic_monetary_code.rs`.
2. If the new module is a directory, add its relative path (from `worker/`)
   to `MONETARY_DIRS`.
3. If the new module is a single file, add its relative path to
   `MONETARY_FILES`.
4. Run the scope-sanity test. It will tell you if the new entry is missing
   or mis-typed (panic on missing path).

### Verify the guard fires on a real violation

```sh
# Inject a comment with a forbidden pattern into any monetary file
echo '// TODO: use rand::thread_rng().gen_range(0..100) for fees' \
    >> worker/src/handlers/deposit/thb/handlers/refund.rs

# Run the guard — it must fail with a clear file:line message
cargo test -p event-checkin-worker --test deterministic_monetary_code \
    monetary_modules_contain_no_direct_rng_calls

# Restore the file
git checkout worker/src/handlers/deposit/thb/handlers/refund.rs

# Re-run — all 23 tests must pass
cargo test -p event-checkin-worker --test deterministic_monetary_code
```

### Migrating to dylint (future work)

If the project ever moves to nightly, the source-scan guard can be replaced
with a `dylint` lint that inspects the AST and rejects actual RNG call
expressions (not text patterns). The benefit: zero false positives on
comments, and the lint fires at compile time rather than test time. The
migration path: keep the current test as a fallback, add the dylint lint
as the primary guard, remove the test once dylint is in CI.