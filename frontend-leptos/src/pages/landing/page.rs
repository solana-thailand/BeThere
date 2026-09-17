//! The landing page component itself.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::is_admin_role;
use crate::icons::{Icon, IconName};

use super::auth::{AuthState, trigger_landing_oauth};
use super::nav::SiteHeader;
use super::registrations::MyRegistrations;
use super::upcoming::UpcomingEvents;
use super::waitlist::WaitlistForm;

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
    // Auth state for nav bar
    let (auth_state, set_auth_state) = signal(AuthState::Checking);
    let (user_role, set_user_role) = signal(String::new());

    // Persona toggle: 0 = Attendees, 1 = Organizers
    // The page's one role switcher: 0 = Attendee, 1 = Organizer, 2 = Staff.
    //
    // There used to be two — this pill and a separate tab row in "How it works"
    // — on the same axis, with their own signals and a one-way sync between
    // them, and they did not agree on how many roles exist (two against three).
    // A reader who chose a side at the top had to choose again 800px later
    // (`.issues/105`).
    let (persona, set_persona) = signal(0u8);

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
                    if let Ok(data) =
                        crate::api::fetch::response_json::<serde_json::Value>(&resp).await
                    {
                        let email = data["data"]["email"].as_str().unwrap_or("").to_string();
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
            <SiteHeader auth_state=auth_state user_role=user_role />

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
                    // Staff used to exist only in the "How it works" tabs, which
                    // meant the page carried two switchers for one axis that did
                    // not even agree on how many roles there are (`.issues/105`).
                    <button
                        class="landing-persona-btn"
                        class:landing-persona-btn--active=move || persona.get() == 2
                        on:click=move |_| set_persona.set(2)
                    >
                        "For Event Staff"
                    </button>
                </div>

                <h1 class="landing-hero-h1">
                    {move || match persona.get() {
                        0 => view! {
                            <>
                                "Commit. Show up."
                                <br />
                                <span class="landing-hero-gradient">
                                    "Get your money back."
                                </span>
                            </>
                        }.into_any(),
                        1 => view! {
                            <>
                                "No-shows cost you money."
                                <br />
                                <span class="landing-hero-gradient">
                                    "Fix it with deposits."
                                </span>
                            </>
                        }.into_any(),
                        // Staff had no hero of its own before, because it was
                        // not a hero option. Falling through to the organizer
                        // pitch would sell a door scanner on payouts.
                        _ => view! {
                            <>
                                "Scan. Check in."
                                <br />
                                <span class="landing-hero-gradient">
                                    "Under two seconds."
                                </span>
                            </>
                        }.into_any(),
                    }}
                </h1>
                <p class="landing-hero-desc">
                    {move || match persona.get() {
                        0 => "Put down a deposit to reserve your spot. Show up, check in, and get every cent back — take a quick quiz to unlock a digital badge you own forever.".to_string(),
                        1 => "Set a deposit for your event. Track check-ins live. Attendees who show up get their deposit back.".to_string(),
                        _ => "Open the scanner on any phone, point it at an attendee's QR code, and the check-in is recorded. No app to install, no training.".to_string(),
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
                        <div class="landing-stat-stub-value stub-poppy">"฿0"</div>
                        <div class="landing-stat-stub-label">"Cost To Attend"</div>
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

                // No tab row here any more. The hero pill is the page's one
                // role switcher; this section follows it (`.issues/105`).

                // Tab content — vertical timelines
                {move || match persona.get() {
                    0 => view! {
                        <div class="landing-feature-timeline">
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--green">
                                    <Icon icon=IconName::Ticket class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">"Reserve your spot"</div>
                                    <div class="landing-timeline-desc">"Browse events and reserve your place with the event’s configured THB or USDC deposit. You’ll see the exact amount and payment method before confirming."</div>
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
                                    <div class="landing-timeline-desc">"Your deposit comes back after the event, as a refund or as credit for next time, plus a compressed NFT badge you own forever."</div>
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
                                    <div class="landing-timeline-desc">"Create your event, choose the deposit and refund rules, and show attendees the exact THB or USDC amount before they confirm."</div>
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
                        "Stop losing seats to no-shows. Set a deposit, track check-ins live, and give attendees their deposit back when they show up."
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
