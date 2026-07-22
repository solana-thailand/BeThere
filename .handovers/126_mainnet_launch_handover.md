# Handover 126 — Mainnet Launch Path (post-checkpoint, post-branch-cleanup)

> Continuation thread for the **mainnet launch** of the BeThere escrow/deposit flow.
> Authored after the mandatory plan-checkbox checkpoint + branch-management cleanup.

---

## 1. What happened (this session)

Three workstreams converged on one conclusion: **the mainnet path is no longer a code problem — it's an ops + review problem.**

### 1.1 Divergence #19 (the only `[code]` mainnet gate) — already DONE
Investigation of "fix divergence #19 part 2" (the recommended next action from the prior thread) revealed it is **already complete and end-to-end on `develop`** — the prior summary was stale.

- **Code fix** (commit `f280d5d`, ancestor of `develop`): two-path refund predicate in `frontend-leptos/src/pages/deposit/types.rs` (`event_refund_window_open(event_end_ms, refund_deadline_ms, checked_in)`).
- **Both structs** carry the fields:
  - `domain/src/models/deposit.rs:172` — `refund_deadline_ms`, `checked_in`
  - `frontend-leptos/src/api/deposit.rs:50,56` — mirror type
- **Worker populates both** with fail-safe defaults: `worker/src/handlers/deposit/usdc/handlers.rs` — `refund_deadline_ms = event_end_ms + refund_deadline_hours*3.6M` (0 if unconfigured); `checked_in` from `sheets::get_attendee_by_id(...).is_checked_in()`, fetched **only when a deposit exists**.
- **Both callers wired**: `already_deposited.rs:82`, `mod.rs:353` pass all 3 args.

No work performed — nothing to do.

### 1.2 Mandatory plan-checkbox checkpoint
User flagged unchecked items in `.plans/004_refund_button_event_end_gate.md`. Per instruction: verify every `- [ ]` against actual code, flip completed ones, don't re-execute, create `.docs/` only when ALL are `[x]`.

**Result:**
- **Plan 004 (3 items)** — §3.3 verified-builds items **BLOCKED on Docker** (no container runtime on host: `docker`/`colima`/`podman`/`orb` all absent; `solana-verify` 0.5.0 installed but its deterministic build requires a container). Devnet program `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` has no verify PDA metadata. **Cannot mark `[x]` honestly** — would need `brew install colima docker` (deferred to user decision).
- **5 genuinely code-complete items flipped** in `.plans/005_flow_verification_and_staging.md` (commit `f56e414`, later relocated — see §1.3):
  - §3.1 `[env.staging]` block in `worker/wrangler.toml` (name/D1/KV/R2/DEV_MODE=1/test times all present)
  - §3.1 `deploy.sh staging` arg
  - §3.1 `seed-staging.sh` (seeds `flow-test-event`, staging-scoped)
  - §3.1 `.env.staging.example`
  - §3.5 preflight gate (`preflight.sh` + blocking `run_preflight_gate` in `deploy.sh`, opt-in `BETHERE_PREFLIGHT_GATE=1`, `--force` audit-logged bypass)
- **Plan 008 (17 items)** — feature code exists (`domain/src/pr_pack.rs`, `worker/src/handlers/events/{pr_pack,post_event_registration}.rs`, visibility fix `74bc43d` on HEAD) **but integration test files do NOT** (`worker/tests/{event_summary_flow,post_event_registration,pr_pack}.rs` all absent); rest are manual/ops ACs. **No flippable items.**
- **Plans 006/007/014/016** — manual/ops/future (SIWS staging testing, Dioxus mobile not started, katgpt-rs migration, manual click-throughs). **No flippable items.**
- **`.docs/` NOT created** — precondition (ALL `[x]`) unmet: **103 items remain unchecked**, several externally blocked. Per "don't lie, don't overclaim."

### 1.3 Branch-management cleanup
First reconciliation commit (`f56e414`) landed on `feature/event_recap` — but that's the **superseded** branch (PR #16 CLOSED; content cherry-picked into #19/#20). Stranded.

**Executed the relocation:**
1. `git checkout chore/plan005-staging-scaffold` (PR #19)
2. `git cherry-pick f56e414` → applied cleanly as `52f32a0` (no conflict; flips §3.1/§3.5, disjoint from PR #19's existing §8 reconciliation `63c6a02`)
3. `git checkout feature/event_recap && git reset --hard 70f3cb4` (dropped stranded commit; matches `origin/feature/event_recap`; `f56e414` preserved in reflog)
4. `git push origin chore/plan005-staging-scaffold` → `7402535..52f32a0`, **PR #19 updated**
5. **Held** on deleting `feature/event_recap` — waiting for PRs #19 + #20 to merge.
6. Switched working context from dead `feature/event_recap` to `develop`.

---

## 2. Where is the plan / code / test

| Artifact | Location | State |
|---|---|---|
| Divergence #19 fix | `develop` @ `f280d5d` | ✅ shipped |
| Plan-005 reconciliation | `chore/plan005-staging-scaffold` @ `52f32a0` (PR #19) | ✅ pushed |
| Cluster toggle (mainnet lever) | `worker/src/solana_escrow/mod.rs:47` `usdc_mint()` reads `SOLANA_CLUSTER` | ✅ exists — mainnet is a **secret**, not code |
| Devnet program (live, tested) | `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` | ✅ 31/31 E2E, 39 SVM tests + 16 Kani harnesses |
| Flow-harness scaffold | `flow-harness/` (only on `feature/event_recap`, PR #19/#20) | 🟡 scaffold w/ staging-live **stubs** (`deposit.rs:343` "not yet wired") |

### PR state (authoritative, via `gh`)
| PR | Branch → base | State | Mergeable | CI |
|---|---|---|---|---|
| #16 | `feature/event_recap` → develop | **CLOSED** | — | — |
| #19 | `chore/plan005-staging-scaffold` → develop | OPEN | ✅ CLEAN | ✅ check+clippy+test SUCCESS |
| #20 | `feat/plan008-event-lifecycle` → develop | OPEN | ✅ CLEAN | ✅ check+clippy+test SUCCESS |

Both split PRs are **mergeable in any order, 0 file overlap, green**.

---

## 3. Reflection — struggling / solved

**Solved:**
- **Stale-summary trap.** The prior thread's summary said "divergence #19 part 2 pending." The codebase proved it shipped (`f280d5d` on `develop`). Resisted inventing work; verified before acting.
- **grep-tool blind spot.** `event_refund_window_open` wasn't found by content grep in `frontend-leptos/` despite existing — worked around with `git show branch:path` + `git grep`. Don't trust a single search method.
- **Stranded-commit recovery.** Caught that `f56e414` landed on a dead branch; cleanly relocated via cherry-pick + reset, zero conflict, reflog-safe.
- **Honest checkpoint.** Resisted pressure to create `.docs/` or flip blocked items; only flipped 5 genuinely-verified items; correctly diagnosed Docker block + stubbed harness + missing integration tests.

**Struggled with / open nuance:**
- **Plan 005 §3.4 "Implement E2E harness"** is ambiguous: the crate *exists* (scaffold) but flows are stubs gated behind live staging. Treated as `[ ]` (not done) — conservative. If a future thread considers this done, verify `flow-harness/src/flows/deposit.rs:326` "Staging-live stubs" block + absence of `results/.last-green`.
- **`feature/event_recap` deletion timing.** Held off even though it's dead weight — it's the source lineage of the cherry-picks. Safe to delete only after **both** #19 and #20 merge.

---

## 4. Remaining work

### ① MERGE PR #19 + #20 *(user/team decision — the immediate unblock)*
Both clean/green, 0 file overlap, any order. Lands: staging scaffold + event-lifecycle + visibility fix + both doc reconciliations onto `develop`.

### ② Submit escrow to EXTERNAL security audit *(user action — the long-pole)*
Internal audit done (15 findings, 12 fixed). External review (Audit Arena) **not yet submitted**. **Start now** — submit-and-wait. Program: `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`.

### ③ Mainnet ops *(user action)* — in order:
1. Generate mainnet program keypair, fund ~1.5 SOL rent, deploy `bethere_escrow.so` to `mainnet-beta`.
2. Make `ESCROW_PROGRAM_ID` cluster-aware (alongside the existing `usdc_mint()` toggle).
3. Set prod secrets: `SOLANA_CLUSTER=mainnet-beta`, mainnet `HELIUS_API_KEY`/`HELIUS_RPC_URL`.
4. Helius webhook for mainnet TX monitoring + Squads multi-sig upgrade authority + monitoring/alerting.
5. **⚠️ Resolve Wrangler 4.x canary bug (error 10013)** — `deploy.sh` falls back to PUT API → **100% traffic immediately, no canary**. Decide pre-go-live: feature-flag the cluster toggle, OR accept full-cut with fast rollback.

### ④ Autonomous agent work (no merge needed)
- **Draft the canary-deploy mitigation runbook** for the Wrangler bug — a real pre-mainnet deliverable.
- After #19/#20 merge: delete `feature/event_recap`; re-flip now-verifiable plan-005/008 checkboxes.
- **Do NOT** look for mainnet code work — it's done. Don't re-fix divergence #19.

---

## 5. Issues ref / plan status

- **Plan 004 §3.3** (3 verified-build items): **BLOCKED on Docker**. Install `colima`/`docker` or OrbStack before `solana-verify build` can earn the explorer badge. Note: Quasar's non-standard build may need a custom `--base-image`.
- **Plan 005**: 14 items still `[ ]` (E2E harness stubs, no `flow-harness.yml` CI, no mutation test, live-staging ACs need a deployed staging env).
- **Plan 006** (SIWS, 18 items): manual/ops (staging testing, monitor 48h, flag flips).
- **Plan 007** (Dioxus mobile, 43 items): **future work, not started** — needs scoping decision.
- **Plan 008** (17 items): feature code done; integration test files absent + manual ACs.
- **Plan 014** (5 items): katgpt-rs paradigm migration — research.
- **Plan 016** (3 items): manual click-throughs pending.
- **Issue 001**: deposit commitment & refund system — escrow layer implemented & devnet-validated.

**Total: 103 unchecked items across plans 004–016**, mostly ops/manual/future, **not** stale code.

---

## 6. How to dev / test

### Re-verify divergence #19 is on develop
```sh
git checkout develop
git merge-base --is-ancestor f280d5d develop && echo "✓ #19 fix on develop"
git show develop:frontend-leptos/src/pages/deposit/types.rs | grep -A6 "pub fn event_refund_window_open"
# Expect 3-arg signature: (event_end_ms, refund_deadline_ms, checked_in)
```

### Confirm PR mergeability before merge
```sh
gh pr view 19 --json mergeable,mergeStateStatus,statusCheckRollup
gh pr view 20 --json mergeable,mergeStateStatus,statusCheckRollup
# Both should report MERGEABLE / CLEAN / SUCCESS
```

### After merge — clean up
```sh
git checkout develop && git pull
git branch -D feature/event_recap                 # local
git push origin --delete feature/event_recap      # remote (PR #16 already closed)
```

### If unblocking plan 004 §3.3 (verified builds)
```sh
brew install colima docker && colima start
cargo install solana-verify --force --locked   # already 0.5.0
solana-verify build                            # may need custom --base-image for Quasar
```

### Canary-bug reproduction (Wrangler 4.x)
```sh
wrangler deploy --dry-run --env staging 2>&1 | grep -i 10013   # confirms fallback to PUT API
```

---

## 7. Branch state at handoff

```
develop @ fc69740 (= origin/develop)         ← current working context, clean
├── chore/plan005-staging-scaffold @ 52f32a0 (= origin)  ← PR #19 OPEN, includes §3.1/§3.5 reconciliation
├── feat/plan008-event-lifecycle    @ 4f3c072 (= origin)  ← PR #20 OPEN
└── feature/event_recap             @ 70f3cb4 (= origin)  ← PR #16 CLOSED, dead weight, deletable post-merge
```

All branches in sync with remotes (0/0). Working tree clean.

---

**TL;DR for the next thread:** Mainnet is an ops problem now, not a code problem. Merge #19+#20, submit the external audit (long-pole), then run the mainnet-ops checklist. The one autonomous deliverable available is the Wrangler canary-bug mitigation runbook. Don't write mainnet code — it's done.
````
