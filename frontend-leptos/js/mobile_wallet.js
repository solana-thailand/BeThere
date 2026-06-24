/**
 * Mobile Wallet Adapter (MWA) bootstrap for Solana Mobile support.
 *
 * Registers `@solana-mobile/wallet-standard-mobile` so MWA-compliant wallets
 * (Phantom, Solflare, Seed Vault Wallet) appear as Wallet Standard providers
 * on Android Chrome. On Android, this opens a local intent-based channel to
 * the wallet app — same UX as native Android dApps.
 *
 * Why a separate module (not inline in index.html):
 *  - Follows the project convention from handover 007 (no inline scripts, all
 *    JS lives in `/js/*.js` and is wired via `#[wasm_bindgen(module = "...")]`).
 *  - Keeps the runtime Android guard + esm.sh dynamic import logic testable.
 *
 * Detection integration:
 *  - After `registerMwa()` runs, MWA wallets appear in `window.navigator.wallets`
 *    (the Wallet Standard registry).
 *  - `solana_wallet.js#getDetectedWallets()` already iterates that registry,
 *    so existing deposit/claim/escrow flows pick up MWA wallets automatically.
 *  - No per-page Rust changes required.
 *
 * Imported via `#[wasm_bindgen(module = "/js/mobile_wallet.js")]` in Rust.
 *
 * Refs:
 *  - https://docs.solanamobile.com/get-started/web/installation
 *  - Plan 011 (`.plans/011_solana_mobile_demo_day.md`)
 */

// Runtime guard: only register on Android. iOS doesn't support MWA, and
// registering on desktop is wasted work (no wallet app to intent to).
function isAndroid() {
  try {
    return /android/i.test(navigator.userAgent);
  } catch (e) {
    return false;
  }
}

// Track registration state so repeated calls are no-ops.
var __mwaRegistered = false;

/**
 * Register Mobile Wallet Adapter.
 *
 * Loads `@solana-mobile/wallet-standard-mobile` from esm.sh (pinned in
 * `MWA_LIB_URL`), then calls `registerMwa()` with BeThere's app identity.
 *
 * No-op on non-Android platforms and on repeated calls.
 *
 * @returns {Promise<boolean>} resolves true if registration succeeded,
 *   false if skipped (non-Android, already registered, or load failed).
 */
export async function registerMwa() {
  // Skip on non-Android (iOS unsupported by MWA; desktop has no wallet app).
  if (!isAndroid()) {
    console.log("[mobile_wallet] Skipping MWA registration (non-Android)");
    return false;
  }

  // Skip if already registered (App component may mount more than once).
  if (__mwaRegistered) {
    console.log("[mobile_wallet] MWA already registered — skipping");
    return true;
  }

  // Pin to a specific version for prod. Bump deliberately; verify Local Network
  // Access mitigation (>= v0.5.0) is still present after any upgrade.
  // Ref: https://docs.solanamobile.com/recipes/mobile-wallet-adapter/local-network-access
  // Pinned to 0.5.3 (latest as of 2026-06-20). This is the first stable line that
  // includes the Local Network Access mitigation required for Android 14+.
  // Bump deliberately; verify the LNA mitigation is preserved after any upgrade.
  var MWA_LIB_URL =
    "https://esm.sh/@solana-mobile/wallet-standard-mobile@0.5.3";

  try {
    console.log("[mobile_wallet] Loading MWA library from", MWA_LIB_URL);
    var mod = await import(MWA_LIB_URL);
    var register = mod.registerMwa;
    var createDefaultAuthorizationCache = mod.createDefaultAuthorizationCache;
    var createDefaultChainSelector = mod.createDefaultChainSelector;
    var createDefaultWalletNotFoundHandler =
      mod.createDefaultWalletNotFoundHandler;

    if (typeof register !== "function") {
      console.error("[mobile_wallet] registerMwa not found in module exports");
      return false;
    }

    register({
      appIdentity: {
        name: "BeThere",
        uri: "https://bethere.solana-thailand.workers.dev",
        icon: "https://bethere.solana-thailand.workers.dev/api/badge.svg",
      },
      authorizationCache: createDefaultAuthorizationCache
        ? createDefaultAuthorizationCache()
        : undefined,
      chains: ["solana:devnet", "solana:mainnet"],
      chainSelector: createDefaultChainSelector
        ? createDefaultChainSelector()
        : undefined,
      onWalletNotFound: createDefaultWalletNotFoundHandler
        ? createDefaultWalletNotFoundHandler()
        : undefined,
    });

    __mwaRegistered = true;
    console.log(
      "[mobile_wallet] MWA registered — Phantom/Solflare/Seed Vault will appear in wallet picker on Android",
    );
    return true;
  } catch (e) {
    console.error("[mobile_wallet] Failed to register MWA:", e);
    return false;
  }
}

/**
 * Synchronous probe — has MWA been successfully registered in this session?
 *
 * Useful for UI affordances ("Solana Mobile ready" badge) without awaiting.
 */
export function isMwaRegistered() {
  return __mwaRegistered;
}

/**
 * Synchronous probe — is this an Android device (where MWA could work)?
 */
export function isAndroidDevice() {
  return isAndroid();
}
