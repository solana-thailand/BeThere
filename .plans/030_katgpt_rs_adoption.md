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

- [x] **Numbering gate** (2026-09-23).
  - `.issues/.highwater` (144), `.plans/.highwater` (030) and
    `.handovers/.highwater` (137). Allocate with
    `python3 scripts/verify/numbering_gate.py --next .issues` (flock-safe).
  - `scripts/verify/numbering_gate.py` fails on a new duplicate `.md` prefix, a
    pinned group gaining or losing a file, a stale pin, or a highwater below
    the disk. Pinned: plan 014 (9 sibling docs), handovers 001/124/126.
    Issues 091/095/097 were never collisions; each is an `.md` plus its
    `.sql` backfill companion.
  - Runs in CI (`ci.yml`, after its `--self-test`, 10/10). A planted
    `.issues/144_probe_collision.md` turned the live run red.
- [x] **Verdict vocabulary** (2026-09-23). `issue_ledger.py` checks that
  `Status:` opens with one of: open, in progress, fixed on develop, deployed,
  closed, closed negative, parked (a reopen trigger must be named), declined.
  Markup is ignored, so ``fixed on `develop` `` counts.
  - Issues above `LEGACY_MAX = 144` are enforced. The 141 older issues use
    about 30 free-form phrasings (27 already conform); they are reported, not
    failed, and not rewritten.
  - `--vocab` (git-free) and `--self-test` (9/9) run in CI. A planted
    `145_probe.md` turned `--vocab` red. Until issue 145 exists, a clean run
    prints "vacuous", not ✅.
- [ ] **Green-zero guard.** In CI, assert a minimum passed-test count for the
  worker, domain and frontend suites. The count may only move down with a
  commit that says why. A module split that drops a whole test file would then
  turn CI red instead of reading "ok".
- [x] **Audit `scripts/verify/*.sh`** (2026-09-23) for katgpt's three gate lies:
  - PASS on a partial run;
  - exit 0 on zero checks;
  - an EXIT trap laundering a `set -e` abort (macOS bash 3.2).

  Findings, all reproduced on `/bin/bash` 3.2.57:
  - **EXIT traps: clean.** None of the 5 handlers calls `exit`; an abort
    still exits 1 and `exit 3` still exits 3.
  - **Signal resume: fixed.** In `claim_token_window_staging.sh`,
    `trap restore EXIT INT TERM` ran the handler on Ctrl-C/TERM and then
    *resumed*, so the remaining checks ran against the restored row and could
    end in exit 0. INT/TERM now `exit 130`/`143`, and restore still runs once.
  - **False-clean probe: fixed.** `pii_log_probe.sh --grep` exited 0 even
    with leaks, and a missing log fell through `|| echo "(none)"` as clean.
    Now: leak → 1, missing or empty log → 2, clean → 0 (tested all four).
  - **Zero-check exits: none found.** `post_deploy_smoke.sh` and the claim
    window script are linear and exit early on missing fixtures; the size
    budgets exit 0 only on a measured result.
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
