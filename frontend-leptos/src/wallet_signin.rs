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

/// Reusable Solana wallet sign-in button.
///
/// Renders the purple "Sign in with Solana Wallet" button; clicking it opens a
/// wallet-picker modal. Selecting an installed wallet runs the full SIWS flow
/// and, on success, invokes `on_success`.
#[component]
pub fn WalletSignInButton(
    /// Fired after a successful SIWS verification (session cookie is set).
    #[prop(into)]
    on_success: Callback<()>,
) -> impl IntoView {
    let (show_modal, set_show_modal) = signal(false);
    let (loading, set_loading) = signal(false);
    let (error_msg, set_error_msg) = signal(None::<String>);
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());

    // Detect wallets on mount (Wallet Standard registry may populate late, so
    // we also refresh when the modal opens, below).
    Effect::new(move |_| {
        set_detected_wallets.set(get_detected_wallets_js());
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
                                        "/api/auth/wallet/verify",
                                        &[("content-type", "application/json")],
                                        Some(verify_body.to_string()),
                                    )
                                    .await
                                    {
                                        Ok(v_resp) if v_resp.status() == 200 => {
                                            log::info!("[wallet_signin] SIWS authenticated");
                                            on_success.run(());
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
                class="btn-google btn-solana-wallet"
                on:click=move |_| {
                    set_detected_wallets.set(get_detected_wallets_js());
                    set_show_modal.set(true);
                }
            >
                <span inner_html=solana_icon()></span>
                "Sign in with Solana Wallet"
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
                                let installed_list = detected_wallets.get();
                                let supported = vec![
                                    ("Phantom", "https://phantom.app/"),
                                    ("Solflare", "https://solflare.com/"),
                                    ("Backpack", "https://backpack.app/"),
                                ];

                                supported.into_iter().map(|(w_name, download_url)| {
                                    let is_installed = installed_list.iter().any(|w| w.eq_ignore_ascii_case(w_name)) || is_wallet_available_js(w_name);
                                    let name_str = w_name.to_string();
                                    let icon_name = crate::icons::wallet_icon_name(w_name);
                                    let connect_fn = connect_wallet_by_name;

                                    view! {
                                        <div class="siws-wallet-option">
                                            <span style="display: flex; align-items: center; gap: 12px;">
                                                <span style="display: flex; align-items: center; justify-content: center; width: 34px; height: 34px; background: rgba(255,255,255,0.06); border-radius: 10px;">
                                                    <Icon icon=icon_name class="icon-sm" />
                                                </span>
                                                {w_name}
                                            </span>

                                            {if is_installed {
                                                view! {
                                                    <span
                                                        class="siws-badge-installed"
                                                        on:click=move |_| connect_fn(name_str.clone())
                                                    >
                                                        "Connect →"
                                                    </span>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <a
                                                        href=download_url
                                                        target="_blank"
                                                        rel="noopener noreferrer"
                                                        class="siws-badge-install"
                                                        on:click=move |e| e.stop_propagation()
                                                    >
                                                        "Get Extension ↗"
                                                    </a>
                                                }.into_any()
                                            }}
                                        </div>
                                    }
                                }).collect::<Vec<_>>()
                            }}
                        </div>
                    </div>
                </div>
            </Show>
        </Portal>
    }
}
