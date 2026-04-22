//! Login page — Google OAuth sign-in entry point.
//!
//! Handles:
//! - Token extraction from URL after OAuth callback redirect
//! - Error display from URL params (not_authorized, auth_failed, etc.)
//! - Redirect to `/staff` if already authenticated
//!
//! This page is NOT wrapped in `ProtectedRoute` since it's the public
//! entry point. It handles its own auth state checks.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::api;
use crate::auth::get_url_error;

/// Google SVG icon markup.
///
/// Defined as a module-level constant to avoid the `#[component]` macro
/// misinterpreting hex color values like `#4285F4` as Rust tokens.
fn google_icon() -> &'static str {
    "<svg viewBox=\"0 0 24 24\" width=\"20\" height=\"20\">\
        <path fill=\"#4285F4\" d=\"M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 0 1-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z\"/>\
        <path fill=\"#34A853\" d=\"M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z\"/>\
        <path fill=\"#FBBC05\" d=\"M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z\"/>\
        <path fill=\"#EA4335\" d=\"M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z\"/>\
    </svg>"
}

/// Login page component.
#[component]
pub fn Login() -> impl IntoView {
    let navigate = use_navigate();

    // Reactive state
    let (loading, set_loading) = signal(false);
    let (error_msg, set_error_msg) = signal(None::<String>);

    // On mount: check for URL errors, check if already authenticated via cookie
    Effect::new(move |_| {
        // Check for error in URL params (from OAuth callback failures)
        if let Some(err) = get_url_error() {
            log::warn!("[login] error from URL: {err}");
            set_error_msg.set(Some(err));
        }

        // Check if already authenticated via cookie
        let nav = navigate.clone();
        leptos::task::spawn_local(async move {
            match crate::api::get_me().await {
                Ok(_) => {
                    log::info!("[login] already authenticated via cookie, redirecting to /staff");
                    nav("/staff", Default::default());
                }
                Err(_) => {
                    log::info!("[login] not authenticated, showing login form");
                }
            }
        });
    });

    // Handle login button click.
    // Fetches the Google OAuth URL and redirects the browser.
    let handle_login = move |_| {
        set_loading.set(true);
        set_error_msg.set(None);

        leptos::task::spawn_local(async move {
            match api::get_auth_url().await {
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

    view! {
        <div class="center-page">
            <div class="container" style="display:flex;flex-direction:column;align-items:center;">
                // Logo
                <div class="logo">"🎫"</div>

                // Title
                <h1>"Event Check-In"</h1>

                // Subtitle
                <p class="subtitle">
                    "Sign in with your Google account to access the staff check-in portal."
                </p>

                // Event link
                <a
                    href="https://solana-thailand.github.io/genesis/events/road-to-mainnet-1-bangkok/"
                    target="_blank"
                    rel="noopener noreferrer"
                    style="color:var(--primary);font-size:0.9rem;margin-bottom:0.5rem;"
                >
                    "🔗 Road to Mainnet 1 — Bangkok"
                </a>

                // Google sign-in button (hidden when loading)
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

                // Error message
                <Show
                    when=move || error_msg.get().is_some()
                    fallback=|| view! { <div></div> }
                >
                    <div class="error-msg visible" role="alert" aria-live="assertive">
                        {move || error_msg.get().unwrap_or_default()}
                    </div>
                </Show>

                // Footer
                <div class="footer">"Built with 🦀 Rust (Leptos + Axum)"</div>
            </div>
        </div>
    }
}
