# 123 — The escrow program is SBPFv0, so after SIMD-0500 it can no longer be deployed or upgraded

**Status:** open. Not urgent *today* (SIMD-0500 had not activated on mainnet as of
2026-09-17) and it breaks nothing that is already running, but it blocks the
mainnet escrow deployment and every future program upgrade once it does activate.
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
