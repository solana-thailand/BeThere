//! Shared "Sign in with Solana Wallet" button + SIWS flow.
//!
//! Single source of truth for the security-sensitive Sign-In With Solana
//! handshake (connect → nonce → signMessage → verify). Used by both the
//! dedicated login page and the inline sign-in on public event (slug) pages so
//! the UI and the crypto flow stay identical.
//!
//! On successful verification a session cookie is set by the worker and the
//! `on_success` callback fires; the caller decides what to do next (navigate,
//! reload, re-check auth).

use leptos::prelude::*;
use leptos::portal::Portal;
use wasm_bindgen::prelude::*;

use crate::icons::{Icon, IconName};

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    fn get_detected_wallets_js() -> Vec<String>;

    #[wasm_bindgen(js_name = "getDetectedWalletsAsync")]
    fn get_detected_wallets_async_js() -> js_sys::Promise;

    #[wasm_bindgen(js_name = "isWalletAvailable")]
    fn is_wallet_available_js(wallet_name: &str) -> bool;

    #[wasm_bindgen(js_name = "connectWallet")]
    fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = "signMessage")]
    fn sign_message_js(wallet_name: &str, message: &str) -> js_sys::Promise;
}

/// Solana SVG icon markup.
fn solana_icon() -> &'static str {
    "<svg viewBox=\"0 0 397.7 311.7\" width=\"20\" height=\"20\">\
        <path fill=\"#9945FF\" d=\"M64.6 237.9c2.4-2.4 5.7-3.8 9.2-3.8h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1l62.7-62.7z\"/>\
        <path fill=\"#14F195\" d=\"M64.6 3.8C67 1.4 70.3 0 73.8 0h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1L64.6 3.8z\"/>\
        <path fill=\"#00C2FF\" d=\"M333.1 120.1c-2.4-2.4-5.7-3.8-9.2-3.8H6.5c-5.8 0-8.7 7-4.6 11.1l62.7 62.7c2.4 2.4 5.7 3.8 9.2 3.8h317.4c5.8 0 8.7-7 4.6-11.1l-62.7-62.7z\"/>\
    </svg>"
}

/// Wallets we can point at an install page when they are not detected.
const KNOWN_WALLETS: [(&str, &str); 3] = [
    ("Phantom", "https://phantom.app/"),
    ("Solflare", "https://solflare.com/"),
    ("Backpack", "https://backpack.app/"),
];

/// Registry name of the Mobile Wallet Adapter pseudo-wallet, registered by
/// `js/mobile_wallet.js` on Android. It stands in for whichever wallet app the
/// user actually has, so the raw name means nothing to them and is relabelled.
const MWA_WALLET: &str = "Mobile Wallet Adapter";

/// User-facing name for a Wallet Standard registry entry.
fn wallet_label(name: &str) -> &str {
    match name {
        MWA_WALLET => "Solana Wallet App",
        other => other,
    }
}

/// Secondary line under the wallet name, where the name alone is not enough.
fn wallet_hint(name: &str) -> Option<&'static str> {
    match name {
        MWA_WALLET => Some("Opens Phantom, Solflare or Seed Vault"),
        _ => None,
    }
}

/// Name + icon cell shared by the connect and install rows.
fn wallet_identity(name: &str) -> AnyView {
    let icon_name = crate::icons::wallet_icon_name(name);
    let label = wallet_label(name).to_string();
    let hint = wallet_hint(name);
    view! {
        <span style="display: flex; align-items: center; gap: 12px;">
            <span style="display: flex; align-items: center; justify-content: center; width: 34px; height: 34px; background: rgba(255,255,255,0.06); border-radius: 10px;">
                <Icon icon=icon_name class="icon-sm" />
            </span>
            <span style="display: flex; flex-direction: column; gap: 2px; text-align: left;">
                <span>{label}</span>
                {hint
                    .map(|h| {
                        view! {
                            <span style="font-size: 0.75rem; font-weight: 400; color: #94a3b8;">
                                {h}
                            </span>
                        }
                    })}
            </span>
        </span>
    }
    .into_any()
}

/// A wallet that is present and can complete the SIWS handshake.
///
/// The whole row is the click target (CSS already makes it look clickable);
/// the badge is a visual affordance only.
fn wallet_connect_row(name: &str, connect: impl Fn(String) + 'static) -> AnyView {
    let name_owned = name.to_string();
    view! {
        <div
            class="siws-wallet-option"
            role="button"
            tabindex="0"
            on:click=move |_| connect(name_owned.clone())
        >
            {wallet_identity(name)}
            <span class="siws-badge-installed">"Connect →"</span>
        </div>
    }
    .into_any()
}

/// A known wallet that is absent — offer its install page. Desktop only: the
/// links point at extension stores that do not exist on mobile.
fn wallet_install_row(name: &str, download_url: &str) -> AnyView {
    view! {
        <div class="siws-wallet-option" style="cursor: default;">
            {wallet_identity(name)}
            <a
                href=download_url.to_string()
                target="_blank"
                rel="noopener noreferrer"
                class="siws-badge-install"
                on:click=move |e| e.stop_propagation()
            >
                "Get Extension ↗"
            </a>
        </div>
    }
    .into_any()
}

/// Wallet apps we can hand off to with a universal "browse" link.
///
/// The path shapes are vendor-specific and are taken from their docs:
///  - Phantom  <https://docs.phantom.com/phantom-deeplinks/other-methods/browse>
///  - Solflare <https://docs.solflare.com/solflare/technical/deeplinks/other-methods/browse>
///
/// Backpack is deliberately absent: it publishes no equivalent browse link, and
/// guessing one would produce a button that silently goes nowhere.
const DEEP_LINK_WALLETS: [(&str, &str); 2] = [
    ("Phantom", "https://phantom.app/ul/browse/"),
    ("Solflare", "https://solflare.com/ul/v1/browse/"),
];

/// Build a universal link that reopens the current page inside a wallet app's
/// in-app browser.
///
/// This is the only sign-in route on iOS, which has no Mobile Wallet Adapter.
/// Inside that browser the wallet injects a normal provider, the origin is
/// unchanged, and the existing SIWS handshake runs untouched.
///
/// Returns `None` only if the location is unreadable, in which case the row is
/// skipped rather than rendered as a dead link.
fn wallet_browse_url(base: &str) -> Option<String> {
    let location = web_sys::window()?.location();
    let href = location.href().ok()?;
    let origin = location.origin().ok()?;
    let target = js_sys::encode_uri_component(&href);
    let referrer = js_sys::encode_uri_component(&origin);
    Some(format!("{base}{target}?ref={referrer}"))
}

/// Hand-off row rendered as a real anchor.
///
/// Deliberately not a JS `location.href` assignment: both vendors document that
/// browse links must be *clicked*, and iOS is markedly more reliable about
/// routing a user-activated link to the app than a scripted navigation.
fn wallet_deep_link_row(name: &str, base: &str) -> Option<AnyView> {
    let url = wallet_browse_url(base)?;
    Some(
        view! {
            <a class="siws-wallet-option" href=url style="text-decoration: none;">
                {wallet_identity(name)}
                <span class="siws-badge-install">"Open App ↗"</span>
            </a>
        }
        .into_any(),
    )
}

/// Caption introducing the hand-off rows.
fn deep_link_caption() -> AnyView {
    view! {
        <p style="margin: 8px 0 0; font-size: 0.8rem; color: #94a3b8; line-height: 1.45;">
            "Or open this page inside your wallet app and sign in from there:"
        </p>
    }
    .into_any()
}

/// Shown when nothing connectable was found, so the modal is never empty.
fn no_wallet_row(mobile: bool) -> AnyView {
    let msg = match mobile {
        true => {
            "No Solana wallet found in this browser. Open this page from inside              your wallet app's built-in browser to sign in."
        }
        false => "No Solana wallet detected. Install one and refresh the page.",
    };
    view! {
        <div
            class="siws-wallet-option"
            style="cursor: default; color: #94a3b8; font-size: 0.85rem; line-height: 1.5;"
        >
            {msg}
        </div>
    }
    .into_any()
}

/// Which SIWS flow the button performs.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum WalletFlow {
    /// Sign in — issues a session cookie via `/api/auth/wallet/verify`.
    #[default]
    Login,
    /// Bind — attaches the wallet to the already-authenticated account via
    /// `/api/auth/wallet/bind`. Does not change the current session.
    Bind,
}

/// Reusable Solana wallet button (sign-in or bind).
///
/// Renders a button; clicking it opens a wallet-picker modal. Selecting an
/// installed wallet runs the SIWS handshake (connect → nonce → signMessage →
/// verify/bind) and, on success, invokes `on_success` with the wallet address.
#[component]
pub fn WalletSignInButton(
    /// Fired with the connected wallet address after success.
    #[prop(into)]
    on_success: Callback<String>,
    /// Login (default) issues a session; Bind attaches to the current account.
    #[prop(optional)]
    flow: WalletFlow,
    /// Trigger button CSS class (default: the login-style purple button).
    #[prop(optional, into)]
    class: Option<String>,
    /// Inline style applied to the trigger button.
    #[prop(optional, into)]
    style: Option<String>,
    /// Trigger button label (default: "Sign in with Solana Wallet").
    #[prop(optional, into)]
    label: Option<String>,
) -> impl IntoView {
    let btn_class = class.unwrap_or_else(|| "btn-google btn-solana-wallet".to_string());
    let btn_style = style.unwrap_or_default();
    let btn_label = label.unwrap_or_else(|| "Sign in with Solana Wallet".to_string());
    let show_icon = btn_class.contains("btn-solana-wallet");
    let endpoint = match flow {
        WalletFlow::Login => "/api/auth/wallet/verify",
        WalletFlow::Bind => "/api/auth/wallet/bind",
    };
    let (show_modal, set_show_modal) = signal(false);
    let (loading, set_loading) = signal(false);
    let (error_msg, set_error_msg) = signal(None::<String>);
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());

    // Detect wallets on mount. Seed synchronously, then await the polling
    // variant: extensions inject late, and Mobile Wallet Adapter only registers
    // once `mobile_wallet.js` finishes its dynamic import — without the second
    // pass a mobile user who opens the picker quickly sees no wallet at all.
    // The list is refreshed again when the modal opens, below.
    Effect::new(move |_| {
        set_detected_wallets.set(get_detected_wallets_js());
        leptos::task::spawn_local(async move {
            let Ok(val) = wasm_bindgen_futures::JsFuture::from(get_detected_wallets_async_js()).await
            else {
                return;
            };
            let names: Vec<String> = js_sys::Array::from(&val)
                .iter()
                .filter_map(|v| v.as_string())
                .collect();
            if !names.is_empty() {
                set_detected_wallets.set(names);
            }
        });
    });

    let connect_wallet_by_name = move |wallet_name: String| {
        set_show_modal.set(false);
        set_loading.set(true);
        set_error_msg.set(None);

        leptos::task::spawn_local(async move {
            let promise = connect_wallet_js_raw(&wallet_name);
            match wasm_bindgen_futures::JsFuture::from(promise).await {
                Ok(val) => {
                    if let Some(pubkey) = val.as_string()
                        && !pubkey.is_empty()
                        && !pubkey.contains("__wallet_error__")
                    {
                        // Request SIWS challenge
                        let req_body = serde_json::json!({ "wallet_address": pubkey });
                        match crate::api::fetch::post(
                            "/api/auth/wallet/nonce",
                            &[("content-type", "application/json")],
                            Some(req_body.to_string()),
                        )
                        .await
                        {
                            Ok(resp) if resp.status() == 200 => {
                                if let Ok(data) = crate::api::fetch::response_json::<
                                    serde_json::Value,
                                >(&resp)
                                .await
                                {
                                    let nonce = data["data"]["nonce"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string();
                                    let message = data["data"]["message"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string();

                                    // Sign the server-issued challenge
                                    let sig_promise = sign_message_js(&wallet_name, &message);
                                    let signature = match wasm_bindgen_futures::JsFuture::from(
                                        sig_promise,
                                    )
                                    .await
                                    {
                                        Ok(v) => v.as_string().unwrap_or_default(),
                                        Err(e) => {
                                            log::error!("[wallet_signin] signMessage failed: {e:?}");
                                            String::new()
                                        }
                                    };
                                    if signature.is_empty() || signature.contains("__wallet_error__")
                                    {
                                        set_error_msg.set(Some(
                                            "Message signing was cancelled or failed.".into(),
                                        ));
                                        set_loading.set(false);
                                        return;
                                    }

                                    let verify_body = serde_json::json!({
                                        "wallet_address": pubkey,
                                        "signature": signature,
                                        "message": message,
                                        "nonce": nonce,
                                    });

                                    match crate::api::fetch::post(
                                        endpoint,
                                        &[("content-type", "application/json")],
                                        Some(verify_body.to_string()),
                                    )
                                    .await
                                    {
                                        Ok(v_resp) if v_resp.status() == 200 => {
                                            log::info!("[wallet_signin] SIWS {endpoint} ok");
                                            on_success.run(pubkey);
                                            return;
                                        }
                                        _ => {
                                            set_error_msg
                                                .set(Some("Wallet verification failed.".into()));
                                        }
                                    }
                                }
                            }
                            _ => {
                                set_error_msg.set(Some("Failed to request wallet nonce.".into()));
                            }
                        }
                    } else {
                        set_error_msg
                            .set(Some(format!("{wallet_name} connection failed or cancelled.")));
                    }
                }
                Err(e) => {
                    log::error!("[wallet_signin] connect error for {wallet_name}: {e:?}");
                    set_error_msg.set(Some(format!(
                        "Could not connect to {wallet_name}. Please make sure it is installed."
                    )));
                }
            }
            set_loading.set(false);
        });
    };

    view! {
        <Show
            when=move || !loading.get()
            fallback=move || {
                view! {
                    <div class="loading visible">
                        <span class="spinner"></span>
                        " Connecting Solana Wallet..."
                    </div>
                }
            }
        >
            <button
                class=btn_class.clone()
                style=btn_style.clone()
                on:click=move |_| {
                    set_detected_wallets.set(get_detected_wallets_js());
                    set_show_modal.set(true);
                }
            >
                {show_icon.then(|| view! { <span inner_html=solana_icon()></span> })}
                {btn_label.clone()}
            </button>
        </Show>

        <Show
            when=move || error_msg.get().is_some()
            fallback=|| view! { <div></div> }
        >
            <div class="error-msg visible" role="alert" aria-live="assertive" style="margin-top: 12px;">
                <Icon icon=IconName::Denied class="icon-md icon-danger" />
                " "
                {move || error_msg.get().unwrap_or_default()}
            </div>
        </Show>

        <Portal>
            <Show
                when=move || show_modal.get()
                fallback=|| view! { <div></div> }
            >
                <div
                    class="siws-modal-backdrop"
                    on:click=move |_| set_show_modal.set(false)
                >
                    <div
                        class="siws-modal-card"
                        on:click=move |e| e.stop_propagation()
                    >
                        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
                            <div style="display: flex; align-items: center; gap: 10px;">
                                <span inner_html=solana_icon()></span>
                                <h3 style="margin: 0; font-size: 1.25rem; font-weight: 700; color: #fff; letter-spacing: -0.01em;">
                                    "Connect Wallet"
                                </h3>
                            </div>
                            <button
                                style="background: rgba(255,255,255,0.06); border: 1px solid rgba(255,255,255,0.1); color: #94a3b8; width: 32px; height: 32px; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 1rem; cursor: pointer; transition: all 0.15s;"
                                on:click=move |_| set_show_modal.set(false)
                            >
                                "✕"
                            </button>
                        </div>

                        <p style="color: #94a3b8; font-size: 0.88rem; line-height: 1.5; margin-top: 0; margin-bottom: 24px;">
                            "Select your Solana wallet to sign in securely with Sign-In With Solana (SIWS)."
                        </p>

                        <div style="display: flex; flex-direction: column; gap: 12px;">
                            {move || {
                                let detected = detected_wallets.get();
                                let mobile = crate::wallet::is_mobile_device();

                                // Everything the registry actually found, in
                                // detection order. These are the only rows that
                                // can complete a sign-in — on Android that
                                // includes the Mobile Wallet Adapter entry,
                                // which hands off to the installed wallet app.
                                let mut rows: Vec<AnyView> = detected
                                    .iter()
                                    .map(|name| wallet_connect_row(name, connect_wallet_by_name))
                                    .collect();

                                // Then the known wallets that were not found.
                                for (name, download_url) in KNOWN_WALLETS {
                                    if detected.iter().any(|d| d.eq_ignore_ascii_case(name)) {
                                        continue;
                                    }
                                    // `is_wallet_available_js` catches wallets
                                    // that inject a provider without announcing
                                    // a matching registry name.
                                    match (is_wallet_available_js(name), mobile) {
                                        (true, _) => {
                                            rows.push(
                                                wallet_connect_row(name, connect_wallet_by_name),
                                            )
                                        }
                                        (false, false) => {
                                            rows.push(wallet_install_row(name, download_url))
                                        }
                                        // Absent on mobile: no extension store
                                        // to send them to, so say nothing here.
                                        (false, true) => {}
                                    }
                                }

                                // Mobile hand-off. Offered even when something
                                // connectable was found: on Android the Mobile
                                // Wallet Adapter row covers only the wallet the
                                // OS resolves the intent to, and on iOS there is
                                // no adapter at all, so this is the sole route.
                                if mobile {
                                    let links: Vec<AnyView> = DEEP_LINK_WALLETS
                                        .iter()
                                        .filter_map(|(name, base)| {
                                            wallet_deep_link_row(name, base)
                                        })
                                        .collect();
                                    if !links.is_empty() {
                                        rows.push(deep_link_caption());
                                        rows.extend(links);
                                    }
                                }

                                match rows.is_empty() {
                                    true => vec![no_wallet_row(mobile)],
                                    false => rows,
                                }
                            }}
                        </div>
                    </div>
                </div>
            </Show>
        </Portal>
    }
}
