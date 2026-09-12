# 076 — e2e shell helpers are copy-pasted across scripts, with no shared library

**Status:** fixed 2026-09-13 (devnet runs still outstanding — see Verification)
**Found:** 2026-09-13, while validating the #072 ownership assertion
**Severity:** low (maintenance / correctness-drift risk, not a live defect)

## What

`scripts/e2e/` holds 8 shell scripts (this was first written as 9, counting a
Python helper). None of them source a shared library —
each one carries its own copy of the same helpers (`info`, `warn`, `fail`,
`skip`, the pass/fail counters, RPC plumbing).

Issue #072 added `assert_escrow_owned_by_program()` to the two rollover
scripts. It is ~30 lines and is **duplicated verbatim**:

- `scripts/e2e/test_rollover_devnet.sh:98`
- `scripts/e2e/test_rollover_full_lifecycle.sh:89`

A prior write-up described this as "a shared
`assert_escrow_owned_by_program()`". That was wrong — it is two copies. This
issue exists so the claim is corrected in the record rather than left to
mislead the next reader.

## Why it matters

This is exactly the shape that produced #072 in the first place. The original
ownership check existed in `test_escrow_devnet.sh` with a `grep "owner:"`
pattern that could never match, and because nothing was shared, the bug had no
single place to be fixed and no single place to be noticed. Compare
`duplicated-state-transition-paths` in the project memory: a guard lands on one
copy and the sibling keeps the defect.

Concretely, the next change to the assertion — a new `solana` CLI field name, a
different RPC error shape, an added check — has to be made twice, and a reviewer
diffing only one script will believe it is done.

## Why it was not fixed inline

Extracting one function is not the fix; it would leave the other ~5 duplicated
helpers in place and add a sourcing convention that only two of the eight
follow. The real fix is a `scripts/e2e/lib/common.sh` that all of them source,
which touches every script in the directory and needs its own verification pass
(each script is currently standalone and several are run by hand on devnet).
That is a deliberate refactor, not a drive-by.

## Proposed fix

1. Add `scripts/e2e/lib/common.sh` with the logging helpers, the pass/fail/skip
   counters and the summary printer.
2. Add `scripts/e2e/lib/solana.sh` with `assert_escrow_owned_by_program()` and
   the other `solana account` parsers, keeping the case-insensitive,
   line-anchored patterns from #072 and their comments.
3. Convert scripts one at a time, running each against devnet before moving on.
4. Keep them ShellCheck-clean under the pinned v0.11.0 gate
   (`scripts/verify/shellcheck_gate.sh`) — `source` of a non-constant path needs
   a `# shellcheck source=` directive or SC1091 fires in CI.

## Verification

The duplication itself:

```sh
rg -n 'assert_escrow_owned_by_program\(\)' scripts/e2e/
```

After the refactor, that should report exactly one definition, and both rollover
scripts should still pass their devnet runs.

## Related

- `.issues/072_e2e_script_dead_assertions.md` — the unreachable `pass` branch
  this helper replaced.
- Memory `cli-output-grep-never-matches`, `duplicated-state-transition-paths`.

---

## Fix — 2026-09-13

### What landed

`scripts/e2e/lib/`, sourced by all eight scripts in the directory:

- **`lib/common.sh`** — the colour constants, the `PASS`/`FAIL`/`SKIP` tallies,
  `pass` / `fail` / `skip` / `info` / `warn` / `section`, and `check_json`.
- **`lib/solana.sh`** — `assert_escrow_owned_by_program` (with the #072
  rationale comment that used to travel beside it) and `sign_and_submit_tx`.

Call sites source them through a `SCRIPT_DIR` derived from `${BASH_SOURCE[0]}`:

```sh
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
source "$SCRIPT_DIR/lib/common.sh"
```

Net **−265 lines**. `rg 'assert_escrow_owned_by_program\(\)' scripts/e2e/` now
reports one definition, as the Verification section above asks, and the same is
true of `check_json` and `sign_and_submit_tx`.

### The extraction was provably behaviour-preserving

Before moving anything, each helper was diffed across every script that carried
a copy. `check_json` was **byte-identical in five** scripts,
`sign_and_submit_tx` in **three**, `assert_escrow_owned_by_program` in **both**
rollover scripts. Moving an identical definition into a sourced file cannot
change behaviour, so the risky part of this refactor was the part that did not
exist. Two scripts needed real edits and are called out below.

### Deliberate behaviour changes

| where | change | why |
|---|---|---|
| `test_full_e2e.sh` | `YELLOW='\031[1;33m'` → `\033[1;33m` | a typo: `\031` is not the ANSI escape, so SKIP/WARN lines emitted a stray control byte and never turned yellow. Unifying the constants fixed it by construction. |
| `test_escrow_surfpool.sh` | `log_pass`/`log_fail`/`log_info`/`log_warn` → `pass`/`fail`/`info`/`warn`; `YELLOW` `0;33` → `1;33` | one naming convention across the directory; the bright yellow is what the other seven already used. |
| `test_lifecycle.sh` | `sign_and_submit()` is now a one-line wrapper over `sign_and_submit_tx` | it hardcoded the default CLI keypair; the shared helper takes the path. The #073 argv comment now lives in one place. |
| `test_lifecycle.sh`, `test_devnet.sh` | `pass`/`fail` now tally into `PASS`/`FAIL` | these scripts previously had uncounted variants. Neither reads the counters, so output and exit status are unchanged. |

### Gate changes

`scripts/verify/shellcheck_gate.sh` now runs `shellcheck -x
--source-path=SCRIPTDIR`. Without `-x` every `source` line raises SC1091 and the
shared variables read as unassigned (which also produced fresh SC2086 findings
on `[ $FAIL -eq 0 ]` in three scripts); `--source-path=SCRIPTDIR` is what makes
the relative `# shellcheck source=` directives resolve against the sourcing
script rather than the gate's CWD. Two findings remain suppressed at the line,
each with a stated reason, per the gate's own policy: `BOLD` is `SC2034`-unused
inside `common.sh` because only consumers read it, and `$RPC_URL` in
`solana.sh` trips `SC2153` against the function's own `rpc_url` local.

Gate: **27 scripts, clean.**

## Verification

### Done

- `scripts/verify/e2e_lib_smoke.sh` (new, wired into the `deploy-scripts` CI
  job). Covers everything in `lib/` that needs no chain and no Worker: the
  tallies, `check_json`'s three outcomes, **both** address-less branches of
  `assert_escrow_owned_by_program`, that the signer script resolves from the
  library, that every script still sources what it calls, and that no script
  has re-grown a local copy of a library helper.
- **The smoke test was negative-controlled in four directions** before being
  trusted — break `skip()`'s tally, make `check_json` always return 0, delete a
  script's `source` line, re-add a local `pass()` — and it failed on each, then
  passed again once reverted. A probe that has only ever been seen to pass is
  not evidence (memory `false-clean-probes-from-shell-aliases`); this one has
  been seen to fail for the right reasons.
- `bash -n` over all ten files; ShellCheck gate clean.

### Not done

**No devnet run.** Step 3 of the proposal above asks for each script to be run
against devnet as it is converted. That is blocked on the same thing blocking
`.issues/072`–`075`: the test wallet
(`9Bz7p4RWdX7eaR4hFUeCc7aSZjDHsie8q1u8imwavkBN`) has no devnet USDC and Circle's
faucet needs a human to clear a captcha.

So what is proven is that the helpers behave identically and that every script
still resolves them — not that a full devnet lifecycle still passes end to end.
Given the byte-identical extraction that is a small gap, but it is a real one,
and the two scripts in the table above (`test_escrow_surfpool.sh`,
`test_lifecycle.sh`) are where it matters most. Run
`bash scripts/e2e/run_all_e2e.sh` once the wallet is funded.
