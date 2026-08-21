//! Mobile Wallet Adapter (MWA) registration for Solana Mobile support.
//!
//! Wires `js/mobile_wallet.js` into Rust so MWA registration runs once at app
//! boot. After registration, MWA wallets (Phantom, Solflare, Seed Vault) appear
//! in the Wallet Standard registry and are picked up by the existing detection
//! code in `solana_wallet.js#getDetectedWallets()` — no per-page changes.
//!
//! See `.plans/011_solana_mobile_demo_day.md` for full design.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/mobile_wallet.js")]
extern "C" {
    /// Register Mobile Wallet Adapter.
    ///
    /// Loads `@solana-mobile/wallet-standard-mobile` from esm.sh and calls
    /// `registerMwa()`. No-op on non-Android platforms (iOS unsupported by MWA,
    /// desktop has no wallet app to intent to). Safe to call multiple times.
    #[wasm_bindgen(js_name = "registerMwa")]
    pub fn js_register_mwa() -> js_sys::Promise;

    /// Has MWA been successfully registered in this session? (sync probe)
    #[wasm_bindgen(js_name = "isMwaRegistered")]
    pub fn js_is_mwa_registered() -> bool;

    /// Is this an Android device (where MWA could work)? (sync probe)
    #[wasm_bindgen(js_name = "isAndroidDevice")]
    pub fn js_is_android_device() -> bool;

    /// Is this a phone/tablet browser? (sync probe)
    #[wasm_bindgen(js_name = "isMobileDevice")]
    fn js_is_mobile_device() -> bool;
}

/// Is the app running in a phone or tablet browser?
///
/// Broader than [`js_is_android_device`], which gates MWA registration: iOS has
/// no Mobile Wallet Adapter but still needs mobile-shaped UI (no "install the
/// browser extension" affordances, app deep links instead).
pub fn is_mobile_device() -> bool {
    js_is_mobile_device()
}

/// Register Mobile Wallet Adapter at app boot.
///
/// Should be called exactly once during app initialization. The underlying JS
/// is idempotent (subsequent calls are no-ops after the first successful
/// registration). Errors are logged from JS but never propagate — MWA is an
/// additive enhancement; failure to register must not break the app.
///
/// Detailed logs (registration attempt, success, failure, skip reason) are
/// emitted by `mobile_wallet.js` to the browser console; this wrapper stays
/// silent on the Rust side to avoid pulling in a logging facade dependency.
pub fn init_mobile_wallet_adapter() {
    let promise = js_register_mwa();
    // Spawn without awaiting — registration runs in the background and
    // completes whenever the esm.sh fetch finishes. Wallet detection polls
    // `window.navigator.wallets` so MWA wallets appear automatically once
    // registration lands, even if the user opens the wallet picker first.
    wasm_bindgen_futures::spawn_local(async move {
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    });
}
