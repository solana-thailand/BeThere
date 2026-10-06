//! The landing page component itself.

use leptos::prelude::*;
use leptos_router::components::A;

use super::auth::AuthState;
use super::header::{LandingHeader, SideIndex};
use super::hero::Hero;
use super::how::HowItWorks;
use super::join::Join;
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
            <LandingHeader auth_state=auth_state theme=theme />
            <SideIndex />

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

            // ===== Footer (.plans/043): one row of links, one of fine print =====
            <footer class="lp-footer">
                <div class="lp-wrap">
                    <div class="lp-frow">
                        <a class="lp-flogo" href="/">"BeThere"</a>
                        <nav class="lp-flinks">
                            <a href="#how">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.how))}</a>
                            <a href="/faq">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.faq))}</a>
                            <A href="/login">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.staff_portal))}</A>
                            <a href="https://discord.gg/PGbUgNmsns" target="_blank" rel="noopener noreferrer">"Discord"</a>
                            <a href="https://x.com/ozoneRatchapon" target="_blank" rel="noopener noreferrer">"X"</a>
                            <a href="https://github.com/solana-thailand/BeThere" target="_blank" rel="noopener noreferrer">"GitHub"</a>
                        </nav>
                    </div>
                    <div class="lp-fine">
                        <span>
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.copyright))}
                            " · "
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.partner))}
                        </span>
                        // A status line, not a promise: real deposits are THB the
                        // organizer holds; the escrow runs on devnet only (0.2).
                        <span>{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.deposit_status))}</span>
                        // Which build is live (.plans/038 P2-f): set by build.sh,
                        // absent from builds that do not know their commit.
                        {option_env!("BETHERE_GIT_SHA").filter(|sha| !sha.is_empty()).map(|sha| view! {
                            <span class="lp-version">{format!("v{} · {sha}", env!("CARGO_PKG_VERSION"))}</span>
                        })}
                    </div>
                </div>
            </footer>

        </div>
    }
}
