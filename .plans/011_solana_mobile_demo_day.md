# Plan 011 — Solana Mobile Demo-Day Slice (MWA + PWA)

> **Status**: PLANNED — awaiting operator go-ahead to start coding.
> **Type**: feature (frontend + minimal worker touch)
> **Priority**: P0 for Demo Day (2026-06-23). Submission deadline 2026-06-22 midnight UTC.
> **Created**: 2026-06-20
> **Branch**: `feature/solana_mobile_demo` (off `develop`)
> **Supersedes**: `.issues/042_solana_mobile_support.md` Phase A/B timing (was P2 post-mainnet → now P0 pre-demo). Phase C (dApp Store listing) explicitly **deferred** — see §5.
> **Depends on**: nothing. Independent of all open plan/issue work.

---

## 1. Problem / Goal

Make BeThere **actually work on a Solana Mobile handset** (Seeker / Saga / any Android with an MWA-compliant wallet) for the IslandDAO V4 demo on 2026-06-23. Specifically: open the URL on the phone, "Add to Home Screen," launch fullscreen, connect Phantom/Solflare/Seed Vault, and complete the on-chain flows (deposit, check-in, claim, refund) **on-device**.

Today, none of this works. The Leptos frontend assumes desktop browser-extension wallets (`window.solana` / `window.phantom`). On mobile there are no extensions — so every wallet-dependent flow silently fails.

The demo outcome we want: a judge with a Seeker scans a BeThere QR → app opens fullscreen → wallet intent fires → USDC deposit lands on-chain → staff check-in mints the cNFT into the judge's wallet. **All on the phone, no laptop in sight.**

---

## 2. Scope

### In scope (the 1-day slice)

- **Phase A — MWA Web**: register `@solana-mobile/wallet-standard-mobile` so any MWA wallet (Phantom / Solflare / Seed Vault) appears as a connect option on Android Chrome. Extends wallet detection in deposit / claim / refund handlers.
- **Phase B — PWA shell**: `manifest.json` + service worker + iOS meta tags + 192/512 icons. Makes the app installable, fullscreen, and offline-capable for the SPA shell.

### Out of scope (explicitly deferred)

- **Phase C — dApp Store listing.** Requires Publisher Account + **KYC/KYB verification** + APK upload to ArDrive + on-chain metadata NFT mint. First-time publisher setup is multi-day and cannot be demo'd in 1 day. Start KYC in parallel if desired; listing lands post-demo.
- **Google Play TWA.** Play Store review is 1–7 days.
- **iOS MWA support.** Per Solana Mobile docs, MWA is Android-only. iOS users keep the existing wallet-address manual-entry fallback. Do not attempt.
- **Native Android app (React Native / Kotlin).** Multi-week rewrite. Deferred per issue #042.

### Demo outcome definition (what "done" means)

A 5-minute live demo on a Seeker or Android-Chrome-with-Phantom handset:

1. Open `bethere.solana-thailand.workers.dev` → "Add to Home Screen" prompt appears.
2. Launch from home screen → fullscreen, no browser chrome.
3. Register for the demo event (Google sign-in works on mobile already).
4. Tap "Deposit USDC" → MWA wallet picker opens → select Phantom → sign → USDC lands in escrow PDA on devnet.
5. Staff scans the attendee QR → on-chain `mark_checked_in` TX signed via staff's MWA wallet.
6. Attendee taps "Claim NFT" → cNFT minted into their wallet via Helius.
7. (Stretch) Refund flow: no-show attendee taps refund → USDC returns from escrow PDA.

If steps 1–6 work on-device, the demo is a success.

---

## 3. Implementation Plan

### Phase A — MWA Web (~5h)

#### A.1 Load the MWA library

**File**: `frontend-leptos/index.html`

Add ESM script tag loading `@solana-mobile/wallet-standard-mobile` from a CDN (esm.sh or jsdelivr). Avoids npm/bundling churn in the Leptos/Trunk pipeline.

```html
<script type="module">
  import { registerMwa, createDefaultAuthorizationCache, createDefaultChainSelector, createDefaultWalletNotFoundHandler }
    from "https://esm.sh/@solana-mobile/wallet-standard-mobile@latest";
  window.__registerMwa = registerMwa;
  window.__mwaDeps = { createDefaultAuthorizationCache, createDefaultChainSelector, createDefaultWalletNotFoundHandler };
</script>
```

**Pin to a specific version** before deploy (don't ship `@latest` to prod). Verify ≥ v0.5.0 for the Local Network Access mitigation (required for Android 14+).

#### A.2 Rust interop module

**File**: `frontend-leptos/src/wallet/mobile.rs` (new)

`wasm_bindgen` externs + a `register_mwa()` entry point that calls `window.__registerMwa` with `appIdentity: { name: "BeThere", uri: "https://bethere.solana-thailand.workers.dev", icon: "/icons/icon-192.png" }`, chains `["solana:devnet", "solana:mainnet"]`, and the default cache/selector/handlers.

Guard with a feature flag or runtime check (`navigator.userAgent.contains("Android")`) so desktop browsers don't pay the cost.

#### A.3 Extend wallet detection

**Files**:
- `frontend-leptos/src/wallet/mod.rs` (or wherever the wallet picker lives — confirm via grep at start of work)
- `frontend-leptos/src/pages/deposit/*.rs`
- `frontend-leptos/src/pages/ticket/claim.rs`

The current detection probably polls `window.solana` / `window.phantom`. MWA-registered wallets appear via the **Wallet Standard** (`window.provider` registering with `registerWalletStandard`), so the existing wallet-standard listener should pick them up automatically once `registerMwa()` runs. Verify this — the bulk of A.3 may be "wire up the registration call at app boot" rather than per-page changes.

#### A.4 Test on device

Manual, on an actual Android device or Seeker (no emulator substitute for MWA — it relies on Android intents). Install Phantom or Solflare from Play Store, browse to the dev URL, walk the flow.

**Total Phase A: ~5h** (matches issue #042 estimate).

### Phase B — PWA shell (~4.5h)

#### B.1 Web App Manifest

**File**: `frontend-leptos/manifest.json` (new) — name, short_name, theme_color (`#13131b` from `style.css` `--bg-primary`), background_color, display: `standalone`, start_url, icons array.

#### B.2 Service worker

**File**: `frontend-leptos/sw.js` (new) — cache the SPA shell (`/`, `/index.html`, WASM, CSS, JS) on install, network-first for `/api/*`, cache-first for static. Keep it minimal — this is for installability + offline shell, not full offline-first.

**Trunk wiring**: `Trunk.toml` `[[hooks]]` to copy `manifest.json` + `sw.js` to `dist/`. Register the SW from `index.html` (`navigator.serviceWorker.register("/sw.js")`).

#### B.3 Icons

Generate 192×192 and 512×512 PNGs from the existing BeThere logo source (check `domain/` or `frontend-leptos/icons/` for the source asset). Also 180×180 for iOS `apple-touch-icon`.

#### B.4 iOS meta tags

**File**: `frontend-leptos/index.html` — `<meta name="apple-mobile-web-app-capable" content="yes">`, `<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">`, `<link rel="apple-touch-icon" href="/icons/icon-180.png">`.

Even though MWA doesn't work on iOS, "Add to Home Screen" still works and gives a native-feeling install for the no-wallet flows (registration, ticket display).

#### B.5 Deploy verification

After deploy: open on Android Chrome, confirm "Install app" prompt appears, confirm launch is fullscreen, confirm SW is registered (DevTools → Application → Service Workers).

**Total Phase B: ~4.5h** (matches issue #042 estimate).

### Combined: ~9.5h — fits one focused day.

---

## 4. Data Safety & Risk

| Risk tier | Items in this plan | Mitigation |
|---|---|---|
| 🟢 Zero-risk | manifest, SW, icons, meta tags | Pure additive frontend; redeploy previous bundle to roll back |
| 🟢 Zero-risk | MWA registration call | Additive — existing wallets continue to work; MWA is opt-in via the picker |
| 🟡 Low-risk | Wallet detection changes | Could in theory mis-route a signing request. Mitigation: keep the existing `window.solana` path as primary; MWA only adds new options to the picker |

**No D1 / R2 / KV / on-chain changes.** No migrations. No new escrow logic. Pure frontend.

**Rollback**: `wrangler rollback` reverts the WASM bundle. The manifest/SW are served from the bundle, so they roll back too.

**Free-tier impact**: zero. Static assets are served by Cloudflare's CDN (`[assets]` binding) — manifest and SW count as static files, not Worker invocations. MWA registration is pure client-side JS, no edge cost.

---

## 5. What is explicitly NOT happening (so nobody re-asks)

### dApp Store listing — deferred

Reasons:
1. **KYC/KYB verification** for the Publisher Account is multi-day and cannot be expedited reliably.
2. Listing requires **on-chain metadata NFT mint** (~0.2 SOL in publisher wallet for fees + ArDrive storage).
3. Listing does not improve the live demo — judges can browse directly to the URL.

**If the user wants this post-demo**: start the Publisher Account KYC now in parallel (operator task, ~30 min to fill forms, then wait). The Phase A+B work is a strict prerequisite for the listing anyway (PWA is required for dApp Store web-app submissions), so this work is not wasted.

### Native iOS — never (in this plan)

Per Solana Mobile docs, MWA is structurally Android-only. iOS users get the existing manual wallet-address entry fallback that already exists for deposit / claim. Do not promise iOS wallet signing.

---

## 6. Validation Plan

| Check | Method | Pass criterion |
|---|---|---|
| MWA library loads | Desktop Chrome DevTools → Network → script 200 | No console errors |
| `registerMwa` called | Console → `window.__registerMwa` is a function | truthy |
| Manifest valid | Lighthouse PWA audit | Score ≥ 90 |
| SW registered | DevTools → Application → Service Workers | sw.js shows "activated" |
| Install prompt appears | Android Chrome → menu → "Install app" | Prompt visible |
| Wallet picker shows MWA option | Android Chrome + Phantom installed | "Mobile Wallet Adapter" or wallet name appears |
| Deposit flow signs via MWA | Walk the flow on device | TX lands on devnet |
| Claim flow mints cNFT via MWA | Walk the flow on device | cNFT in wallet |
| Desktop browsers unaffected | Repeat deposit/claim on desktop Chrome | Existing wallets still work |

---

## 7. Pre-flight checklist (before starting)

- [ ] Confirm with operator: Phase A+B only, dApp Store deferred? (this plan assumes yes)
- [ ] Verify `frontend-leptos/index.html` exists and is the entry HTML
- [ ] Confirm what wallet library / pattern the frontend currently uses (grep `window.solana` / `wallet-standard`)
- [ ] Have an Android device or Seeker available for on-device testing (cannot substitute emulator for MWA)
- [ ] Have Phantom or Solflare installed on the test device
- [ ] Pin MWA library version before deploy (no `@latest` in prod)

---

## 8. Open questions for operator

1. **Is a Seeker available** for the on-device test, or are we testing on a generic Android + Phantom? (Both work; Seeker gives Seed Vault Wallet which is the most demo-impressive.)
2. **Cluster**: devnet or mainnet for the demo deposit? Plan assumes devnet (matches current `cluster:devnet` from `/api/health`). Real USDC on mainnet would be more impressive but requires mainnet escrow init.
3. **Who staff-scans during the demo** — operator or another team member? Their phone also needs an MWA wallet.
4. **Pin versions**: confirm operator is OK with whatever MWA lib version is latest at deploy time, or pick a specific version now.

---

## Refs

- `.issues/042_solana_mobile_support.md` — original Solana Mobile plan (P2 post-mainnet); superseded by this plan for Demo Day scope
- [MWA for Web Apps](https://docs.solanamobile.com/get-started/web/apps)
- [Installing Mobile Wallet Standard](https://docs.solanamobile.com/get-started/web/installation)
- [Local Network Access mitigation](https://docs.solanamobile.com/recipes/mobile-wallet-adapter/local-network-access)
- [Publish a Web App to dApp Store](https://docs.solanamobile.com/recipes/general/publishing-a-web-app) — for post-demo Phase C

## Related plans

- Plan 010 — Free-tier optimization (independent; do not block on this)
