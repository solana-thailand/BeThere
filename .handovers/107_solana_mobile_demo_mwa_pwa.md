# Handover 107 — Solana Mobile Demo Slice (MWA + PWA)

> **Branch**: `feature/solana_mobile_demo` (1 commit ahead of `develop`, pushed to `origin`)
> **Status**: ✅ **Pushed + builds clean** (`cargo check`, `cargo clippy`, `trunk build`). **NOT deployed + NOT on-device tested** — needs deploy to dev/prod + manual Android verification before Demo Day.
> **Commit**: `aad83eb` — Phase A (MWA) + Phase B (PWA) in one commit
> **Predecessor**: handover #106 (CI fix + Access & Logistics card)
> **Plan**: `.plans/011_solana_mobile_demo_day.md`
> **Created**: 2026-06-20
> **Demo Day**: 2026-06-23 (3 days)

---

## 1. What Happened

Implemented the Demo-Day Solana Mobile slice per plan #011 — both Phase A (Mobile Wallet Adapter) and Phase B (PWA shell) in a single session. Targeted outcome: BeThere works on Android phones (and Seeker) so the live demo can run end-to-end on a handset, not a laptop.

**Key discovery that simplified the work**: `solana_wallet.js#getDetectedWallets()` already iterates the Wallet Standard registry (`window.navigator.wallets.get()`). MWA-registered wallets appear there automatically. So Phase A was effectively "load the MWA lib + call registerMwa() at app boot" — no per-page Rust changes, no wallet detection code touched.

---

## 2. Changes (1 commit, 6 files, +427 lines)

### `aad83eb feat(mobile): MWA + PWA shell for Solana Mobile (plan #011)`

#### Phase A — Mobile Wallet Adapter

**`frontend-leptos/js/mobile_wallet.js`** (new, 124 lines)
- Exports `registerMwa()` — dynamically imports `@solana-mobile/wallet-standard-mobile@0.5.3` from esm.sh, calls `registerMwa()` with BeThere's app identity
- Runtime guard: only registers on Android (`/android/i.test(navigator.userAgent)`); no-op on iOS/desktop
- Idempotent: tracks `__mwaRegistered` flag, repeated calls are no-ops
- Pinned to v0.5.3 (latest as of 2026-06-20; first stable line with Local Network Access mitigation required for Android 14+)
- Also exports `isMwaRegistered()` and `isAndroidDevice()` sync probes for future UI affordances

**`frontend-leptos/src/wallet.rs`** (new, 55 lines)
- `wasm_bindgen` externs for the three JS functions
- `init_mobile_wallet_adapter()` — spawns the registration promise in the background without blocking app boot; silently swallows errors (MWA is additive, must not break the app if esm.sh is unreachable)

**`frontend-leptos/src/lib.rs`** (+8 lines)
- Added `pub mod wallet;`
- Calls `wallet::init_mobile_wallet_adapter()` at the top of `App()` component

#### Phase B — PWA shell

**`frontend-leptos/manifest.json`** (new)
- Standard PWA manifest: name "BeThere", `display: standalone`, theme/bg `#0f0f0f`
- Icons: uses `/api/badge.svg` (existing deployed Solana-gradient badge) for both `any` and `maskable` purposes
- Shortcuts: "My Ticket" → `/staff`, "Register for Event" → `/`

**`frontend-leptos/sw.js`** (new, 157 lines)
- Minimal service worker for PWA installability + offline shell
- Strategies: network-first for `/api/*`, cache-first for hashed static assets, stale-while-revalidate for SPA shell
- Does NOT make BeThere fully offline-capable — wallet flows always need network

**`frontend-leptos/index.html`** (+41 lines)
- `<link rel="manifest" href="/manifest.json">`
- iOS meta tags: `apple-mobile-web-app-capable`, `apple-mobile-web-app-status-bar-style: black-translucent`, `apple-touch-icon`
- Trunk `data-trunk rel="copy-file"` directives to copy `manifest.json` and `sw.js` to `dist/`
- Inline SW registration script (`navigator.serviceWorker.register("/sw.js")`)

---

## 3. Validation Done

| Check | Method | Result |
|---|---|---|
| Rust compiles | `cargo check --target wasm32-unknown-unknown -p event-checkin-frontend` | ✅ EXIT 0 |
| Clippy clean (new code) | `cargo clippy` | ✅ Zero warnings on `wallet.rs`/`lib.rs` (pre-existing warnings on `wallet_error.rs` unrelated) |
| Security scan | `program_autofixer` on `wallet.rs` | ✅ Zero issues, zero suggestions |
| Frontend builds | `bash build.sh` (trunk release) | ✅ EXIT 0, 1m 05s |
| `mobile_wallet.js` in dist | `ls dist/snippets/.../js/` | ✅ 4.8K |
| `manifest.json` in dist root | `ls dist/` | ✅ 1.0K |
| `sw.js` in dist root | `ls dist/` | ✅ 4.8K |
| WASM size within budget | `ls -lh dist/*.wasm` | ✅ 4.1M raw (well within 3M gzip; ~1.5M gzip expected) |

---

## 4. What Is NOT Validated (must happen before Demo Day)

### 🔴 On-device testing — the critical gap

This code **has not been run on an Android device**. The MWA registration path is correct per Solana Mobile docs but is unverified. Things that can only be checked on-device:

- `registerMwa()` actually fires the Android intent to Phantom
- Phantom appears in the BeThere wallet picker after registration
- Deposit TX signs via MWA (not just via window.solana)
- Claim TX signs via MWA
- "Add to Home Screen" prompt appears in Android Chrome
- Installed PWA launches fullscreen with no browser chrome
- Service worker actually caches the shell

**Test plan**: deploy to prod (or a preview URL), open on Android phone with Phantom installed, walk the demo flow from plan #011 §2.

### 🟡 Deploy

The branch is pushed but not deployed. Standard path: PR `feature/solana_mobile_demo` → `develop` → `main` → `deploy.sh`. Decide whether to deploy directly to prod (with D1 backup first per global rule) or to a preview URL first.

---

## 5. Reflections

### What went well

- **Existing architecture was already Wallet-Standard-aware.** `solana_wallet.js` lines 66–89 already iterated `window.navigator.wallets.get()` — adding MWA was just "wire up the registration call," not "rewrite detection." This was a 5-hour task collapsed to ~1 hour of actual code.
- **Zero-risk profile.** All changes are additive: new JS file, new Rust module, manifest/SW additions to `index.html`. Existing desktop browser-extension wallet flow is untouched. Rollback is `wrangler rollback` (no D1/R2/KV touched).
- **Build clean first try.** Trunk's `data-trunk rel="copy-file"` directive picked up `manifest.json` and `sw.js` without any `build.sh` changes.

### What was struggled with

- **The `edit_file` fuzzy matcher mangled a multi-line edit** on `wallet.rs` — concatenated an `err);` artifact from the old code into the new code. Fixed by rewriting the entire file with `write_file`. Lesson: for large multi-line replacements of structurally different code, prefer `write_file` over `edit_file`.
- **MWA lib version research.** Initial draft guessed v0.27.0; npm registry lookup revealed latest is actually v0.5.3 (the 0.27.0 was probably a confused reference to wallet-adapter versioning). Pinned to 0.5.3 with comment explaining the LNA mitigation constraint.

### What was solved

- The decision to **register from Rust via wasm_bindgen** (rather than an inline `<script>` in `index.html`) keeps the project's handover-007 convention of "no inline scripts, all JS lives in `/js/*.js` and is wired via `wasm_bindgen(module = ...)`." Consistency preserved.
- The decision to **silently swallow registration errors** in Rust avoids pulling in a logging facade dependency for a single call. JS layer logs comprehensively; Rust layer stays silent.

### Open risk: staff scanner (carried from plan #011)

Demo step 5 (on-chain `mark_checked_in` TX) needs the scanner's phone to also have Phantom + SOL. Operator has not yet designated a scanner. **Fallback**: use the off-chain admin dashboard check-in (D1 only) for the live demo and verbally note the on-chain version.

---

## 6. Remaining Work

### Immediate (before Demo Day)

- [ ] **D1 backup before deploy**: `npx wrangler d1 export bethere-db --output backup-pre-demo.sql` (move out of repo per `.gitignore`)
- [ ] **PR `feature/solana_mobile_demo` → `develop`**, merge, merge to `main`
- [ ] **Deploy** via `cd worker && bash deploy.sh`
- [ ] **On-device test** (Android + Phantom) — walk plan #011 §2 demo steps 1–6
- [ ] **Fix anything broken** from on-device test
- [ ] **Designate staff scanner** for Demo Day + ensure their phone has Phantom + devnet SOL

### Optional (if on-device test surfaces issues)

- Pin MWA lib to a specific sha hash via esm.sh's `?pin` query if there's any concern about supply-chain drift between deploy and demo
- Add PNG fallback icons (192/512) if SVG icons don't render correctly on the test device's home screen
- Tune service worker caching if SW interferes with the demo (can always disable SW registration as a quick fix)

---

## 7. How to Dev/Test

### Build the branch

```bash
git fetch origin
git checkout feature/solana_mobile_demo
cd frontend-leptos && bash build.sh                    # EXIT 0 expected
cd .. && cargo check --target wasm32-unknown-unknown -p event-checkin-frontend
```

### Local dev server

```bash
# Terminal 1: worker
cd worker && wrangler dev

# Terminal 2: frontend (with live reload)
cd frontend-leptos && bash build.sh --watch
# Open http://localhost:3001
```

Note: MWA will NOT fire locally (requires Android intent), but you can verify:
- `console.log("[mobile_wallet] Skipping MWA registration (non-Android)")` appears
- `manifest.json` is served at `/manifest.json`
- `sw.js` is served at `/sw.js` and registers (DevTools → Application → Service Workers)

### On-device test (after deploy)

1. Open the deployed URL on Android Chrome with Phantom installed.
2. DevTools → Console should show `[mobile_wallet] Loading MWA library from https://esm.sh/...` then `[mobile_wallet] MWA registered`.
3. DevTools → Application → Manifest should show "BeThere" with no errors.
4. DevTools → Application → Service Workers should show `sw.js` as activated.
5. Chrome menu → "Install app" should be available.
6. Walk the demo flow: register → deposit (USDC) → staff check-in → claim NFT.

---

## 8. Issues Ref

- Plan: `.plans/011_solana_mobile_demo_day.md`
- Issue: `.issues/042_solana_mobile_support.md` (Phase A+B reprioritized P0 for Demo Day)
- Predecessor: handover #106 (CI fix + Access & Logistics card)
- Branch: `feature/solana_mobile_demo` (1 commit, pushed to `origin`)
- **Prod URL**: https://bethere.solana-thailand.workers.dev
- Remote: `git@github.com:solana-thailand/BeThere.git`
- MWA lib: `@solana-mobile/wallet-standard-mobile@0.5.3` via `https://esm.sh/`
- Solana Mobile docs: https://docs.solanamobile.com/get-started/web/installation

---

## 9. Commit Plan

Single commit on `feature/solana_mobile_demo`:

1. `aad83eb feat(mobile): MWA + PWA shell for Solana Mobile (plan #011)`

**Status**:
- ✅ Pushed: `git push -u origin feature/solana_mobile_demo`
- ✅ Validated: `cargo check` EXIT 0, `cargo clippy` clean on new files, `trunk build` EXIT 0
- ⏳ Operator: D1 backup → PR → merge → deploy → on-device test → fix issues → Demo Day
