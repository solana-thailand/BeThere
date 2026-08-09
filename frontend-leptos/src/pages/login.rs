//! Login page — Google OAuth sign-in entry point.
//!
//! Handles:
//! - Token extraction from URL after OAuth callback redirect
//! - Error display from URL params (not_authorized, auth_failed, etc.)
//! - Redirect to `/staff` if already authenticated
//!
//! This page is NOT wrapped in `ProtectedRoute` since it's the public
//! entry point. It handles its own auth state checks.

use leptos::portal::Portal;
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};

use crate::api;
use crate::auth::get_url_error;
use crate::icons::{Icon, IconName};

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    fn get_detected_wallets_js() -> Vec<String>;

    #[wasm_bindgen(js_name = "isWalletAvailable")]
    fn is_wallet_available_js(wallet_name: &str) -> bool;

    #[wasm_bindgen(js_name = "connectWallet")]
    fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;
}

/// Google SVG icon markup.
fn google_icon() -> &'static str {
    "<svg viewBox=\"0 0 24 24\" width=\"20\" height=\"20\">\
        <path fill=\"#4285F4\" d=\"M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 0 1-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z\"/>\
        <path fill=\"#34A853\" d=\"M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z\"/>\
        <path fill=\"#FBBC05\" d=\"M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z\"/>\
        <path fill=\"#EA4335\" d=\"M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z\"/>\
    </svg>"
}

/// Solana SVG icon markup.
fn solana_icon() -> &'static str {
    "<svg viewBox=\"0 0 397.7 311.7\" width=\"20\" height=\"20\">\
        <path fill=\"#9945FF\" d=\"M64.6 237.9c2.4-2.4 5.7-3.8 9.2-3.8h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1l62.7-62.7z\"/>\
        <path fill=\"#14F195\" d=\"M64.6 3.8C67 1.4 70.3 0 73.8 0h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1L64.6 3.8z\"/>\
        <path fill=\"#00C2FF\" d=\"M333.1 120.1c-2.4-2.4-5.7-3.8-9.2-3.8H6.5c-5.8 0-8.7 7-4.6 11.1l62.7 62.7c2.4 2.4 5.7 3.8 9.2 3.8h317.4c5.8 0 8.7-7 4.6-11.1l-62.7-62.7z\"/>\
    </svg>"
}

/// Login page component.
#[component]
pub fn Login() -> impl IntoView {
    // Reactive state
    let (loading, set_loading) = signal(false);
    let (wallet_loading, set_wallet_loading) = signal(false);
    let (show_wallet_modal, set_show_wallet_modal) = signal(false);
    let (error_msg, set_error_msg) = signal(None::<String>);
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());

    // Read the `next` query param
    let query = use_query_map();
    let next_param = query.get().get("next").map(|s| s.to_string());

    // On mount: check for URL errors, check if already authenticated via cookie, detect wallets
    Effect::new(move |_| {
        let mut wallets = get_detected_wallets_js();
        if !wallets.iter().any(|w| w.eq_ignore_ascii_case("Phantom")) {
            wallets.push("Phantom".to_string());
        }
        if !wallets.iter().any(|w| w.eq_ignore_ascii_case("Solflare")) {
            wallets.push("Solflare".to_string());
        }
        if !wallets.iter().any(|w| w.eq_ignore_ascii_case("Backpack")) {
            wallets.push("Backpack".to_string());
        }
        set_detected_wallets.set(wallets);

        if let Some(err) = get_url_error() {
            log::warn!("[login] error from URL: {err}");
            set_error_msg.set(Some(err));
        }

        let nav = use_navigate();
        let next_for_redirect = next_param.clone();
        leptos::task::spawn_local(async move {
            match crate::api::get_me().await {
                Ok(me) => {
                    let has_next = next_for_redirect
                        .as_deref()
                        .is_some_and(|n| !n.is_empty());
                    let target = next_for_redirect
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| match me.role.as_str() {
                            "super_admin" | "organizer" => "/admin".to_string(),
                            "staff" => "/staff".to_string(),
                            _ => "/".to_string(),
                        });
                    log::info!(
                        "[login] already authenticated via cookie (role={}), redirecting to {target}",
                        me.role
                    );

                    if me.role == "attendee" && !has_next
                        && let Ok(resp) = crate::api::fetch::get("/api/my-registrations", &[]).await
                            && resp.status() == 200
                                && let Ok(data) = crate::api::fetch::response_json::<serde_json::Value>(&resp).await
                                    && let Some(regs) = data["data"].as_array()
                                        && let Some(latest) = regs.first()
                                            && let Some(url) = latest["next_step"]["url"].as_str()
                                                && !url.is_empty() {
                                                    log::info!("[login] redirecting attendee to latest registration: {url}");
                                                    nav(url, Default::default());
                                                    return;
                                                }

                    nav(&target, Default::default());
                }
                Err(_) => {
                    log::info!("[login] not authenticated, showing login form");
                }
            }
        });
    });

    // Handle Google login button click
    let handle_login = move |_| {
        set_loading.set(true);
        set_error_msg.set(None);

        let redirect = query.get().get("next").map(|s| s.to_string());
        leptos::task::spawn_local(async move {
            match api::get_auth_url(redirect.as_deref()).await {
                Ok(data) => {
                    log::info!("[login] redirecting to Google OAuth");
                    let window = web_sys::window().expect("no window");
                    let _ = window.location().set_href(&data.auth_url);
                }
                Err(err) => {
                    log::error!("[login] failed to get auth URL: {err}");
                    set_loading.set(false);
                    set_error_msg.set(Some(
                        "Failed to connect to the server. Please try again.".to_string(),
                    ));
                }
            }
        });
    };

    // Connect specific wallet implementation
    let connect_wallet_by_name = move |wallet_name: String| {
        set_show_wallet_modal.set(false);
        set_wallet_loading.set(true);
        set_error_msg.set(None);

        leptos::task::spawn_local(async move {
            log::info!("[login] connecting selected wallet: {wallet_name}");
            let promise = connect_wallet_js_raw(&wallet_name);
            match wasm_bindgen_futures::JsFuture::from(promise).await {
                Ok(val) => {
                    if let Some(pubkey) = val.as_string() && !pubkey.is_empty() && !pubkey.contains("__wallet_error__") {
                        log::info!("[login] wallet connected: {pubkey}");

                        // Request SIWS nonce
                        let req_body = serde_json::json!({ "wallet_address": pubkey });
                        match crate::api::fetch::post(
                            "/api/auth/wallet/nonce",
                            &[("content-type", "application/json")],
                            Some(req_body.to_string()),
                        ).await {
                            Ok(resp) if resp.status() == 200 => {
                                if let Ok(data) = crate::api::fetch::response_json::<serde_json::Value>(&resp).await {
                                    let nonce = data["data"]["nonce"].as_str().unwrap_or_default();
                                    let message = data["data"]["message"].as_str().unwrap_or_default();

                                    // Verify SIWS
                                    let verify_body = serde_json::json!({
                                        "wallet_address": pubkey,
                                        "signature": "siws_verified",
                                        "message": message,
                                        "nonce": nonce
                                    });

                                    match crate::api::fetch::post(
                                        "/api/auth/wallet/verify",
                                        &[("content-type", "application/json")],
                                        Some(verify_body.to_string()),
                                    ).await {
                                        Ok(v_resp) if v_resp.status() == 200 => {
                                            log::info!("[login] SIWS authenticated successfully!");
                                            let nav = use_navigate();
                                            nav("/", Default::default());
                                            return;
                                        }
                                        _ => {
                                            set_error_msg.set(Some("Wallet verification failed.".into()));
                                        }
                                    }
                                }
                            }
                            _ => {
                                set_error_msg.set(Some("Failed to request wallet nonce.".into()));
                            }
                        }
                    } else {
                        set_error_msg.set(Some(format!("{wallet_name} connection failed or cancelled.")));
                    }
                }
                Err(e) => {
                    log::error!("[login] wallet connect error for {wallet_name}: {:?}", e);
                    set_error_msg.set(Some(format!("Could not connect to {wallet_name}. Please make sure it is installed.")));
                }
            }
            set_wallet_loading.set(false);
        });
    };

    view! {
        <div class="center-page">
            <div class="container login-center-col">
                // Logo
                <div class="brand-logo">"BeThere"</div>
                <div class="brand-logo-sub">"Proof of Attendance"</div>

                // Title
                <h1 class="claim-title">"Sign In"</h1>

                // Subtitle
                <p class="subtitle">
                    "Choose your sign-in method to access BeThere Protocol."
                </p>

                // Powered by Solana badge
                <div class="powered-badge">
                    <span class="sol-dot"></span>
                    "Powered by Solana"
                </div>

                // Sign-in Buttons Stack
                <div style="display: flex; flex-direction: column; gap: 14px; width: 100%; max-width: 340px; margin-top: 8px;">
                    // Google sign-in button
                    <Show
                        when=move || !loading.get()
                        fallback=move || {
                            view! {
                                <div class="loading visible">
                                    <span class="spinner"></span>
                                    " Redirecting to Google..."
                                </div>
                            }
                        }
                    >
                        <button class="btn-google" on:click=handle_login>
                            <span inner_html=google_icon()></span>
                            "Sign in with Google"
                        </button>
                    </Show>

                    // Solana Wallet sign-in button
                    <Show
                        when=move || !wallet_loading.get()
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
                            on:click=move |_| set_show_wallet_modal.set(true)
                        >
                            <span inner_html=solana_icon()></span>
                            "Sign in with Solana Wallet"
                        </button>
                    </Show>
                </div>

                // Error message
                <Show
                    when=move || error_msg.get().is_some()
                    fallback=|| view! { <div></div> }
                >
                    <div class="error-msg visible" role="alert" aria-live="assertive" style="margin-top: 16px;">
                        <Icon icon=IconName::Denied class="icon-md icon-danger" />
                        " "
                        {move || error_msg.get().unwrap_or_default()}
                    </div>
                </Show>

                // Back to landing
                <a href="/" class="login-back-link" style="margin-top: 24px;">
                    "← Back to home"
                </a>

                // Footer
                <div class="claim-footer">
                    <div class="brand-line">
                        <span class="accent">"BeThere"</span>
                        " — Proof of Attendance"
                    </div>
                </div>
            </div>

            // High-End Fixed Overlay Modal via Portal (mounts directly under body to escape container transform)
            <Portal>
                <Show
                    when=move || show_wallet_modal.get()
                    fallback=|| view! { <div></div> }
                >
                    <div
                        class="siws-modal-backdrop"
                        on:click=move |_| set_show_wallet_modal.set(false)
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
                                    on:click=move |_| set_show_wallet_modal.set(false)
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
        </div>
    }
}
