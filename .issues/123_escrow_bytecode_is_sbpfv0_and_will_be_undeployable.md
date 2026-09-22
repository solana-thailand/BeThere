# 123 — The escrow program is SBPFv0, so after SIMD-0500 it can no longer be deployed or upgraded

**Status:** open, and **further off than this issue first said** — see
"What the clusters actually say" below. The door SIMD-0500 will close is one the
network has not opened yet: SBPFv3 deployment is disabled on mainnet, devnet
**and** testnet, so a v3 build today is an artifact no cluster would accept.
Nothing already running is affected. Watch with
`bash scripts/check_sbpf_v3_gate.sh`; act when it says ACTIVATED.
**Found:** 2026-09-19, from the Anza post the owner sent
(<https://www.anza.xyz/blog/migrating-solana-programs-to-sbpfv3>).
**Severity:** medium now, blocking later — no live defect, no money at risk.

## Evidence (read from chain and from disk, 2026-09-19)

| what | value |
|---|---|
| deployed devnet program `C6HDeZES…` | `e_flags = 0` → **SBPFv0** |
| local build `bethere-escrow/target/deploy/bethere_escrow.so` | `e_flags = 0` → **SBPFv0** |
| `solana-cli` / `cargo-build-sbf` | 3.1.10 (Agave) |
| `platform-tools` | **v1.52** |
| `--arch` anywhere in the repo | none — `scripts/e2e/test_escrow_surfpool.sh:57` is a plain `cargo build-sbf` |

`e_flags` is bytes `0x30..0x34` of the ELF header; `0` is v0. There is no
`readelf` on this machine, so it was read with a four-line Python struct unpack.

## What the clusters actually say (read 2026-09-19, `solana feature status`)

The first version of this issue was written from the Anza post and inferred the
urgency. Reading the gate instead:

| cluster | `BUwGLeF3Lxyfv1J1wY8biFHBB2hrk2QhbNftQf3VV3cC` — *SIMD-0178/0179/0189, enable deployment and execution of SBPFv3* |
|---|---|
| mainnet | **inactive** |
| devnet  | **inactive** |
| testnet | **inactive** |

Also inactive, and both on placeholder `TestFeature…` keys rather than real ones:
SIMD-0161 (*disables execution of SBPFv0*) and its re-enable counterpart. So v0
**execution** is not being taken away either.

That reorders the whole issue. SIMD-0500 refuses v0/v1/v2 *deployments*; it
cannot sensibly activate before the gate that permits v3 deployments, or nothing
would be deployable at all. Until that gate flips:

- building with `--arch v3` produces something **no cluster will accept**, so
  doing the migration early does not put us ahead — it puts us on an artifact we
  cannot deploy;
- upgrading platform-tools to v1.56+ is a machine-level change that every escrow
  script and the #084 e2e suite runs against, spent to buy nothing today;
- the deployed devnet program and any future upgrade of it are both fine.

`scripts/check_sbpf_v3_gate.sh` is the trigger: it reads all three clusters and
exits 10 when any of them opens. It exits **1**, loudly, if the feature key is no
longer listed — a grep that finds nothing reads exactly like "still closed", and
that false calm is the failure mode worth guarding. Both branches were run
before committing: the real key prints "closed", a key that cannot exist prints
the loud not-found and exits 1.

### Gate watch log

Each line is an actual run of the script, not a restatement of the one above it.

| date | mainnet | devnet | testnet |
|---|---|---|---|
| 2026-09-19 | closed | closed | closed |
| 2026-09-22 | closed | closed | closed |

Until a row reads anything but "closed", the migration plan below stays parked
and there is nothing to do.

## Why it matters

[SIMD-0500](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0500-disable-deployment-of-sbpf-v0-v1-v2.md)
makes the upgradeable loader reject any **new deployment, upgrade, or
finalization** whose ELF is not v3. Anza: *"The feature gate for SIMD-0500 is
planned for activation in Agave v4.4."* Programs already deployed keep executing.

So the cost is not an outage, it is a door closing:

- **Mainnet escrow deployment** (`docs/mainnet_readiness_runbook.md`,
  `ESCROW_PROGRAM_ID_MAINNET` is still the empty fail-loud guard) would be
  **rejected** if attempted after activation with a v0 build.
- **Any devnet upgrade** of `C6HDeZES…` would be rejected too — including one to
  fix a bug found by the #084 e2e suite.
- Finalizing (making immutable) is also refused for v0/v1/v2.

## What the migration needs

Minimums from the Anza post, against what this machine has:

| component | required | here |
|---|---|---|
| `platform-tools` | **v1.56** | v1.52 ❌ |
| `cargo-build-sbf` | **v4.2.0** | 3.1.10 ❌ |
| `solana-define-syscall` | **v3.0.0** | not a direct dep (comes via quasar) |

**Do not pass `--arch v3` to platform-tools older than v1.53** — the post says it
produces incompatible bytecode, and v1.52 is exactly that. So the first step is
the toolchain, not the flag. An outdated `solana-define-syscall` "can brick your
program".

Open question, and the reason this is an issue rather than a patch: the program
is built on **quasar** (pre-release, pinned at `3d6fb0d`) plus `quasar-svm`. The
quasar repo does mention sbpf and `define-syscall`, but whether that pinned rev
produces valid v3 output is unverified. Bumping the pin is itself a deliberate
act — `bethere-escrow/Cargo.toml:20-27` pins exact git revs on purpose.

## Plan when it is picked up

0. **Check the gate first** — `bash scripts/check_sbpf_v3_gate.sh`. Steps 1-6
   are wasted, and the toolchain risk is taken for nothing, while it prints
   "closed".
1. Install an Agave CLI that ships `cargo-build-sbf` ≥ v4.2.0 / platform-tools
   ≥ v1.56. **Machine-level change** — the current 3.1.10 is what every escrow
   script and the #084 e2e suite runs against, so do it knowingly.
2. `cargo build-sbf --arch v3 --tools-version v1.56` (plus `-z defs` to catch
   unresolved symbols, per the post).
3. Verify the artifact: `e_flags` must read **3**, not 0 (`readelf -h` prints
   `CPU Version: 3`; the Python unpack above works without binutils).
4. Re-run the SVM tests (`quasar-svm`), then the devnet e2e (#084, still blocked
   on devnet USDC).
5. Deploy: devnet upgrade first, then mainnet when that is decided.
6. Watch for the behaviour changes the post lists, the dangerous one being that
   **null pointer reads may now succeed instead of aborting**, which turns a
   loud failure into a silent one in unsafe code.

## Related

- `docs/mainnet_readiness_runbook.md` — the mainnet deployment this blocks.
- [084](084_preflight_gate_has_never_been_satisfiable.md) — the e2e suite that
  would have to re-run after a rebuild.
- [075](075_rust_deposit_decoder_stale_length_gate.md) — the worker-side decoders
  are unaffected: they read account bytes, not bytecode.
