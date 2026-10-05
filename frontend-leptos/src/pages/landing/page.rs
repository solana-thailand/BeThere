//! The landing page component itself.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::icons::{Icon, IconName};

use super::auth::AuthState;
use super::hero::Hero;
use super::how::HowItWorks;
use super::join::Join;
use super::nav::SiteHeader;
use super::registrations::MyRegistrations;
use super::sofar::SoFar;
use super::sponsors::Sponsors;
use super::stats::provide_landing_stats;
use super::theme::initial_theme;
use super::upcoming::UpcomingEvents;

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
    // Auth state for nav bar
    let (auth_state, set_auth_state) = signal(AuthState::Checking);
    let (user_role, set_user_role) = signal(String::new());
    let theme = RwSignal::new(initial_theme());
    // One stats fetch for every section that shows a number (rule 1).
    provide_landing_stats();

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
        <div class="landing-page lp" data-theme=move || theme.get().as_str()>

            // ===== Nav Bar =====
            <SiteHeader auth_state=auth_state user_role=user_role theme=theme />

            // ===== Hero (build plan 0.5, .plans/043 L2) =====
            <Hero auth_state=auth_state user_role=user_role />

            // ===== My Registrations (signed in) — straight under the hero =====
            <MyRegistrations />

            // ===== Upcoming Events (two cards + see all) =====
            <UpcomingEvents />

            // ===== How it works: the swimlane (.plans/043 L4) =====
            <HowItWorks />

            // ===== So far: the numbers from the system (.plans/043 L5) =====
            <SoFar />

            // ===== Sponsors: where a logo goes (.plans/043 L6) =====
            <Sponsors />

            // ===== Join: share, and the organizer card (.plans/043 L7) =====
            <Join auth_state=auth_state user_role=user_role />

            // ===== Footer =====
            <footer class="landing-footer">
                <div class="landing-footer-grid">

                    // Column 1 — Brand + social proof
                    <div class="landing-footer-col">
                        <span class="landing-footer-brand-name landing-brand-gradient">
                            "BeThere"
                        </span>
                        <div class="landing-footer-brand-tagline">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.tagline))}
                        </div>
                        <div class="landing-footer-built-with">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.built_with))}
                            <span class="landing-footer-crab"><Icon icon=IconName::Crab class="icon-sm"/></span>
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.rust_solana))}
                        </div>
                        <div class="landing-footer-trust">
                            // A status line, not a promise: real deposits are THB the
                            // organizer holds; the escrow runs on devnet only.
                            <span class="landing-footer-trust-icon"><Icon icon=IconName::Info class="icon-xs"/></span>
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.deposit_status))}
                        </div>
                        <a
                            href="https://github.com/solana-thailand"
                            target="_blank"
                            rel="noopener noreferrer"
                            class="landing-footer-partner"
                        >
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.partner))}
                        </a>
                    </div>

                    // Column 2 — Product
                    <div class="landing-footer-col">
                        <h4>{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.product))}</h4>
                        <a href="#how">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.how))}</a>
                        <a href="/faq">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.faq))}</a>
                        <A href="/login">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.staff_portal))}</A>
                    </div>

                    // Column 3 — Community
                    <div class="landing-footer-col">
                        <h4>{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.community))}</h4>
                        <a href="https://x.com/ozoneRatchapon" target="_blank" rel="noopener noreferrer">"X / Twitter"</a>
                        <a href="https://github.com/solana-thailand/BeThere" target="_blank" rel="noopener noreferrer">"GitHub"</a>
                    </div>

                </div>

                // Bottom row
                <div class="landing-footer-bottom">
                    <span class="landing-footer-copy">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.copyright))}</span>
                    // Which build is live (.plans/038 P2-f): set by build.sh,
                    // absent from builds that do not know their commit.
                    {option_env!("BETHERE_GIT_SHA").filter(|sha| !sha.is_empty()).map(|sha| view! {
                        <span class="landing-footer-version">{format!("v{} · {sha}", env!("CARGO_PKG_VERSION"))}</span>
                    })}
                </div>
            </footer>

        </div>
    }
}
