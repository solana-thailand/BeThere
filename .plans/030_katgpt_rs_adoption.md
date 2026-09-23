# Plan 030: Adopt what transfers from katgpt-rs

**Created:** 2026-09-23 · **Study:** `docs/katgpt_rs_study.md`
**Rule:** process and test hardening can land any time. Nothing here changes
runtime behaviour before RTM#6 (2026-09-27). Product features in §4 need an
owner decision.

## 1. Done 2026-09-23

- [x] Negative-tested gates:
  - the `security_headers_parity` test;
  - `deploy.sh`'s `verify_security_headers`;
  - `forbid(unsafe_code)`;
  - cargo-deny.
- [x] CI as a thin wrapper around documented local commands (`security.yml`).
- [x] `#![forbid(unsafe_code)]` in `domain` and `worker`.

## 2. Ungated, small (S) — process and tests

- [ ] **Numbering gate.**
  - Add `.issues/.highwater`, `.plans/.highwater` and `.handovers/.highwater`.
  - Add `scripts/verify/numbering_gate.py`. It fails on a new duplicate prefix
    and pins the existing ones by membership, each with a reason: issues
    091/095/097, plan 014, handovers 001/124/126.
  - Run it from `issue_ledger.py` or in CI.
- [ ] **Verdict vocabulary.** Extend `scripts/verify/issue_ledger.py` so
  `Status:` must start with one of: OPEN, FIXED-ON-DEVELOP, DEPLOYED, CLOSED,
  CLOSED NEGATIVE, PARKED (reopen trigger: …), DECLINED.
- [ ] **Green-zero guard.** In CI, assert a minimum passed-test count for the
  worker, domain and frontend suites. The count may only move down with a
  commit that says why. A module split that drops a whole test file would then
  turn CI red instead of reading "ok".
- [ ] **Audit `scripts/verify/*.sh`** for katgpt's three gate lies:
  - PASS on a partial run;
  - exit 0 on zero checks;
  - an EXIT trap laundering a `set -e` abort (macOS bash 3.2).
- [ ] **Commit hygiene** in a repo `CLAUDE.md`, about 150 lines, index-style;
  its narratives go to HISTORY:
  - stage named files only;
  - a `Session: <name>, <epoch>` trailer via `-F`;
  - `git fetch` before recording a negative result.
- [ ] `forbid(unsafe_code)` in `frontend-leptos`, if the wasm-bindgen extern
  blocks allow it. It has not been tried yet.

## 3. Ungated, medium (M)

- [ ] **HISTORY.md close protocol.**
  - Closing an issue means one commit that writes a dated HISTORY entry and
    deletes the issue file.
  - Needs an owner OK before purging the existing closed files, since it
    changes where the team looks for history.
- [ ] **Pinned golden vectors.** One `domain` fixture of
  (input → expected bytes) for:
  - escrow PDA derivation and instruction-data encoding;
  - JWT signing;
  - THB/USDC rounding.

  Assert it by value from the worker tests, the escrow tests and
  flow-harness, with a generator binary. This catches the class of
  `[[onchain-struct-offset-drift]]`.
- [ ] **Interleaved A/B timing helper** for CPU-budget claims against the
  free plan's 10 ms/request:
  - median of ratios, per-iteration input, `black_box`, fail on 0 ns;
  - report tail support instead of "p99" when n < 100.

  Feeds `.plans/028`. Native wall time is only a proxy for Workers CPU.
- [ ] **Counting-allocator test** for the check-in hot path (feature-gated,
  proves it is installed). Treat the counts as relative: dlmalloc on wasm
  differs from native.
- [ ] **`.claude/skills/deploy-guard/SKILL.md`**:
  - D1 backup → size budgets → `issue_ledger.py --strict` → `deploy.sh` →
    post-deploy smoke → write-volume check → Status updates;
  - a capped run log.
- [ ] **Toolchain pin.** Add `rust-toolchain.toml`, listing `targets`
  explicitly (the katgpt wasm32 trap), plus a weekly `RUSTUP_TOOLCHAIN=stable`
  rot lane. **After RTM#6**, because it can change builds.

## 4. Owner decisions (product)

- [ ] **Verifiable lucky draw**, from `katgpt-device-verify` fair_roll: commit,
  then reveal with a Solana blockhash, re-runnable in the browser. About 1.5 d.
- [ ] **Tamper-evident check-in log:** Merkle root at event close plus an
  inclusion proof on the ticket. About 2 d.
- [ ] **LtHash checksum** on the deposits/credits ledger with a nightly drift
  alert. About 1 d.
- [ ] Whether to adopt the BOUNDARY.md drift-ledger discipline and the
  second-model AGREE/REVISE review for escrow and money-path plans.
