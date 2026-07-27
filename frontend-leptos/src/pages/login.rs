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
use leptos_router::hooks::{use_navigate, use_query_map};

use crate::api;
use crate::auth::get_url_error;
use crate::icons::{Icon, IconName};

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

    // Read the `next` query param — the path the user should return to after
    // sign-in. Set by deep-link entry points (e.g. the deposit page's
    // "session expired" CTA) so the OAuth roundtrip returns them to the page
    // they were trying to use, not the role-based default (/admin, /staff, /).
    let query = use_query_map();
    let next_param = query.get().get("next").map(|s| s.to_string());

    // On mount: check for URL errors, check if already authenticated via cookie
    Effect::new(move |_| {
        // Check for error in URL params (from OAuth callback failures)
        if let Some(err) = get_url_error() {
            log::warn!("[login] error from URL: {err}");
            set_error_msg.set(Some(err));
        }

        // Check if already authenticated via cookie
        let nav = navigate.clone();
        let next_for_redirect = next_param.clone();
        leptos::task::spawn_local(async move {
            match crate::api::get_me().await {
                Ok(me) => {
                    // Prefer explicit `next` param over role-based defaults.
                    // Capture whether we have one before `filter` consumes the
                    // Option — the attendee-redirect check below needs it too.
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

                    // For attendees, try to redirect to their latest registration.
                    // Skip this if the user has an explicit `next` param —
                    // respect the deep-link target over the heuristic.
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

    // Handle login button click.
    // Fetches the Google OAuth URL and redirects the browser.
    // Passes the `next` param as the OAuth `state` so the worker's callback
    // handler redirects back to it after successful authentication.
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

    view! {
        <div class="center-page">
            <div class="container login-center-col">
                // Logo
                <div class="brand-logo">"BeThere"</div>
                <div class="brand-logo-sub">"Proof of Attendance"</div>

                // Title
                <h1 class="claim-title">"Staff Portal"</h1>

                // Subtitle
                <p class="subtitle">
                    "Sign in with Google to access the staff check-in portal."
                </p>

                // Powered by Solana badge
                <div class="powered-badge">
                    <span class="sol-dot"></span>
                    "Powered by Solana"
                </div>

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
                        <Icon icon=IconName::Denied class="icon-md icon-danger" />
                        " "
                        {move || error_msg.get().unwrap_or_default()}
                    </div>
                </Show>

                // Back to landing
                <a href="/" class="login-back-link">
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
        </div>
    }
}
