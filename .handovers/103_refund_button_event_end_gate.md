# Handover 103 — Refund Button `event_end` Gate + Verified Builds Investigation

> **Plan ref**: `.plans/004_refund_button_event_end_gate.md`
> **Status**: frontend fix implemented + built; verified-builds BLOCKED (no Docker)
> **Commits**: uncommitted on `develop` (awaiting user OK)
> **Created**: 2026-06-17

---

## 1. What Happened

User reported that the "Claim Refund" button on the deposit page
(`/deposit/<id>?event_id=<event>`) was visible **before the event ended**, with copy
*"Don't lose your 15.00 USDC — claim it now"*. This is misleading: the on-chain
`refund` instruction rejects with `RefundNotYetAllowed` until `clock >= event_end`
(`bethere-escrow/src/instructions/refund.rs#L66-76`). The frontend does simulate
before signing (`handlers.rs::make_claim_refund`) so no funds are lost — the user
just sees a *"Transaction would fail: …RefundNotYetAllowed"* toast — but the UI lying
about availability is a UX bug, especially during demos.

This session:

1. Root-caused the bug (frontend gates on the **tier flag**, not the time).
2. Implemented the fix in 3 frontend files (production-grade, no `unwrap`/`todo`/placeholders).
3. Built the frontend and verified the new copy is present in the WASM binary.
4. Reinstalled `solana-verify` + `quasar-cli` as native ARM64 (was x86_64 → "Bad CPU type").
5. Attempted verified builds for `bethere-escrow` — captured hashes, identified two blockers.

---

## 2. Root Cause

`already_deposited_view` gated the refund CTA on `info.refundable`, but
`refundable: bool` on `DepositStatus` (`domain/src/models/deposit.rs#L57-65`) is the
**refundable-tier flag** (`deposit_order <= max_refundable_deposits`) — NOT a time check.

The required data (`event_end_ms`) was already client-side on `DepositStatusResponse`
(used by `compute_refund_info` in `types.rs#L272-281`). The fix just threads it into the gate.

---

## 3. Changes Made

### 3.1 Frontend: refund CTA gated on `event_end`

**`frontend-leptos/src/pages/deposit/types.rs`** — added two helpers:

```rust
pub fn now_ms() -> i64 {
    js_sys::Date::now() as i64
}

pub fn event_refund_window_open(event_end_ms: i64) -> bool {
    event_end_ms > 0 && now_ms() >= event_end_ms
}
```

`event_refund_window_open` mirrors `bethere-escrow::instructions::refund::validate_and_update`
(semantics: refund allowed iff `clock >= event_end`). Treats `event_end_ms <= 0` as
"not yet open" — fails safe on missing/legacy data.

**`frontend-leptos/src/pages/deposit/already_deposited.rs`** — three-state CTA:

| State | Render |
|-------|--------|
| `info.refundable && event_ended` | Existing CTA: "Don't lose your X USDC — claim it now" + button |
| `info.refundable && !event_ended` | **NEW**: `<div class="dep2-info-note">` with "Refund will be available after the event ends." |
| `!info.refundable` | Existing: "Non-refundable deposit" muted badge (unchanged) |

The existing `compute_refund_info` deadline line renders above this in both refundable
branches (so refundable + not-ended attendees still see "Refund window: 7 days after
event ends (MM/DD HH:MM)").

**`frontend-leptos/src/pages/deposit/mod.rs`** — defense-in-depth gate on `RefundChooseWallet`:

```rust
DepositPageState::RefundChooseWallet(data) => {
    if event_refund_window_open(data.event_end_ms) {
        // existing: refund::refund_choose_wallet_view(...)
    } else {
        // NEW: render already_deposited_view directly (no state mutation during render)
        already_deposited::already_deposited_view(&data, &set_state)
    }
}
```

Renders `already_deposited_view` directly instead of mutating state during render —
avoids any re-render loop risk in Leptos. The primary gate is the CTA in
`already_deposited_view`; this protects against deep links, stale state, or future code
paths that might set `RefundChooseWallet` directly.

### 3.2 Build verification

- `cargo check` in `frontend-leptos/` → **passes cleanly** (exit 0, no errors).
- `cargo clippy` in `frontend-leptos/` reports 183 **pre-existing** errors in untouched
  files (`ticket/view_data.rs`, `utils/qr_gen.rs`, `utils/mod.rs`, `wallet_error.rs`,
  etc.). Filtering to the 3 changed files shows only **pre-existing** warnings — zero
  new warnings introduced by this change. The frontend crate is not part of the workspace
  CI's `clippy --workspace` (it's built via `trunk`, not clippy-checked).
- `bash frontend-leptos/build.sh` → succeeds; new string "Refund will be available after
  the event ends" confirmed present in built WASM via `rg -c` (1 match).

### 3.3 Verified builds: `bethere-escrow` — BLOCKED

**Toolchain fixed** (was broken on M5 Pro):

- `cargo install solana-verify --force --locked` → v0.5.0, Mach-O arm64 ✓
- `cargo install quasar-cli --force --locked` → v0.0.0, Mach-O arm64 ✓
- Both were x86_64 binaries returning "Bad CPU type in executable" before reinstall.

**Deployed program identified**:

- Program ID: `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` (devnet)
- Source: `docs/protocol_poc_requirements.md`, `docs/onchain_event_indexing.md`, handover #053
- Old/deprecated ID: `2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo` (handover #045)
- `solana program show` confirms: 88864 bytes, last deployed slot 465066429,
  authority `9Bz7p4RWdX7eaR4hFUeCc7aSZjDHsie8q1u8imwavkBN`

**Hash comparison (without Docker)**:

| Source | SHA-256 | Size |
|--------|---------|------|
| Local `quasar build` from current HEAD | `2427d0bfdd90af3298927bd4706357f40d0f176d66c349c157814c70b3e5839d` | 89856 B |
| Devnet on-chain (`C6HDeZES…`) | `bd6bb64dcec37820440482eb86aff45d9ae152dfe22f50824b32f199f53a8bbe` | 88864 B |

**Hashes DO NOT match** → deployed devnet binary is NOT built from current source HEAD.
The deployed binary predates several `bethere-escrow/src` commits on `develop` (candidates:
`9d99bf4`, `850c143`, `5c19ab0` — SEC-010 introspection work). Local build is reproducible
(confirmed across two clean rebuilds).

**Environment discovery (important for future builds)**:

- `~/.cargo/config.toml` redirects ALL cargo builds to `/Users/ozone/.cargo/target/`
  (`[build] target-dir = "/Users/ozone/.cargo/target"`). This means
  `bethere-escrow/target/` is NOT where the `.so` lands; the real artefacts live at
  `~/.cargo/target/{deploy,sbpf-solana-solana}/bethere_escrow.so`. Confusing: `rm -rf
  bethere-escrow/target` does NOT invalidate the build cache. To clean the sbpf build,
  target `~/.cargo/target/sbpf-solana-solana/` instead.
- `bethere-escrow-client` (dev-dependency in `Cargo.toml`) is generated by
  `quasar idl .` into `bethere-escrow/target/client/rust/`. If `target/` is wiped,
  must regenerate via `quasar idl .` BEFORE any `cargo build-sbf` will resolve metadata.
- `quasar clean` is destructive in a non-obvious way — after running it, subsequent
  `quasar build` reports "Build complete in 0.2s" but produces no `.so` because the
  sbpf target dir was removed while the global cargo cache (`~/.cargo/target/`) was not.
  Workaround: `rm -rf ~/.cargo/target/sbpf-solana-solana && quasar build`.

**BLOCKERS (require Docker — not installed on this host)**:

- `solana-verify build` (the deterministic Docker-based build) — requires Docker / OrbStack / colima.
  Verified absent on this M5 Pro host: `which docker colima podman orb` → none found;
  no relevant apps in `/Applications/`.
- `solana-verify verify-from-repo --remote` — same Docker dependency.
- On-chain OtterSec/Ellipsis verify PDA upload — not possible without the above.

---

## 4. Files Modified

| File | Change | Lines |
|------|--------|-------|
| `frontend-leptos/src/pages/deposit/types.rs` | + `now_ms()` + `event_refund_window_open()` helpers | +21 |
| `frontend-leptos/src/pages/deposit/already_deposited.rs` | Refund CTA 3-state gate | +18/-2 |
| `frontend-leptos/src/pages/deposit/mod.rs` | Defense-in-depth gate on `RefundChooseWallet` arm | +14/-2 |
| `.plans/004_refund_button_event_end_gate.md` | New plan | +220 |
| `.handovers/103_refund_button_event_end_gate.md` | This handover | (this file) |

Total source diff: **+58/-9** across 3 files. Plan and handover are documentation.

---

## 5. Reflections

### What went well

- Root cause was pinpointed in <1 minute by tracing the `refundable` field's doc comment
  ("refundable tier") vs the on-chain `refund` instruction's time check. The data needed
  to fix it (`event_end_ms`) was already client-side.
- Fix stayed under 60 lines of source change. Three-state CTA logic is readable.
- Defense-in-depth gate in `mod.rs` avoided the `set_state`-during-render footgun by
  rendering `already_deposited_view` directly instead.

### What was struggled with

- **Verified builds rabbit hole**: Spent significant time on the toolchain. The initial
  binary hashes (from the stale `bethere-escrow/target/deploy/bethere_escrow.so`) said
  "match" — but that was an artefact of `quasar build` NOT rebuilding after `quasar clean`
  (because of the global cargo target redirect). After cleaning `~/.cargo/target/sbpf-solana-solana/`
  and forcing a real rebuild, the actual reproducible local hash (`2427d0bf…`) emerged —
  and it does NOT match the deployed hash. The deployed devnet binary is older than current
  HEAD. Important finding: the user has un-deployed escrow source changes on `develop`.
- **Quasar + Docker incompatibility**: Even with Docker, `solana-verify build` runs
  `cargo build-sbf` inside Docker — it doesn't know about Quasar's wrappers. May need a
  custom `--base-image` or a custom build hook. Not investigated further; out of scope
  once Docker was confirmed absent.
- The frontend crate has 183 pre-existing clippy errors in unrelated files. This is
  pre-existing technical debt — the crate is never clippy-checked in CI (only built via
  `trunk`). Don't conflate this with my change's quality.

### What was solved

- Refund button UX bug fixed in a minimal, production-grade way.
- Verified-build toolchain unblocked on ARM64.
- Reproducible local hash captured; on-chain hash captured; gap documented.
- Two important environment quirks discovered and documented (cargo target redirect;
  quasar idl regeneration requirement).

---

## 6. Remain Work

### Immediate (frontend fix rollout)

- [ ] **Get user OK to commit + deploy**. Changes are uncommitted on `develop`.
- [ ] Create gitflow branch `feature/004_refund_button_event_end_gate`.
- [ ] Commit with conventional message: `fix(deposit): gate refund CTA on event_end_ms`
- [ ] Push to `origin/develop`.
- [ ] Rebuild frontend: `bash frontend-leptos/build.sh` (already done locally; rebuild
      needed on whatever machine runs `deploy.sh`).
- [ ] Deploy worker: `bash worker/deploy.sh`.
- [ ] Verify on production: visit `/deposit/<id>?event_id=islanddao-v4-demo` with
      `event_end_ms` in the future → assert NO refund button; assert "Refund will be
      available after the event ends" card is shown.

### Verified builds (blocked)

- [ ] **BLOCKED on Docker** — install OrbStack or `brew install colima docker`.
- [ ] After Docker: `solana-verify verify-from-repo --current-dir --program-id C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T -u https://api.devnet.solana.com`
- [ ] Note: may require custom `--base-image` for Quasar builds. If it doesn't match,
      investigate `solana-verify build --base-image <quasar-aware-image>` or pre-build
      locally and use `verify-from-image`.
- [ ] **Decision needed**: either (a) redeploy escrow from current HEAD then verify, or
      (b) accept that devnet lags and just document the gap.

### Lower priority

- [ ] Frontend crate clippy debt (183 errors in unrelated files) — separate cleanup PR.
- [ ] Phantom wallet trust improvements (Wallet Standard app metadata, custom domain) —
      separate concern from verified builds; do NOT bundle.
- [ ] Consider redeploying the devnet escrow to align deployed binary with source —
      there are un-deployed source changes (introspection, SEC-010, etc.).

---

## 7. How to Dev/Test

### Verify the frontend fix locally

```bash
# Build the frontend (already done; included for completeness)
cd event-checkin/frontend-leptos && bash build.sh

# Run the worker locally
cd event-checkin/worker && npx wrangler dev --port 8787

# Visit with an event that has event_end_ms in the FUTURE:
# http://localhost:8787/deposit/<attendee_id>?event_id=<event_with_future_end>
# → should see "Refund will be available after the event ends." (no refund button)

# To test the post-event-end path, manually update D1:
# UPDATE events SET event_end_ms = <past_ms> WHERE id = '<event>';
# Then reload the deposit page → should see the refund button.
```

### Test the defense-in-depth gate

The primary gate is the CTA in `already_deposited_view`. To test the defense-in-depth
gate independently (e.g. simulating a deep link or stale state), you'd need to manually
drive the state machine into `RefundChooseWallet` with `event_end_ms` in the future.
In practice, this is hard to trigger without code changes. Trust the unit logic:
`event_refund_window_open(event_end_ms)` is `event_end_ms > 0 && now_ms() >= event_end_ms`.

### Rebuild bethere-escrow (for verified builds work)

```bash
cd event-checkin/bethere-escrow

# IMPORTANT: clean the GLOBAL target dir, not the local one (per ~/.cargo/config.toml)
rm -rf ~/.cargo/target/sbpf-solana-solana ~/.cargo/target/deploy/bethere_escrow.so
~/.cargo/bin/quasar build
~/.cargo/bin/solana-verify get-executable-hash ~/.cargo/target/deploy/bethere_escrow.so
# Expected: 2427d0bfdd90af3298927bd4706357f40d0f176d66c349c157814c70b3e5839d

# Compare with on-chain:
~/.cargo/bin/solana-verify get-program-hash C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T -u https://api.devnet.solana.com
# Currently: bd6bb64dcec37820440482eb86aff45d9ae152dfe22f50824b32f199f53a8bbe (≠ local)
```

### If `bethere-escrow/target/` is wiped

The `Cargo.toml` references a dev-dependency at `target/client/rust/bethere-escrow-client/`.
This is generated by `quasar idl .`. Before any `cargo build-sbf` will work:

```bash
cd event-checkin/bethere-escrow
~/.cargo/bin/quasar idl .    # regenerates target/client/rust/bethere-escrow-client/
```

---

## 8. Issues Ref

- Plan: `.plans/004_refund_button_event_end_gate.md`
- Predecessor: handover #102 (signer cross-check + on-chain recovery)
- Related (open): handovers #037, #038 flagged this exact bug as "Refund eligibility
  timing (hide until after event_end_ms) ~30min effort" but never picked up.
- Related (escrow source drift): handover #053 (program ID alignment), #052 (redeploy).

---

## 9. Commit Plan (awaiting user OK)

Three commits proposed for clean history, on branch `feature/004_refund_button_event_end_gate`:

1. `fix(deposit): gate refund CTA on event_end_ms`
   - Files: `types.rs`, `already_deposited.rs`, `mod.rs`
2. `docs(plan): add plan 004 — refund button event_end gate`
   - Files: `.plans/004_refund_button_event_end_gate.md`
3. `docs(handover): add #103 — refund gate + verified builds investigation`
   - Files: `.handovers/103_refund_button_event_end_gate.md`

Then `git rebase` onto `develop` (per standing gitflow rule, not merge).
