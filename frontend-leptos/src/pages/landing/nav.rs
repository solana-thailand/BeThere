//! The site header, shared by every page that needs chrome.
//!
//! It lived inline in `page.rs`, which is why `/discover` — built to become the
//! first screen after sign-in — rendered with no wordmark, no menu and no way
//! to sign out (`.issues/100`). Redirecting people there without this would
//! strand them.
//!
//! The in-page anchors are absolute (`/#faq`, not `#faq`) so they still work
//! from a page that does not contain those sections: they navigate home and
//! scroll, rather than doing nothing.
//!
//! Auth state is passed in rather than fetched here. The landing page already
//! loads it for its dashboard button and user card; a header that fetched its
//! own would double every page's calls to `/auth/me`.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::is_admin_role;
use crate::icons::{Icon, IconName};

use super::auth::{AuthState, trigger_landing_oauth, trigger_landing_signout};

#[component]
pub fn SiteHeader(
    auth_state: ReadSignal<AuthState>,
    user_role: ReadSignal<String>,
) -> impl IntoView {
    let (mobile_menu_open, set_mobile_menu_open) = signal(false);
    view! {
        <nav class="landing-nav">
            <div class="landing-nav-inner">
                // A link, not a label. On the landing page it is a no-op, but
                // this header is shared now and on any other page the wordmark
                // was the obvious way home and did nothing (`.issues/107`).
                <a class="landing-nav-brand" href="/">
                    <span class="landing-brand-name landing-brand-gradient">
                        "BeThere"
                    </span>
                </a>
                <div class="landing-nav-links">
                    <a href="/#how-it-works">"How it works"</a>
                    <a href="/#faq">"FAQ"</a>
                    <a href="/#waitlist">"For Organizers"</a>
                    <a href="/past-events">"Past Events"</a>
                </div>
                <div class="landing-nav-right" style="display:flex;align-items:center;gap:8px;">
                    <div class="landing-nav-actions">
                        {move || {
                            let state = auth_state.get();
                            let role = user_role.get();
                            match state {
                                AuthState::NotSignedIn => {
                                    view! {
                                        <button
                                            class="btn btn-outline btn-sm"
                                            on:click=move |_| trigger_landing_oauth()
                                        >
                                            "Sign In"
                                        </button>
                                    }.into_any()
                                }
                                AuthState::SignedIn(email) => {
                                    let clean_email = email.clone();
                                    let short_email = if clean_email.len() > 18 {
                                        format!("{}...", &clean_email[..15])
                                    } else {
                                        clean_email.clone()
                                    };
                                    let avatar_char = clean_email.chars().next().unwrap_or('?').to_uppercase().to_string();
                                    view! {
                                        <A href="/profile" attr:class="landing-user-badge" attr:style="display:flex;align-items:center;gap:6px;background:rgba(20,241,149,0.1);border:1px solid rgba(20,241,149,0.3);padding:4px 10px;border-radius:999px;text-decoration:none;color:#fff;font-weight:600;font-size:0.82rem;transition:all 0.2s ease;">
                                            <span class="landing-user-avatar" style="width:22px;height:22px;border-radius:50%;background:#14F195;color:#000;display:inline-flex;align-items:center;justify-content:center;font-weight:800;font-size:0.72rem;">
                                                {avatar_char}
                                            </span>
                                            <span class="landing-email-text hide-mobile" style="max-width:140px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">{short_email}</span>
                                        </A>
                                        {if is_admin_role(&role) {
                                            view! {
                                                <A href="/admin" attr:class="btn btn-outline btn-xs landing-desktop-only-btn">
                                                    "Dashboard"
                                                </A>
                                            }.into_any()
                                        } else if role == "staff" {
                                            view! {
                                                <A href="/staff" attr:class="btn btn-outline btn-xs landing-desktop-only-btn">
                                                    "Scanner"
                                                </A>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                        <button
                                            class="btn btn-outline btn-xs landing-desktop-only-btn"
                                            style="color:#94a3b8;border-color:rgba(255,255,255,0.15);"
                                            on:click=move |_| trigger_landing_signout()
                                            title="Sign Out"
                                        >
                                            "Sign Out"
                                        </button>
                                    }.into_any()
                                }
                                AuthState::Checking => ().into_any(),
                            }
                        }}
                    </div>
                    // Hamburger button — visible only on mobile
                    <button
                        class="landing-nav-hamburger"
                        on:click=move |_| set_mobile_menu_open.update(|v| *v = !*v)
                    >
                        {move || {
                            let open = mobile_menu_open.get();
                            if open {
                                view! {
                                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                                        <line x1="18" y1="6" x2="6" y2="18"></line>
                                        <line x1="6" y1="6" x2="18" y2="18"></line>
                                    </svg>
                                }.into_any()
                            } else {
                                view! {
                                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                                        <line x1="3" y1="6" x2="21" y2="6"></line>
                                        <line x1="3" y1="12" x2="21" y2="12"></line>
                                        <line x1="3" y1="18" x2="21" y2="18"></line>
                                    </svg>
                                }.into_any()
                            }
                        }}
                    </button>
                </div>
            </div>
            // Mobile dropdown menu
            {move || {
                let open = mobile_menu_open.get();
                if open {
                    view! {
                        <div class="landing-nav-mobile-menu">
                            <a href="/#how-it-works" on:click=move |_| set_mobile_menu_open.set(false)>"How it works"</a>
                            <a href="/#faq" on:click=move |_| set_mobile_menu_open.set(false)>"FAQ"</a>
                            <a href="/#waitlist" on:click=move |_| set_mobile_menu_open.set(false)>"For Organizers"</a>
                            <a href="/past-events" on:click=move |_| set_mobile_menu_open.set(false)>"Past Events"</a>
                            <A href="/profile" on:click=move |_| set_mobile_menu_open.set(false) attr:style="display:flex;align-items:center;gap:8px;">
                                <Icon icon=IconName::User class="icon-sm" />
                                "Developer Profile"
                            </A>
                            {move || match auth_state.get() {
                                AuthState::NotSignedIn | AuthState::Checking => {
                                    view! {
                                        <button
                                            class="btn btn-outline btn-sm landing-mobile-signout"
                                            on:click=move |_| {
                                                set_mobile_menu_open.set(false);
                                                trigger_landing_oauth();
                                            }
                                        >
                                            "Sign In"
                                        </button>
                                    }.into_any()
                                }
                                AuthState::SignedIn(email) => {
                                    let role = user_role.get();
                                    view! {
                                        {if is_admin_role(&role) {
                                            view! {
                                                <A href="/admin" on:click=move |_| set_mobile_menu_open.set(false) attr:style="display:flex;align-items:center;gap:8px;">
                                                    <Icon icon=IconName::Chart class="icon-sm" />
                                                    "Dashboard"
                                                </A>
                                            }.into_any()
                                        } else if role == "staff" {
                                            view! {
                                                <A href="/staff" on:click=move |_| set_mobile_menu_open.set(false) attr:style="display:flex;align-items:center;gap:8px;">
                                                    <Icon icon=IconName::Camera class="icon-sm" />
                                                    "Scanner"
                                                </A>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                        <div class="landing-mobile-divider" style="padding-top:8px;">
                                            <span class="landing-email-text">{email}</span>
                                        </div>
                                        <button
                                            class="btn btn-outline btn-sm landing-mobile-signout"
                                            on:click=move |_| {
                                                set_mobile_menu_open.set(false);
                                                trigger_landing_signout();
                                            }
                                        >
                                            "Sign Out"
                                        </button>
                                    }.into_any()
                                }
                            }}
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}
        </nav>

    }
}
