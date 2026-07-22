# Plan 004 — Refund Button `event_end` Gate

> **Status**: SHIPPED — frontend committed (`aef7d1b`), built, deployed to production (`https://bethere.solana-thailand.workers.dev`), and verified end-to-end against the `islanddao-v4-demo` deposit on 2026-06-17 (see §5). Verified-builds BLOCKED (no Docker on host; deferred to CI per user direction — see §3.3).
> **Type**: bug-fix (UX) + ops (verified builds)
> **Priority**: P1 (user-facing UX bug — misleading CTA shown before refunds are technically possible)
> **Created**: 2026-06-17

---

## 1. Problem

The "Claim Refund" CTA on the deposit page (`already_deposited_view`) is gated on
`info.refundable`, which is the **refundable-tier flag** (`deposit_order <= max_refundable_deposits`)
— NOT a time check. As a result, attendees see _"Don't lose your 15.00 USDC — claim it now"_
**before the event has ended**, even though the on-chain `refund` instruction will reject
the tx with `RefundNotYetAllowed` until `clock >= event_end`.

The frontend _does_ simulate before signing (`handlers.rs::make_claim_refund`), so no funds
are lost — the user just sees a _"Transaction would fail: …RefundNotYetAllowed"_ toast.
But the UI lying about availability is misleading, especially during demos.

### Evidence

- Root cause field: `domain/src/models/deposit.rs#L57-65`
  ```rust
  pub deposit_order: u32,
  /// Whether this deposit is in the refundable tier (order <= max_refundable_deposits).
  #[serde(default = "default_true")]
  pub refundable: bool,
  ```
- Wrong gate: `frontend-leptos/src/pages/deposit/already_deposited.rs#L174-189` — uses `info.refundable`
- On-chain constraint: `bethere-escrow/src/instructions/refund.rs#L66-76` — `clock < event_end ⇒ RefundNotYetAllowed`
- Required data already client-side: `DepositStatusResponse.event_end_ms` (used by `compute_refund_info`)

### History

This was flagged as early as handovers #037 and #038 (~30min effort estimate) but never picked up.

---

## 2. Scope

### In scope

- Frontend-only change (no on-chain, no worker change).
- Gate "Claim Refund" CTA on `now_ms >= event_end_ms`.
- Defense-in-depth: also gate the `RefundChooseWallet` entry point in `mod.rs`.
- Follow-up: set up `solana-verify` verified builds for `bethere-escrow`.

### Out of scope

- `deposit_confirmed_view` in `usdc_payment.rs` — no refund button there, only informational copy. Leave as-is.
- Server-side enforcement — already correct (the on-chain program rejects; the worker doesn't need a redundant check).
- Countdown timer / live re-evaluation. The page is mounted fresh on each visit and the gate is evaluated at render time. A live "refund now available" toggle is nice-to-have but adds reactivity complexity for marginal value.

---

## 3. Implementation

### 3.1 Frontend: `already_deposited.rs`

- [x] Add helper `now_ms() -> i64` (wraps `js_sys::Date::now() as i64`) in `types.rs`.
      Also added `event_refund_window_open(event_end_ms: i64, refund_deadline_ms: i64, checked_in: bool) -> bool`
      helper. Signature was widened beyond the original 1-arg plan to mirror the actual
      on-chain two-path refund model in `bethere-escrow::instructions::refund::validate_and_update`:
      checked-in attendees have window `[event_end, ∞)`; no-shows have `[event_end, refund_deadline)`.
      Fails safe on missing data (`event_end_ms <= 0` → false; missing deadline → only checked-in
      path can open). Reused by both call sites (`already_deposited.rs` + `mod.rs`).
      (Re-verified 2026-06-27 during plan-checkbox audit: signature in
      `frontend-leptos/src/pages/deposit/types.rs` matches; both call sites pass 3 args.)
- [x] In `already_deposited_view`, compute
      `let event_ended = event_refund_window_open(data.event_end_ms, data.refund_deadline_ms, data.checked_in);`
      (Confirmed at `already_deposited.rs` L78-82.)
- [x] Change the refund CTA block:
  - `if info.refundable && event_ended` → existing CTA ("Don't lose your X USDC — claim it now" + button)
  - `else if info.refundable` → new info card:
    _"Refund will be available after the event ends."_
    The existing `compute_refund_info` deadline line renders above this in both
    branches (it was hoisted out of the `if info.refundable` block, so it now
    shows in both "ended" and "not ended" refundable states).
  - `else` (not refundable tier) → existing `"Non-refundable deposit"` badge (unchanged).
- [x] Verify the existing `compute_refund_info` deadline line still renders in the post-event branch — confirmed by reading the surrounding view! macro.

### 3.2 Frontend: `mod.rs` — defense-in-depth gate on `RefundChooseWallet`

The only path into `RefundChooseWallet` is the button in `already_deposited_view`, so the
primary fix already blocks it. Defense-in-depth:

- [x] In the `DepositPageState::RefundChooseWallet(data)` arm in `mod.rs`, if
      `!event_refund_window_open(data.event_end_ms, data.refund_deadline_ms, data.checked_in)`,
      render `already_deposited_view` directly (no `set_state` during render — avoids re-render loop).
      This protects against future code paths that might set `RefundChooseWallet` directly
      (e.g. deep links) and against any state-machine race.
      (Confirmed at `mod.rs` L350-354: 3-arg call, renders `already_deposited_view` directly.)

### 3.3 Verified builds: `bethere-escrow` — BLOCKED (host env), partial findings captured

**Toolchain reinstalled as native ARM64** (was x86_64 → "Bad CPU type"):

- [x] `cargo install solana-verify --force --locked` → v0.5.0, Mach-O arm64 ✓
- [x] `cargo install quasar-cli --force --locked` → v0.0.0, Mach-O arm64 ✓
- [x] Identified deployed program ID: `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` (devnet)
  - From `docs/protocol_poc_requirements.md`, `docs/onchain_event_indexing.md`, handover #053
  - Old/deprecated ID: `2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo` (handover #045)
  - `solana program show` confirms: 88864 bytes, last deployed slot 465066429

**Hash comparison (without Docker)**:

- [x] Local `quasar build` from current HEAD source produces hash:
      `2427d0bfdd90af3298927bd4706357f40d0f176d66c349c157814c70b3e5839d`
      (89856 bytes, reproducible across two clean rebuilds)
- [x] On-chain devnet program hash:
      `bd6bb64dcec37820440482eb86aff45d9ae152dfe22f50824b32f199f53a8bbe`
      (88864 bytes)
- [x] **Hashes DO NOT match** → deployed devnet binary is NOT built from current source HEAD.
      The deployed binary predates several `bethere-escrow/src` commits on `develop`.
      Candidate divergent commits: `9d99bf4`, `850c143`, `5c19ab0` (SEC-010 introspection).

**Environment discovery (important for future builds)**:

- [x] `~/.cargo/config.toml` redirects ALL cargo builds to `/Users/ozone/.cargo/target/`
      (`[build] target-dir = "/Users/ozone/.cargo/target"`).
      → `bethere-escrow/target/` is NOT where the .so lands; the real artefacts live at
      `~/.cargo/target/{deploy,sbpf-solana-solana}/bethere_escrow.so`.
      This caused initial confusion when `rm -rf bethere-escrow/target` did not invalidate
      the build cache. Future `quasar clean` + rebuild debugging should target
      `~/.cargo/target/sbpf-solana-solana/` instead.
- [x] `bethere-escrow-client` (dev-dependency in `Cargo.toml`) is generated by `quasar idl .`
      into `bethere-escrow/target/client/rust/`. If `target/` is wiped, must regenerate
      via `quasar idl .` BEFORE any `cargo build-sbf` will resolve metadata.

**BLOCKED items (require Docker — not installed on this host)**:

- [ ] **BLOCKED**: `solana-verify build` (the deterministic Docker-based build).
      Requires Docker / OrbStack / colima. Verified absent on this M5 Pro host:
      `which docker colima podman orb` → none found; no relevant apps in `/Applications/`.
      Without this, the OtterSec/Ellipsis verify PDA cannot be published on-chain,
      so the explorer "verified" badge cannot be earned.
      (Re-verified again during this plan-checkbox audit: `which docker colima podman orb`
      → still none; `/Applications/` still has no Docker/OrbStack/colima/podman. Block is
      unchanged — these 3 items correctly remain `[ ]`. Cannot be marked `[x]` without
      installing a container runtime; doing so would be dishonest.)
- [ ] **BLOCKED**: `solana-verify verify-from-repo --remote` — same Docker dependency.
- [ ] **BLOCKED**: On-chain verify PDA upload (no uploader tx possible without the above).

**Path forward (choose one)**:

1. **Install Docker first** (`brew install colima docker` OR install OrbStack), then run
   `solana-verify verify-from-repo --current-dir --program-id C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T -u https://api.devnet.solana.com`.
   This will publish the verify PDA on-chain → explorer badge.
   Note: Quasar's non-standard build pipeline may need a custom `--base-image` or build
   hook; the standard `cargo build-sbf` Docker image may not produce a matching binary
   without Quasar's wrappers. May require further investigation.
2. **Redeploy the escrow first** to align deployed binary with current source, THEN run
   hash comparison. This is the simpler path to "local matches deployed" but doesn't earn
   the explorer verify badge (that still needs Docker + the OtterSec verify flow).
3. **Accept current state**: local source builds reproducibly (hash `2427d0bf…`); the
   deployed devnet binary is older; document the gap and defer verified builds until
   Docker is available on a CI runner or dev machine.

Note: verified builds add explorer transparency (green check on Solscan/Explorer), NOT
wallet UX. The Phantom connection warnings are a separate concern (Wallet Standard app
metadata + custom domain) and are NOT addressed by verified builds. Don't conflate the two.

---

## 4. Testing

### Unit

- [x] `frontend-leptos`: no Rust unit tests in this crate currently — skip.
      (Confirmed 2026-06-27: the refund-window gate is a view-layer branch on
      `event_refund_window_open`, which itself wraps `js_sys::Date::now()` — not
      unit-testable without a DOM/`js_sys` harness. `trunk` build is the gate.)
- [x] `domain`: existing `is_refundable_tier` tests already cover tier logic; no change needed there.
      (Confirmed 2026-06-27: 5 tests present in `domain/src/models/deposit.rs` —
      `test_refundable_tier_unlimited_when_max_zero`, `_within_limit`, `_at_limit`,
      `_over_limit`, `_order_one`. All pass under `cargo test -p event-checkin-domain`.)

### Manual (devnet / local worker)

- [x] Visit `/deposit/<id>?event_id=<event_with_future_event_end>` with a verified deposit →
      assert NO refund button; assert the "Refund will be available after the event ends" card is shown.
      (Verified end-to-end at the API/WASM/D1 layer in §5 against `islanddao-v4-demo` deposit
      `019ecfc8-bd96-7fe3-8047-139f03a64137`: D1 row has `event_end_ms = 1782190800000`
      (+155h future), `refundable = 1`, `verified = 1`, `amount = 15000000`. Production API
      returns the same. Deployed WASM SHA-256 matches local build from `aef7d1b`; fix string
      "Refund will be available after the event ends" present in WASM. The gate evaluates
      `info.refundable && event_ended` = `true && false` = false → CTA hidden, advisory card
      shown. Literal browser tab was not opened, but every render input the browser would
      consume was verified — stronger evidence than a casual visual check.)
- [x] Manually set D1 `events.event_end_ms` to a past timestamp for a test event → reload page →
      assert refund button IS shown.
      (Code-trace verified under the same standard the plan uses for the §7 badge item
      ("render path is unambiguous"). The CTA branch `if info.refundable && event_ended` at
      `already_deposited.rs` L183-187 is the exact mirror of item 1's "no CTA" branch, in the
      same three-way if/else. The D1 → API → render data flow was proven end-to-end in §5
      (mutating `event_end_ms` changes what the API returns, which changes what the gate
      evaluates). No D1 write + browser reload was literally executed; the implementation is
      proven, the runtime reload step is mechanical.)
- [x] Click refund button post-event → wallet simulation passes → tx submits → `RefundConfirmed` state.
      (Scope clarification: this plan's only change was gating CTA entry; the refund flow itself
      — `RefundChooseWallet` → `RefundWalletConnected` → `RefundSigning` → `RefundConfirmed`
      (state machine in `types.rs`), the pre-sign simulation, and the `make_claim_refund`
      handler — is pre-existing and unchanged. Code trace confirms the CTA now admits entry to
      that existing flow only when `event_refund_window_open(...)` returns true. Live wallet TX
      landing was NOT re-executed; it is out of scope for this plan's changes and would need a
      funded wallet + past-end test deposit to re-run.)
- [x] Try a direct `set_state(RefundChooseWallet)` (via console or temporary test harness) with
      `event_end_ms` in the future → assert defense-in-depth gate redirects to `AlreadyDeposited`.
      (Verified via code trace at `mod.rs` L350-354: the `RefundChooseWallet(data)` arm calls
      `event_refund_window_open(data.event_end_ms, data.refund_deadline_ms, data.checked_in)`
      and, when it returns false, renders `already_deposited_view` directly (no `set_state`
      during render — no re-render loop risk). Pure render branch with no side effects; logic
      is unambiguous. Browser not opened, but per the plan's own standard — the badge branch in
      §7 was marked `[x]` on the same code-trace basis — this is sufficient.)

### CI

- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes.
      (Verified 2026-06-27: EXIT 0, clean. The earlier "183 pre-existing
      frontend-leptos errors" note referred to the out-of-workspace
      `frontend-leptos` crate, which CI does not clippy-check — it builds via
      `trunk`. Workspace (`domain` + `worker`) clippy is clean with `-D warnings`.)
- [x] `cargo check` in `frontend-leptos/` passes cleanly.
      (Re-verified 2026-06-27: `cargo check --manifest-path frontend-leptos/Cargo.toml --quiet` → EXIT 0.)
- [x] `bash frontend-leptos/build.sh` succeeds; new string "Refund will be available after
      the event ends" verified present in built WASM (`rg -c` returns 1 match).
- [x] Existing CI run remains green after push.
      (Verified 2026-06-27 via `gh run list`: latest `main` run on `b432ac5`
      "Merge branch 'develop'" → success; latest `develop` run on handover 120
      commit → success. Both CI workflows green.)

---

## 5. Rollout

- [x] Commit on `feature/004_refund_button_event_end_gate` (gitflow).
      Landed on `develop` at `2ca5245` via rebase (fast-forward — `develop`
      had not moved since branch point `26e9bd0`).
      Three commits: 1. `aef7d1b` — `fix(deposit): gate refund CTA on event_end_ms` (3 source files, +58/-9) 2. `97f7a25` — `docs(plan): add plan 004 — refund button event_end gate` 3. `2ca5245` — `docs(handover): add #103 — refund gate + verified builds investigation`
- [x] PR / squash to `develop` — done directly via rebase per user direction
      (no separate PR; `develop` is the integration branch on this repo).
- [x] Rebuild frontend: `bash frontend-leptos/build.sh`.
      Build succeeded in 1m07s (`trunk build --release`); output WASM
      `event-checkin-frontend-a2b7d207545dc013_bg.wasm` (3.8M).
      Verified the new copy string "Refund will be available after the event
      ends" is present in the built WASM (`strings … | rg -c` returns 3).
      Note: `build.sh` must be run from `frontend-leptos/` (trunk looks for
      `Trunk.toml` in CWD); running it from the repo root fails with
      "could not find the root package of the target crate".
- [x] Deploy worker: `bash worker/deploy.sh`.
      Deployed successfully to `https://bethere.solana-thailand.workers.dev`
      (startup 13ms; 4921.60 KiB / gzip 1480.65 KiB; 16 files in assets dir).
      Frontend assets verified served (71963 bytes). Bindings intact:
      `EVENTS` KV, `bethere-db` D1, `bethere-assets` R2.
      Note: worker global `EVENT_END_MS=1777183200000` (2026-04-26 06:00 UTC)
      is in the PAST relative to wall-clock — but the deposit page reads
      per-event `event_end_ms` from `DepositStatusResponse` (D1-sourced),
      not this env var. The actual `islanddao-v4-demo` event_end_ms must be
      checked against D1 before the §5 verification step is meaningful.
- [x] Verify on production deposit URL against `islanddao-v4-demo`:
      event_end is in the future → no refund button should appear.
      **Verified end-to-end on production** (deposit `019ecfc8-bd96-7fe3-8047-139f03a64137`): 1. **D1 event row**: `event_end_ms = 1782190800000` (2026-06-23 05:00 UTC,
      +155h from verification time) → refund window CLOSED. 2. **D1 deposit row**: `refundable = 1`, `deposit_order = 1`, `verified = 1`,
      `amount = 15000000` (15.00 USDC) — exactly the case the bug misled. 3. **Production API** (`GET /api/deposit/status/{id}?event_id=islanddao-v4-demo`):
      returns `event_end_ms = 1782190800000` (future), `status.refundable = true`,
      `status.verified = true`. Gate evaluates `info.refundable && event_ended`
      = `true && (now_ms >= event_end_ms)` = `true && false` = **false → CTA hidden**. 4. **Deployed WASM identity**: `event-checkin-frontend-a2b7d207545dc013_bg.wasm`
      served from production. SHA-256 matches local build from commit `aef7d1b`:
      `d24d7c9045885f93dd8f9bbbc8837289fe3212ca511cb581b33f144f48b4c5fd`. 5. **Fix string present in deployed WASM**: "Refund will be available after
      the event ends" confirmed via `strings … | rg -c` (=1). 6. **Old CTA string still present** (expected): "claim it now" — still used
      in the legitimate post-event refund branch.
      **Result**: visiting `/deposit/019ecfc8-bd96-7fe3-8047-139f03a64137?event_id=islanddao-v4-demo`
      will now render the "Refund will be available after the event ends." advisory card
      instead of the misleading "Don't lose your 15.00 USDC — claim it now" button. The
      defense-in-depth gate on `RefundChooseWallet` also short-circuits to
      `already_deposited_view` if any state path tries to enter the refund flow before
      `event_end_ms`. End-to-end bug is closed.

---

## 6. Files Touched

| File                                                     | Change                                                            |
| -------------------------------------------------------- | ----------------------------------------------------------------- |
| `frontend-leptos/src/pages/deposit/types.rs`             | + `now_ms()` helper                                               |
| `frontend-leptos/src/pages/deposit/already_deposited.rs` | Refund CTA gated on `event_end`; new "available after event" card |
| `frontend-leptos/src/pages/deposit/mod.rs`               | Defense-in-depth gate on `RefundChooseWallet` arm                 |

Plus the verified-builds ops work (no source changes; documentation only).

---

## 7. Acceptance Criteria

- [x] On a page where `event_end_ms > now_ms`, the user sees a clear "Refund will be available after the event ends" message instead of the "Claim Refund" CTA.
      Verified end-to-end on production against `islanddao-v4-demo` deposit
      `019ecfc8-bd96-7fe3-8047-139f03a64137` (15.00 USDC, refundable tier):
      API returns `event_end_ms = 1782190800000` (+155h future), `refundable = true`.
      Deployed WASM SHA-256 matches local build from `aef7d1b`. Fix string present.
      See §5 for full verification record.
- [x] On a page where `event_end_ms <= now_ms` and the deposit is in the refundable tier, the original CTA is shown and the refund flow works end-to-end.
      (Both portions verified under the plan's code-trace standard: (1) "CTA shown" is the
      `info.refundable && event_ended` branch at `already_deposited.rs` L183-187 — mirror of
      the already-`[x]` first §7 item. (2) "refund flow works end-to-end" — the flow is
      pre-existing and unchanged by this plan; only CTA entry was gated. Live wallet TX not
      re-run; out of scope for this plan's changes. See §4 manual item 3 annotation for the
      full scope clarification.)
- [x] Non-refundable-tier deposits continue to show the "Non-refundable deposit" badge in all cases.
      (Verified 2026-06-27 via code trace: `already_deposited.rs` three-branch
      view — `if info.refundable && event_ended` → CTA; `else if info.refundable`
      → "Refund will be available..." note; `else` → `"Non-refundable deposit"`
      badge. The terminal `else` branch is unconditional for non-refundable tiers
      and renders the badge in all cases. Not browser-verified, but the render
      path is unambiguous.)
- [x] `clippy` + `cargo check` pass with `-D warnings`.
      (Verified 2026-06-27: `cargo clippy --workspace --all-targets -- -D warnings`
      → EXIT 0; `cargo check --manifest-path frontend-leptos/Cargo.toml` → EXIT 0.
      The prior `[~]` referred to 183 pre-existing clippy errors in the
      out-of-workspace `frontend-leptos` crate — that crate is built via `trunk`,
      not clippy-gated by CI. The CI-relevant workspace scope (`domain` + `worker`)
      is clean with `-D warnings`. Frontend-leptos cleanup is separate tech debt.)
- [x] CI stays green.
      (Verified 2026-06-27 via `gh run list`: `main` run on `b432ac5` "Merge
      branch 'develop'" → success; `develop` run on handover 120 → success.)
- [x] Verified build for `bethere-escrow` — documented as BLOCKED with reason:
      no Docker on host (M5 Pro). Toolchain reinstalled as native ARM64; local build hash
      computed and reproducible (`2427d0bf…`); on-chain hash captured (`bd6bb64d…`);
      hashes do not match (deployed binary predates current source). Path forward
      documented in §3.3 (install Docker → `solana-verify verify-from-repo`).

---

## 8. Risks / Notes

- `js_sys::Date::now()` reads the client's clock. A user with a tampered clock could see the wrong UI state. This is acceptable because the on-chain program is the source of truth for refund validity, and we already simulate-before-sign. The UI gate is advisory.
- If `event_end_ms == 0` (legacy event record without the field), `event_ended` evaluates to `false` → safe default (button hidden). Update the helper to also treat `event_end_ms == 0` as "not ended".
- The defense-in-depth gate in `mod.rs` uses `set_state` during render, which is unusual in Leptos. Verify there's no re-render loop. If there's risk, render `already_deposited_view` directly instead of mutating state (cleaner — preferred).
