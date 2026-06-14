# Goal: Sanity check auto_prompt on BeThere

Smallest possible auto_prompt test. Confirms the fork, custom v6 prompt, and `.plans/` chaining all work on this codebase before tackling larger audits.

## Tasks
- [x] Run `cargo build` in `bethere-escrow/` — confirm clean compile
- [x] Run `cargo test` in `bethere-escrow/` — capture pass/fail counts
- [x] If any test fails: fix the root cause, re-run until green
- [x] Report final test counts in a comment, then mark [x]

<!-- Final test counts: 43 passed / 0 failed / 0 ignored / 0 measured.
     Build clean with zero warnings after registering `cfg(kani)` in Cargo.toml check-cfg. -->

## Acceptance Criteria
- [x] `cargo build` clean
- [x] `cargo test` green (all tests pass)
- [x] Any flaky/failing tests fixed at root cause (not skipped)

## Constraints
- No skipping tests to make them pass
- Production grade: no mock, no TODO, no unwrap()
- Fix diagnostics before marking done