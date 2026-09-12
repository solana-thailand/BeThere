# 076 — e2e shell helpers are copy-pasted across scripts, with no shared library

**Status:** open
**Found:** 2026-09-13, while validating the #072 ownership assertion
**Severity:** low (maintenance / correctness-drift risk, not a live defect)

## What

`scripts/e2e/` holds 9 shell scripts. None of them source a shared library —
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
helpers in place and add a sourcing convention that only two of nine scripts
follow. The real fix is a `scripts/e2e/lib/common.sh` that all nine source,
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
