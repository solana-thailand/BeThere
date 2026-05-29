# 042 — Solana Mobile Support

> **Date**: 2026-05-28
> **Status**: 📋 Planned
> **Priority**: P2 (post-mainnet)
> **Depends on**: Phase 10 (mainnet deployment)

## Summary

Enable BeThere to work natively on Solana Mobile devices (Seeker, Saga, and any Android phone with an MWA-compliant wallet). This covers three layers:

1. **MWA Web** — Make the Leptos WASM frontend work with Mobile Wallet Adapter on Android Chrome
2. **PWA** — Convert the Trunk build to a Progressive Web App for home-screen install
3. **dApp Store** — List BeThere on the Solana dApp Store for distribution

## Problem

Today, BeThere's wallet-dependent flows (USDC deposit, escrow init, on-chain check-in, refund, NFT claim) rely on desktop browser extension wallets (`window.solana`, `window.phantom`, etc.). On mobile:

- **No browser extensions exist** on Android/iOS browsers
- Attendees at physical events use their phones — desktop is unrealistic
- Staff scanning QR codes on phones can't connect desktop wallets for on-chain check-in
- The `/deposit/:attendee_id`, `/claim/:token`, and refund pages are broken on mobile without wallet access

## Proposed Solution

### Phase A: MWA Web (Zero new codebase)

Register `@solana-mobile/wallet-standard-mobile` in the Leptos frontend via JS interop. This enables any MWA-compliant wallet (Phantom, Solflare, Seed Vault Wallet) to connect locally on Android Chrome.

**Architecture:**

```
Leptos WASM (existing)
  └── JS interop layer
      ├── solana_wallet.js (existing — window.solana / wallet-standard)
      └── registerMwa() from @solana-mobile/wallet-standard-mobile
          └── Android Intent → local WebSocket → wallet app
```

**Changes required:**

| Area | Change | Effort |
|------|--------|--------|
| `index.html` | Add `<script>` loading `@solana-mobile/wallet-standard-mobile` via ESM CDN | 0.5h |
| `frontend-leptos/src/` | Add `wallet_mobile.rs` with `wasm_bindgen` interop for `registerMwa()` | 1h |
| `deposit/js_interop.rs` | Extend wallet detection to include MWA-registered wallets | 1h |
| `claim.rs` | Same wallet detection update | 0.5h |
| Testing | Manual testing on Android device with Phantom/Solflare | 2h |

**Total: ~5h**

**Key constraint:** MWA Web only works on **Android Chrome**. iOS is not supported.

**Package:** `@solana-mobile/wallet-standard-mobile` ≥ v0.5.0 (includes Local Network Access mitigation)

### Phase B: PWA (Home-screen install)

Convert the Trunk-built SPA to a PWA. This gives:

- Home-screen icon on Android/iOS
- Full-screen mode (no browser chrome)
- Offline shell for the landing page and cached events
- Required for Solana dApp Store submission (Phase C)

**Changes required:**

| Area | Change | Effort |
|------|--------|--------|
| `frontend-leptos/manifest.json` | Create Web App Manifest (name, icons, theme, display: standalone) | 1h |
| `frontend-leptos/sw.js` | Service worker for shell caching | 2h |
| `Trunk.toml` | Copy manifest + SW to dist, add `<link rel="manifest">` | 0.5h |
| Icons | Generate 192x192 + 512x512 PWA icons | 0.5h |
| iOS meta tags | `<meta name="apple-mobile-web-app-*">` tags in `index.html` | 0.5h |

**Total: ~4.5h**

**Note:** This upgrades existing P3-2 (PWA Install Prompt) from the UX roadmap from "nice-to-have" to "required for mobile support."

### Phase C: Solana dApp Store (Distribution)

Submit BeThere as a web app to the [Solana dApp Store](https://docs.solanamobile.com/get-started/web/apps). This provides:

- Discovery for Solana Mobile users (Seeker, Saga)
- No Google Play review needed
- Free distribution

**Prerequisites:** Phase A + Phase B complete.

**Changes required:**

| Area | Change | Effort |
|------|--------|--------|
| dApp Store listing | App metadata, screenshots, description | 2h |
| TWA wrapper (optional) | Trusted Web Activity for native-like install | 4h |
| Submission | Submit via Solana dApp Store portal | 1h |

**Total: ~7h** (or ~3h without TWA)

### Not Proposed (Deferred)

| Option | Why deferred |
|--------|-------------|
| React Native app | Requires new codebase (~2-4 weeks). Backend API is reusable, but entire frontend needs rewrite. Only justified if push notifications, native camera, or offline-first are critical. |
| Kotlin native app | Same as above, but Kotlin. Only justified if deep Android integration needed. |
| Flutter app | Community SDK, not officially maintained. Lower priority. |

## Mobile UX Considerations

### Attendee flows (primary mobile users)

| Flow | Mobile Impact | MWA Support |
|------|---------------|-------------|
| `/e/{slug}` (registration) | Already responsive, Google Sign-In works on mobile | N/A |
| `/deposit/:id` (USDC deposit) | **Needs MWA** — wallet must sign Solana Pay TX | ✅ Phase A |
| `/deposit/:id` (THB deposit) | PromptPay QR + slip upload — already mobile-friendly | N/A |
| `/ticket/:id` (ticket/QR) | QR display works on mobile | N/A |
| `/claim/:token` (NFT claim) | **Needs MWA** — wallet must be detected for cNFT mint | ✅ Phase A |
| Refund (from deposit page) | **Needs MWA** — wallet signs `refund` instruction | ✅ Phase A |

### Staff/Admin flows (secondary mobile users)

| Flow | Mobile Impact | MWA Support |
|------|---------------|-------------|
| `/staff` (scanner) | Camera QR scanning works on mobile | N/A |
| On-chain check-in | **Needs MWA** — staff wallet signs `mark_checked_in` TX | ✅ Phase A |
| `/admin` (dashboard) | Responsive but desktop-optimized | N/A |
| Escrow init | **Needs MWA** — organizer wallet signs `create_event` TX | ✅ Phase A |

### Design decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Mobile framework | **MWA Web + PWA** | Zero new codebase. Leptos WASM already runs in mobile Chrome. |
| Wallet protocol | **Mobile Wallet Adapter** | Standard protocol. Works with all major wallets. No per-wallet integration. |
| iOS support | **Not supported (MWA limitation)** | iOS doesn't support MWA. iOS users can use desktop or manual wallet address entry (existing fallback). |
| Distribution | **Solana dApp Store** | Free, no censorship, targets Solana users. |
| React Native | **Deferred** | High effort, low ROI for current scope. Revisit if native features needed. |

## References

- [Mobile Wallet Adapter docs](https://docs.solanamobile.com/solana-mobile-stack/mobile-wallet-adapter)
- [MWA for Web Apps](https://docs.solanamobile.com/get-started/web/apps)
- [Installing Mobile Wallet Standard](https://docs.solanamobile.com/get-started/web/installation)
- [Publish a Web App to dApp Store](https://docs.solanamobile.com/recipes/general/publishing-a-web-app)
- [Local Network Access mitigation](https://docs.solanamobile.com/recipes/mobile-wallet-adapter/local-network-access) — requires `@solana-mobile/wallet-standard-mobile` ≥ v0.5.0
- [Sample apps](https://docs.solanamobile.com/sample-apps/sample_app_overview)

## Relationship to Other Docs

| Document | Relationship |
|----------|-------------|
| `DISCUSSION.md` §8 | Wallet-signed operations — the flows that need MWA |
| `DISCUSSION.md` §10 | Attendee journey by format — mobile is primary for in-person |
| `docs/ux_roadmap.md` P3-2 | PWA install prompt — upgraded to Phase B |
| `.issues/018_wallet_error_recovery.md` | Wallet error UX — needs mobile-specific error messages |
| `README.md` Roadmap | Phase 15 addition |
