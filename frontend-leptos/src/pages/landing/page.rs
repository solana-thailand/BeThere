//! The landing page component itself.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::is_admin_role;
use crate::icons::{Icon, IconName};

use super::auth::{AuthState, trigger_landing_oauth, trigger_landing_signout};
use super::registrations::MyRegistrations;
use super::upcoming::UpcomingEvents;
use super::waitlist::WaitlistForm;

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
    let (mobile_menu_open, set_mobile_menu_open) = signal(false);

    // Auth state for nav bar
    let (auth_state, set_auth_state) = signal(AuthState::Checking);
    let (user_role, set_user_role) = signal(String::new());

    // Persona toggle: 0 = Attendees, 1 = Organizers
    let (persona, set_persona) = signal(0u8);
    // Feature tab: 0 = Attendee, 1 = Organizer, 2 = Staff
    let (feature_tab, set_feature_tab) = signal(0u8);

    // Sync feature tab when persona changes
    Effect::new(move |_| {
        let p = persona.get();
        if p <= 1 {
            set_feature_tab.set(p);
        }
    });

    // Check auth on mount
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window
                .location()
                .origin()
                .unwrap_or_else(|_| "http://localhost:8787".to_string());
            let url = format!("{origin}/api/auth/me");
            match crate::api::fetch::get(&url, &[]).await {
                Ok(resp) if resp.status() == 200 => {
                    if let Ok(data) = crate::api::fetch::response_json::<serde_json::Value>(&resp).await {
                        let email = data["data"]["email"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let role = data["data"]["role"]
                            .as_str()
                            .unwrap_or("attendee")
                            .to_string();
                        if !email.is_empty() {
                            log::info!("[landing] user signed in: {email} ({role})");
                            set_auth_state.set(AuthState::SignedIn(email));
                            set_user_role.set(role);
                        } else {
                            set_auth_state.set(AuthState::NotSignedIn);
                        }
                    } else {
                        set_auth_state.set(AuthState::NotSignedIn);
                    }
                }
                _ => {
                    set_auth_state.set(AuthState::NotSignedIn);
                }
            }
        });
    });

    view! {
        <div class="landing-page">

            // ===== Nav Bar =====
            <nav class="landing-nav">
                <div class="landing-nav-inner">
                    <div class="landing-nav-brand">
                        <span class="landing-brand-name landing-brand-gradient">
                            "BeThere"
                        </span>
                    </div>
                    <div class="landing-nav-links">
                        <a href="#how-it-works">"How it works"</a>
                        <a href="#faq">"FAQ"</a>
                        <a href="#waitlist">"For Organizers"</a>
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
                                <a href="#how-it-works" on:click=move |_| set_mobile_menu_open.set(false)>"How it works"</a>
                                <a href="#faq" on:click=move |_| set_mobile_menu_open.set(false)>"FAQ"</a>
                                <a href="#waitlist" on:click=move |_| set_mobile_menu_open.set(false)>"For Organizers"</a>
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

            // ===== Hero =====
            <section class="landing-hero">


                // BeThere name + tagline
                <div class="landing-hero-brand landing-brand-gradient">
                    "BeThere"
                </div>

                // Persona toggle
                <div class="landing-persona-toggle">
                    <button
                        class="landing-persona-btn"
                        class:landing-persona-btn--active=move || persona.get() == 0
                        on:click=move |_| set_persona.set(0)
                    >
                        "For Attendees"
                    </button>
                    <button
                        class="landing-persona-btn"
                        class:landing-persona-btn--active=move || persona.get() == 1
                        on:click=move |_| set_persona.set(1)
                    >
                        "For Organizers"
                    </button>
                </div>

                <h1 class="landing-hero-h1">
                    {move || if persona.get() == 0 {
                        view! {
                            <>
                                "Commit. Show up."
                                <br />
                                <span class="landing-hero-gradient">
                                    "Get your money back."
                                </span>
                            </>
                        }.into_any()
                    } else {
                        view! {
                            <>
                                "No-shows cost you money."
                                <br />
                                <span class="landing-hero-gradient">
                                    "Fix it with deposits."
                                </span>
                            </>
                        }.into_any()
                    }}
                </h1>
                <p class="landing-hero-desc">
                    {move || if persona.get() == 0 {
                        "Put down a deposit to reserve your spot. Show up, check in, and get every cent back — take a quick quiz to unlock a digital badge you own forever.".to_string()
                    } else {
                        "Set a deposit for your event. Track check-ins live. No-shows auto-payout to you. Attendees who show up get refunded.".to_string()
                    }}
                </p>
                // Solana pill badge
                <div class="solana-pill">
                    "Built on Solana"
                    <Icon icon=IconName::Solana />
                </div>

                // Platform stats — paper ticket stubs on the night ground
                <div class="landing-stat-stubs">
                    <div class="landing-stat-stub">
                        <div class="landing-stat-stub-value stub-green">"100%"</div>
                        <div class="landing-stat-stub-label">"Refund Guarantee"</div>
                    </div>
                    <div class="landing-stat-stub">
                        <div class="landing-stat-stub-value stub-poppy">"Instant"</div>
                        <div class="landing-stat-stub-label">"PromptPay & Solana Payouts"</div>
                    </div>
                    <div class="landing-stat-stub">
                        <div class="landing-stat-stub-value">"< 1s"</div>
                        <div class="landing-stat-stub-label">"Smart Contract Check-In"</div>
                    </div>
                </div>

                <div class="landing-ctas">
                    {move || {
                        let state = auth_state.get();
                        let role = user_role.get();
                        let p = persona.get();
                        match &state {
                            AuthState::SignedIn(_) if is_admin_role(&role) || role == "organizer" => {
                                view! {
                                    <A href="/admin" attr:class="btn btn-primary landing-cta-link">
                                        "Go to Dashboard →"
                                    </A>
                                }.into_any()
                            }
                            AuthState::SignedIn(_) if role == "staff" => {
                                view! {
                                    <A href="/staff" attr:class="btn btn-primary landing-cta-link">
                                        "Open Scanner →"
                                    </A>
                                }.into_any()
                            }
                            AuthState::SignedIn(_) => {
                                view! {
                                    <a href="#events" class="btn btn-primary landing-cta-link">
                                        "Find Events ↓"
                                    </a>
                                }.into_any()
                            }
                            _ if p == 1 => {
                                // Organizer persona — primary = create event
                                view! {
                                    <button
                                        class="btn btn-primary landing-cta-link"
                                        on:click=move |_| trigger_landing_oauth()
                                    >
                                        "Create an Event →"
                                    </button>
                                }.into_any()
                            }
                            _ => {
                                // Attendee persona — primary = find events, secondary = create event
                                view! {
                                    <a href="#events" class="btn btn-primary landing-cta-link">
                                        "Find Events ↓"
                                    </a>
                                    <button
                                        class="btn btn-outline landing-cta-link"
                                        on:click=move |_| trigger_landing_oauth()
                                    >
                                        "Create an Event →"
                                    </button>
                                }.into_any()
                            }
                        }
                    }}
                </div>
            </section>

            // ===== Upcoming Events =====
            <UpcomingEvents />

            // ===== My Registrations (visible when signed in) =====
            <MyRegistrations />

            // ===== How It Works =====
            <section id="how-it-works" class="landing-section">
                <div class="landing-section-header">
                    <h2 class="landing-h2">
                        "How it works"
                    </h2>
                    <p class="landing-subtitle">
                        "Choose your role to see the experience."
                    </p>
                </div>

                // Tab buttons
                <div class="landing-features-tabs">
                    <button
                        class="landing-features-tab"
                        class:landing-features-tab--active=move || feature_tab.get() == 0
                        on:click=move |_| set_feature_tab.set(0)
                    >
                        "I am an Attendee"
                    </button>
                    <button
                        class="landing-features-tab"
                        class:landing-features-tab--active=move || feature_tab.get() == 1
                        on:click=move |_| set_feature_tab.set(1)
                    >
                        "I am an Organizer"
                    </button>
                    <button
                        class="landing-features-tab"
                        class:landing-features-tab--active=move || feature_tab.get() == 2
                        on:click=move |_| set_feature_tab.set(2)
                    >
                        "I am Event Staff"
                    </button>
                </div>

                // Tab content — vertical timelines
                {move || match feature_tab.get() {
                    0 => view! {
                        <div class="landing-feature-timeline">
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--green">
                                    <Icon icon=IconName::Ticket class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Reserve your spot"</div>
                                    <div class="landing-timeline-desc">"Browse events and pay a deposit to secure your registration. Deposits start from 500 THB or 0.01 SOL."</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--amber">
                                    <Icon icon=IconName::QrCode class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Show your QR at the venue"</div>
                                    <div class="landing-timeline-desc">"Open your ticket on any phone, show the QR code, and get scanned in under 2 seconds. No app needed."</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--indigo">
                                    <Icon icon=IconName::Puzzle class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Complete the brief quest"</div>
                                    <div class="landing-timeline-desc">"After check-in, take a quick, fun quiz on your mobile device. It takes under a minute and confirms your engagement."</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--green">
                                    <Icon icon=IconName::Recycle class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Get your full refund"</div>
                                    <div class="landing-timeline-desc">"Your deposit is refunded on-chain automatically, plus you receive a compressed NFT badge you own forever."</div>
                                </div>
                            </div>
                        </div>
                    }.into_any(),
                    1 => view! {
                        <div class="landing-feature-timeline">
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--indigo">
                                    <Icon icon=IconName::Target class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Set up event & deposit amount"</div>
                                    <div class="landing-timeline-desc">"Create your event, set the deposit stake, and define the staking parameters. Supports THB via PromptPay or SOL/USDC."</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--indigo">
                                    <Icon icon=IconName::Chart class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Monitor real-time registrations"</div>
                                    <div class="landing-timeline-desc">"Track locked deposits and RSVPs on a live dashboard. See exactly who committed — no guesswork."</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--amber">
                                    <Icon icon=IconName::Camera class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Scan check-ins at the venue"</div>
                                    <div class="landing-timeline-desc">"Staff use the mobile scanner portal to verify attendance in under 2 seconds. No app install required."</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--green">
                                    <Icon icon=IconName::Coin class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Keep no-show deposits"</div>
                                    <div class="landing-timeline-desc">"Unclaimed deposits from no-shows are automatically transferred to your organizer ledger. Attendees who showed up get refunded."</div>
                                </div>
                            </div>
                        </div>
                    }.into_any(),
                    _ => view! {
                        <div class="landing-feature-timeline">
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--amber">
                                    <Icon icon=IconName::Camera class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Open scanner on any mobile browser"</div>
                                    <div class="landing-timeline-desc">"No app to install. Open the staff scanner on any smartphone — works in Chrome, Safari, and more."</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--amber">
                                    <Icon icon=IconName::QrCode class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Verify attendee QR code in 1 second"</div>
                                    <div class="landing-timeline-desc">"Point the camera at the attendee's QR code. Instant verification with visual + haptic feedback."</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--amber">
                                    <Icon icon=IconName::Chain class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Instant on-chain ledger confirmation"</div>
                                    <div class="landing-timeline-desc">"Every check-in is recorded on Solana. Manual search fallback available for lost QR codes."</div>
                                </div>
                            </div>
                        </div>
                    }.into_any(),
                }}
            </section>

            // ===== FAQ =====
            <section id="faq" class="landing-section-narrow">
                <div class="landing-section-header">
                    <h2 class="landing-h2">
                        "Frequently asked questions"
                    </h2>
                    <p class="landing-subtitle">
                        "Everything you need to know."
                    </p>
                </div>

                <div class="landing-faq-grid">

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "What is BeThere?"
                        </h3>
                        <p class="landing-faq-a">
                            "A deposit-backed event check-in platform on Solana. Attendees lock a deposit, show up, get scanned, and receive a full refund plus a compressed NFT badge. No-shows forfeit their deposit to the organizer."
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "Do attendees need a crypto wallet?"
                        </h3>
                        <p class="landing-faq-a">
                            "Not to check in! QR scanning works on any phone. A wallet is only needed when claiming the NFT badge and deposit refund afterward."
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "How does the deposit work?"
                        </h3>
                        <p class="landing-faq-a">
                            "Organizers set a deposit amount (e.g., 500 THB / ~$15). After check-in, the deposit is refunded on-chain. No-shows forfeit to the organizer."
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "Is it only for crypto events?"
                        </h3>
                        <p class="landing-faq-a">
                            "It works for any event — meetups, workshops, conferences, hackathons. The blockchain part runs behind the scenes; attendees don't need to know anything about crypto."
                        </p>
                    </div>

                </div>

                <div class="landing-faq-cta">
                    <a href="#waitlist" class="btn btn-outline landing-faq-cta-link">
                        "Want to host events? Learn more ↓"
                    </a>
                </div>
            </section>

            // ===== Waitlist (organizer-focused) =====
            <section id="waitlist" class="landing-section">
                <div class="landing-waitlist-inner">
                    <h2 class="landing-h2">
                        "Bring deposit-backed events to your community"
                    </h2>
                    <p class="landing-faq-a">
                        "Stop losing money to no-shows. Set a deposit, track check-ins live, and auto-refund attendees who show up."
                    </p>
                    {move || {
                        let state = auth_state.get();
                        let role = user_role.get();
                        match &state {
                            AuthState::SignedIn(_) if is_admin_role(&role) || role == "organizer" => {
                                view! {
                                    <A href="/admin" attr:class="btn btn-primary landing-waitlist-submit">
                                        "Go to Dashboard →"
                                    </A>
                                }.into_any()
                            }
                            AuthState::SignedIn(_) => {
                                view! {
                                    <div class="landing-waitlist-signed-in">
                                        <p class="landing-faq-a">
                                            "Signed in! Contact us to get organizer access."
                                        </p>
                                        <a
                                            href="https://x.com/ozoneRatchapon"
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-outline btn-sm"
                                        >
                                            "DM us on X/Twitter"
                                        </a>
                                    </div>
                                }.into_any()
                            }
                            _ => {
                                view! { <WaitlistForm /> }.into_any()
                            }
                        }
                    }}
                </div>
            </section>

            // ===== Footer =====
            <footer class="landing-footer">
                <div class="landing-footer-grid">

                    // Column 1 — Brand + social proof
                    <div class="landing-footer-col">
                        <span class="landing-footer-brand-name landing-brand-gradient">
                            "BeThere"
                        </span>
                        <div class="landing-footer-brand-tagline">
                            "Show up. Get refunded."
                        </div>
                        <div class="landing-footer-built-with">
                            "Built with "
                            <span class="landing-footer-crab"><Icon icon=IconName::Crab class="icon-sm"/></span>
                            " Rust & Solana"
                        </div>
                        <div class="landing-footer-trust">
                            <span class="landing-footer-trust-icon"><Icon icon=IconName::Lock class="icon-xs"/></span>
                            "Non-custodial & secure"
                        </div>
                        <a
                            href="https://github.com/solana-thailand"
                            target="_blank"
                            rel="noopener noreferrer"
                            class="landing-footer-partner"
                        >
                            "Alpha partner: Solana Developer Thailand"
                        </a>
                    </div>

                    // Column 2 — Product
                    <div class="landing-footer-col">
                        <h4>"Product"</h4>
                        <a href="#how-it-works">"How It Works"</a>
                        <a href="#faq">"FAQ"</a>
                        <A href="/login">"Staff Portal"</A>
                    </div>

                    // Column 3 — Community
                    <div class="landing-footer-col">
                        <h4>"Community"</h4>
                        <a href="https://x.com/ozoneRatchapon" target="_blank" rel="noopener noreferrer">"X / Twitter"</a>
                        <a href="https://github.com/solana-thailand/BeThere" target="_blank" rel="noopener noreferrer">"GitHub"</a>
                    </div>

                </div>

                // Bottom row
                <div class="landing-footer-bottom">
                    <span class="landing-footer-copy">"© 2026 BeThere. All rights reserved."</span>
                    <span class="landing-footer-powered">
                        "Built on Solana"
                        <Icon icon=IconName::Solana />
                    </span>
                </div>
            </footer>

        </div>
    }
}
