# Plan 007 — Dioxus Mobile App (Isolated)

> **Status**: DRAFT — not started; depends on plan 005 (flow harness) and plan 006 (SIWS)
> **Type**: feature (new platform) + R&D (Rust↔MWA bridge)
> **Priority**: P2 (new surface, not blocking current production)
> **Created**: 2026-06-17
> **Decisions locked**: 2 = Dioxus mobile (phased, isolated); 1c = hybrid SIWS; 3b = automated flow harness
> **Predecessors**: 005 (staging worker + flow harness), 006 (SIWS auth endpoints)

---

## 1. Goal

Build a native Android mobile app for the BeThere event platform using Dioxus (Rust), targeting the Solana Mobile ecosystem (Mobile Wallet Adapter / MWA, Seed Vault readiness), **without touching the production web app or worker behavior**.

The mobile app is a NEW deliverable sitting alongside the existing Leptos web frontend. It reuses:
- The `domain` crate (Rust types, business logic) as a direct dependency
- The existing worker API (additive-only consumption)
- The on-chain `bethere-escrow` program (read + transaction construction)

It does NOT reuse:
- The Leptos frontend code (web-only WASM, different framework)
- Any production deploy artifact

---

## 2. Core Principle: Isolation

The production system (`frontend-leptos` + `worker` + `bethere-escrow`, served at `bethere.solana-thailand.workers.dev`) must be **structurally insulated** from all mobile development. This is not a constraint to fight — it's the default state if we isolate correctly.

### The 5 Isolation Rules

1. **Separate crate** — all Dioxus code lives in `mobile-dioxus/`. Never edited inside `frontend-leptos/` or `worker/src/`. A bug in mobile physically cannot change the web binary or worker bundle.
2. **Separate deploy target** — mobile ships to Play Store internal testing. It CANNOT reach the production worker deploy pipeline. The worker only changes when we explicitly run `deploy.sh`, and that's a deliberate gate.
3. **Additive-only API** — if mobile needs new endpoints, they're ADDED alongside existing ones (e.g. SIWS in plan 006). Existing endpoints are never modified or removed. Web keeps calling the same endpoints it calls today.
4. **Staging environment for dev** — mobile points at the staging worker (set up in plan 005), never `bethere.solana-thailand.workers.dev`. Accidental "pointed at prod" is structurally impossible.
5. **Feature flags for any worker change** — the rare case where the worker DOES change (new endpoint), it's behind a flag, off by default. Production behavior is identical until a flag flips.

### Isolation diagram

```
┌─────────────────────────────────────────────────────────────┐
│  PRODUCTION (untouched, keeps running identically)          │
│  ┌──────────────┐   ┌──────────────┐   ┌─────────────────┐  │
│  │ frontend-    │   │   worker     │   │ bethere-escrow   │  │
│  │ leptos (WASM)│◄─►│ (Cloudflare) │◄─►│ (on-chain)       │  │
│  └──────────────┘   └──────┬───────┘   └─────────────────┘  │
│                            │                                │
│                  existing API endpoints (unchanged)         │
└────────────────────────────┼────────────────────────────────┘
                             │  (additive only — rule #3)
┌────────────────────────────┼────────────────────────────────┐
│  NEW (mobile, isolated)    ▼                                │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  mobile-dioxus/  (NEW crate, separate branch)        │   │
│  │  - Dioxus UI (Rust)                                  │   │
│  │  - reuses `domain` crate (read-only dep)             │   │
│  │  - MWA wallet bridge (Rust↔Kotlin, NEW)              │   │
│  └──────────────────────────────────────────────────────┘   │
│  Points at STAGING worker during dev (rule #4)              │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. Scope

### In scope

- New `mobile-dioxus/` crate (Dioxus 0.6+, Android-first).
- Read-only event/deposit/ticket views (phase 0).
- SIWS authentication via MWA (phase 1, depends on plan 006).
- Deposit / refund / claim flows via MWA (phase 2).
- Rust↔Kotlin MWA bridge (NEW infrastructure; R&D — see §6).
- CI build for the mobile crate (APK artifact, not deployed to prod).

### Out of scope (explicitly deferred)

- **iOS** — Android-first per Solana Mobile ecosystem focus. iOS later.
- **Web frontend migration (Leptos → Dioxus)** — this is the ONLY phase that would touch production web, and it's a separate plan (call it plan 008 if it happens). The full payoff of Dioxus only lands if web also migrates, but that's a deliberate future decision, not now.
- **Seed Vault integration** — depends on Solana Mobile hardware (Saga / Chapter 2). Build MWA first; Seed Vault is a later optimization.
- **App Store / Play Store public release** — internal testing (Play Internal) only during these phases. Public release is a separate launch plan.
- **Production worker changes** — none in this plan. SIWS endpoints come from plan 006 and land first.

---

## 4. Architecture

### 4.1 Directory structure

```
event-checkin/
├── frontend-leptos/        # UNCHANGED — production web
├── worker/                 # additive-only changes (from plan 006)
├── domain/                 # shared, additive-only
├── bethere-escrow/         # UNCHANGED
└── mobile-dioxus/          # NEW — this plan
    ├── Cargo.toml
    ├── src/
    │   ├── main.rs              # Dioxus launcher (dioxus::launch)
    │   ├── api/                 # worker API client (typed, reuses domain)
    │   ├── pages/               # screens (events, deposit, ticket, claim)
    │   ├── wallet/              # wallet abstraction (trait + impls)
    │   │   ├── mod.rs           # WalletProvider trait
    │   │   ├── android.rs       # MWA bridge via UniFFI/JNI
    │   │   └── mock.rs          # test/stub impl
    │   └── platform/            # Android-specific Kotlin shims
    ├── android/                 # Android project (Gradle, Kotlin MWA SDK)
    │   ├── app/
    │   │   └── src/main/java/.../MainActivity.kt
    │   └── build.gradle.kts
    └── README.md
```

### 4.2 Wallet layer (platform-specific)

The wallet layer is the ONLY platform-specific part. It's abstracted behind a trait so the UI and business logic stay platform-agnostic:

```rust
// mobile-dioxus/src/wallet/mod.rs
#[async_trait]
pub trait WalletProvider: Send + Sync {
    async fn connect(&self) -> Result<Pubkey, WalletError>;
    async fn sign_in_with_solana(&self, message: &SiwsMessage) -> Result<Signature, WalletError>;
    async fn sign_and_send(&self, tx: &Transaction) -> Result<Signature, WalletError>;
}
```

Implementations:
- `android::MwaWallet` — calls Kotlin MWA SDK via UniFFI/JNI. THIS IS THE R&D (see §6).
- `mock::MockWallet` — for unit tests and phase-0 dev without a real wallet.

On web (future Dioxus web migration), a `web::WalletStandardWallet` would implement the same trait via `wasm_bindgen` to `window.solana`. Not in scope now.

### 4.3 API client (reuses domain types)

The mobile API client uses the `domain` crate's types directly (no reimplementation, no WASM boundary):

```rust
// mobile-dioxus/src/api/mod.rs
use domain::models::{DepositStatusResponse, Event, Attendee};

pub struct ApiClient { base_url: String, /* ... */ }

impl ApiClient {
    pub async fn public_events(&self) -> Result<Vec<Event>> { /* GET /api/public/events */ }
    pub async fn deposit_status(&self, attendee_id: &str, event_id: &str) -> Result<DepositStatusResponse> { /* ... */ }
    // ... typed wrappers over existing endpoints
}
```

`base_url` comes from config — staging during dev, never production (rule #4).

### 4.4 API surface consumed (from worker route audit)

Read-only (phase 0):
- `GET /api/public/events` — event list
- `GET /api/public/event/{slug}` — event detail
- `GET /api/deposit/status/{attendee_id}?event_id=...` — deposit status
- `GET /api/public/ticket/{id}` — public ticket view

Authed (phase 1+, via SIWS JWT):
- `POST /api/auth/siws` — SIWS verification (NEW, from plan 006)
- `GET /api/auth/me` — current user
- `GET /api/my-registrations` — user's registrations

Write/transaction (phase 2):
- `POST /api/deposit/usdc` — construct deposit tx
- `POST /api/escrow/refund` — construct refund tx
- `POST /api/claim/{token}` — NFT claim

All existing endpoints are consumed as-is. The only NEW endpoint is SIWS (from plan 006, not this plan).

---

## 5. Phased Implementation

### Phase 0 — Read-only mobile (proves architecture, zero prod risk)

**Goal**: prove Dioxus mobile builds, runs on Android, calls the staging worker, renders real data. No auth, no writes.

- [ ] Scaffold `mobile-dioxus/` crate with Dioxus 0.6+ mobile target.
- [ ] Android project skeleton (Gradle, `MainActivity.kt` hosting Dioxus).
- [ ] `ApiClient` with read-only methods hitting staging worker.
- [ ] Screens: event list (`GET /api/public/events`), event detail (`GET /api/public/event/{slug}`), deposit status (`GET /api/deposit/status/{id}`).
- [ ] Build APK via CI; install on emulator + physical device.
- [ ] Verify: app launches, fetches staging data, renders.

**Definition of done**: APK builds, installs, shows real event data from staging. No wallet, no auth, no writes. Production worker untouched (we haven't deployed anything).

### Phase 1 — SIWS authentication (depends on plan 006)

**Goal**: mobile user authenticates via Solana wallet instead of Google.

- [ ] `WalletProvider` trait + `MockWallet` impl.
- [ ] SIWS message construction (domain, statement, nonce, URI per SIWS spec).
- [ ] POST `/api/auth/siws` (endpoint added in plan 006) — verify signature server-side, issue JWT.
- [ ] Store JWT, attach to authed requests (`/api/auth/me`, `/api/my-registrations`).
- [ ] `MwaWallet` v0: `connect()` + `sign_in_with_solana()` via MWA `signMessage` deep-link.
- [ ] Screens: login (wallet connect + SIWS sign), my registrations.

**Definition of done**: user taps "Sign in with Solana", Phantom (or other MWA wallet) opens, signs SIWS message, mobile receives JWT, can call authed endpoints. Production web users see zero change (web still uses Google auth).

### Phase 2 — Deposit / refund / claim (full feature parity for core flows)

**Goal**: mobile user can deposit USDC, claim refund, claim NFT — all via MWA.

- [ ] `MwaWallet` v1: `sign_and_send()` via MWA `signAndSendTransaction` deep-link.
- [ ] Transaction construction reusing `domain::solana` builders (same as worker uses).
- [ ] Deposit flow: `POST /api/deposit/usdc` → construct tx → MWA sign → confirm.
- [ ] Refund flow: `POST /api/escrow/refund` → construct tx → MWA sign → confirm.
- [ ] Claim flow: `POST /api/claim/{token}` → NFT mint → MWA sign → confirm.
- [ ] Apply the SAME `event_end` gate as plan 004 (reuse `domain::event_refund_window_open` — extract it to `domain` if not already there; see §11 notes).

**Definition of done**: full deposit → attend → refund/claim lifecycle works on mobile via Phantom MWA. Production web flows unchanged.

### Phase 3 — (OPTIONAL, FUTURE, SEPARATE PLAN) Web Leptos → Dioxus migration

NOT in this plan. This is the only phase that would touch production web. If/when it happens:
- Feature-flagged, canaried, reversible.
- New plan (e.g. plan 008).
- Deferred indefinitely unless we decide to converge on one framework.

---

## 6. R&D Risk: The Rust↔MWA Bridge

This is the most honest risk in the plan and deserves its own section.

### What MWA is

Mobile Wallet Adapter (MWA) is Solana Mobile's Android standard for dApp↔wallet communication via intent-based deep-linking. It's how Phantom/Solflare/Backpack on Android receive sign requests from dApps. It's Kotlin/Android-native.

### Why there's no Rust binding

There is no official Rust binding for the MWA SDK. The ecosystem is Kotlin-first (Android) and React Native (via community SDKs). A Rust mobile app wanting MWA must bridge itself.

### What the bridge looks like

```
Rust (Dioxus)  →  UniFFI/JNI  →  Kotlin MWA SDK  →  Phantom/Solflare
```

Options, in order of preference:
1. **UniFFI** (Mozilla's Rust↔Kotlin FFI) — cleanest, generates Kotlin bindings from Rust. Our direction.
2. **Raw JNI** — more boilerplate, more control. Fallback if UniFFI has issues.
3. **Kotlin shim as host activity** — Dioxus renders inside a Kotlin Activity that owns the MWA client; Rust calls it via IPC. Heavier but most compatible.

### Honest risk assessment

- **We're among the first.** I'm not aware of a production-grade open-source Rust↔MWA bridge. This is R&D, not "follow the docs."
- **Ongoing maintenance.** MWA evolves (v2 → v3 etc.); the bridge must track it. Pin SDK versions in `android/build.gradle.kts`.
- **Mitigation via phasing**: phase 0 + phase 1 don't need `signAndSendTransaction` — only `signMessage` (simpler API surface). The hard tx-signing bridge lands in phase 2, AFTER we've proven the simpler paths. If the bridge proves too costly in phase 1, we re-evaluate before committing to phase 2.

### Fallback if MWA bridge fails

If the bridge proves unbuildable in phase 1/2, fallback is: **Dioxus mobile for UI + read-only + SIWS (signMessage is simpler), and delegate tx-signing to an in-app WebView that runs the existing Leptos deposit/refund pages.** Less elegant, but ships without the bridge risk. This is the documented escape hatch.

---

## 7. Testing

### Unit

- [ ] `wallet::MockWallet` covers the trait contract.
- [ ] `api::ApiClient` tested against a mock HTTP server (`wiremock` crate).
- [ ] SIWS message construction tested against the SIWS spec test vectors.

### Integration (against staging worker from plan 005)

- [ ] Phase 0: read-only endpoints return data on staging.
- [ ] Phase 1: SIWS auth issues JWT; authed endpoints work on staging.
- [ ] Phase 2: deposit/refund/claim on devnet escrow via staging worker.

### E2E on device

- [ ] APK on Android emulator (API 33+).
- [ ] APK on physical Android device with Phantom MWA installed.
- [ ] Full lifecycle: open app → SIWS → view event → deposit → (simulate event end) → refund.

### The safety net from plan 005

- [ ] Every worker deploy (none directly in this plan, but plan 006's SIWS deploy) runs the automated flow harness from plan 005 BEFORE going to production. Existing flows must pass. This is what makes additive changes safe.

---

## 8. Rollout

- [ ] Phase 0 APK distributed via Play Store internal testing (just the team).
- [ ] Phase 1 APK adds SIWS; team tests auth.
- [ ] Phase 2 APK adds full flows; team + small beta tests.
- [ ] Public Play Store release: SEPARATE plan, not this one.
- [ ] **At no point does the production worker deploy depend on this plan.** Worker changes come from plan 006 and are gated by plan 005's harness.

---

## 9. Files Touched

| Area | Change |
|------|--------|
| `mobile-dioxus/` (NEW) | Entire crate — Dioxus app, API client, wallet layer, Android project |
| `domain/` | Additive only — extract `event_refund_window_open` for cross-frontend reuse (see §11) |
| `worker/` | NONE in this plan (plan 006 handles SIWS endpoints) |
| `frontend-leptos/` | NONE — production web untouched |
| `bethere-escrow/` | NONE — on-chain program untouched |
| `.github/workflows/` | Add mobile APK build job (separate from worker deploy) |

---

## 10. Acceptance Criteria

- [ ] Phase 0: APK installs on Android, renders real staging data, zero production worker changes.
- [ ] Phase 1: SIWS auth works end-to-end (mobile → MWA wallet → staging worker JWT), production web auth unchanged.
- [ ] Phase 2: deposit/refund/claim work via MWA on devnet, production web flows unchanged.
- [ ] `mobile-dioxus/` is a separate crate; `cargo build` in workspace does NOT compile it unless explicitly targeted (avoid build-time coupling).
- [ ] No file under `frontend-leptos/`, `worker/src/`, or `bethere-escrow/src/` is modified by this plan (except the documented `domain` extraction in §11).
- [ ] The automated flow harness (plan 005) passes on staging before any worker deploy referenced by this plan.

---

## 11. Risks / Notes

- **MWA bridge is R&D.** See §6. Phase 1 (signMessage only) is the proof point. If it fails, fallback to WebView delegation for tx-signing.
- **Dioxus mobile maturity.** Dioxus 0.6 mobile is usable but younger than React Native. Expect edges; report upstream.
- **Single-developer risk.** The MWA bridge is specialized knowledge. Document heavily; consider open-sourcing the bridge as a standalone crate to attract maintenance.
- **Solana Mobile SDK version drift.** MWA spec evolves; the bridge must track it. Pin SDK versions in `android/build.gradle.kts`.
- **The `domain` extraction.** Moving `event_refund_window_open` from `frontend-leptos/src/pages/deposit/types.rs` to `domain` is a tiny refactor but it's the ONE place this plan touches code outside `mobile-dioxus/`. Do it carefully: (a) add to `domain`, (b) update `frontend-leptos` to re-export from domain, (c) run plan 005 harness, (d) then proceed. This is the only cross-cutting change.
- **This plan does NOT justify itself by mobile demand.** If there's no real user pull for a mobile app, phase 0 is still worth it (proves architecture, exercises the staging env from plan 005), but phases 1–2 should wait for demand. Don't build mobile in search of a problem.
- **Phantom on Android is the primary test wallet.** Solflare and Backpack are secondary. Verify all three in phase 2.

---

## 12. Dependencies

- **plan 005** (staging worker + flow harness) — REQUIRED before phase 0. The staging worker is where mobile dev points; the harness is what makes any downstream worker change safe.
- **plan 006** (SIWS auth endpoints) — REQUIRED before phase 1. Mobile auth IS SIWS; can't do phase 1 without the endpoint existing.

**Sequencing**: 005 → 006 → 007 (this plan). 005 and 006 can be drafted in parallel; 007 starts after 005 lands (staging env must exist first).

---

## 13. Open Questions (resolve before phase 0)

- [ ] Which Solana Mobile SDK version do we target? (MWA v1 stable vs v2 prerelease — check current as of phase 0 kickoff.)
- [ ] Min Android API level? (Recommend 26 / Android 8.0+ for modern wallet adoption; verify against Phantom's minApi.)
- [ ] Do we need an `assetlinks.json` for deep-link verification? (Required for MWA intent verification on production builds.)
- [ ] App signing key strategy for Play Internal Testing? (Separate from worker deploy keys entirely.)
````
