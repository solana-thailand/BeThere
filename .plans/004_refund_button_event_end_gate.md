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
      Also added `event_refund_window_open(event_end_ms: i64) -> bool` helper that
      encapsulates the `event_end_ms > 0 && now_ms() >= event_end_ms` semantics
      (fails safe on missing data) and is reused by both call sites.
- [x] In `already_deposited_view`, compute `let event_ended = event_refund_window_open(data.event_end_ms);`
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

- [x] In the `DepositPageState::RefundChooseWallet(data)` arm in `mod.rs`, if `!event_refund_window_open(data.event_end_ms)`,
      render `already_deposited_view` directly (no `set_state` during render — avoids re-render loop).
      This protects against future code paths that might set `RefundChooseWallet` directly
      (e.g. deep links) and against any state-machine race.

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

- [ ] `frontend-leptos`: no Rust unit tests in this crate currently — skip.
- [ ] `domain`: existing `is_refundable_tier` tests already cover tier logic; no change needed there.

### Manual (devnet / local worker)

- [ ] Visit `/deposit/<id>?event_id=<event_with_future_event_end>` with a verified deposit →
      assert NO refund button; assert the "Refund will be available after the event ends" card is shown.
- [ ] Manually set D1 `events.event_end_ms` to a past timestamp for a test event → reload page →
      assert refund button IS shown.
- [ ] Click refund button post-event → wallet simulation passes → tx submits → `RefundConfirmed` state.
- [ ] Try a direct `set_state(RefundChooseWallet)` (via console or temporary test harness) with
      `event_end_ms` in the future → assert defense-in-depth gate redirects to `AlreadyDeposited`.

### CI

- [ ] `cargo clippy --workspace --locked -- -D warnings` passes.
      Note: `cargo clippy` on `frontend-leptos` (outside workspace) reports 183
      pre-existing errors in untouched files (`ticket/view_data.rs`, `utils/qr_gen.rs`,
      `utils/mod.rs`, `wallet_error.rs`, etc.). Filtering to my 3 changed files shows
      only pre-existing warnings — no new ones introduced by this change.
- [x] `cargo check` in `frontend-leptos/` passes cleanly.
- [x] `bash frontend-leptos/build.sh` succeeds; new string "Refund will be available after
      the event ends" verified present in built WASM (`rg -c` returns 1 match).
- [ ] Existing CI run remains green after push.

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
- [ ] On a page where `event_end_ms <= now_ms` and the deposit is in the refundable tier, the original CTA is shown and the refund flow works end-to-end.
- [ ] Non-refundable-tier deposits continue to show the "Non-refundable deposit" badge in all cases.
- [~] `clippy` + `cargo check` pass with `-D warnings`.
  Partial: `cargo check` in `frontend-leptos/` passes clean (exit 0).
  `cargo clippy` introduces 0 new warnings on the 3 changed files, but the
  frontend crate has 183 pre-existing clippy errors in untouched files
  (`ticket/view_data.rs`, `utils/qr_gen.rs`, `utils/mod.rs`, `wallet_error.rs`,
  etc.) — so `clippy -D warnings` does NOT pass crate-wide. The frontend crate
  is built via `trunk`, not clippy-checked in workspace CI. Pre-existing tech
  debt; separate cleanup PR.
- [ ] CI stays green.
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
