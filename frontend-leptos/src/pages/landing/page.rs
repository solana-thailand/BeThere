//! The landing page component itself.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::is_admin_role;
use crate::icons::{Icon, IconName};

use super::auth::AuthState;
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
            // One screen, one decision (.plans/038 P1-1, after lu.ma): a
            // headline, one line of value, one button. The audience tabs,
            // stat cards, brand eyebrow and Solana pill are gone: organizers
            // get the host link above the waitlist, Solana is in the footer.
            <section class="landing-hero">
                <h1 class="landing-hero-h1">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.headline_1))}
                    <br />
                    <span class="landing-hero-gradient">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.headline_2))}
                    </span>
                </h1>
                <p class="landing-hero-value">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.value))}
                </p>
                <div class="landing-ctas">
                    {move || {
                        let role = user_role.get();
                        match auth_state.get() {
                            AuthState::SignedIn(_) if is_admin_role(&role) || role == "organizer" => view! {
                                <A href="/admin" attr:class="btn btn-primary landing-cta-link">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.cta.dashboard))}
                                </A>
                            }.into_any(),
                            AuthState::SignedIn(_) if role == "staff" => view! {
                                <A href="/staff" attr:class="btn btn-primary landing-cta-link">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.cta.scanner))}
                                </A>
                            }.into_any(),
                            _ => view! {
                                <a href="#events" class="btn btn-primary landing-cta-link">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.cta.find_events))}
                                </a>
                            }.into_any(),
                        }
                    }}
                </div>
            </section>

            // ===== My Registrations (signed in) — straight under the hero =====
            <MyRegistrations />

            // ===== Upcoming Events (two cards + see all) =====
            <UpcomingEvents />

            // ===== How It Works — three steps on one line =====
            <section id="how-it-works" class="landing-section">
                <div class="landing-section-header">
                    <h2 class="landing-h2">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.title))}
                    </h2>
                </div>
                <ol class="landing-how-row">
                    <li class="landing-how-step">
                        <Icon icon=IconName::Ticket class="icon-md"/>
                        <span>{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s1_title))}</span>
                    </li>
                    <li class="landing-how-step">
                        <Icon icon=IconName::QrCode class="icon-md"/>
                        <span>{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s2_title))}</span>
                    </li>
                    <li class="landing-how-step">
                        <Icon icon=IconName::Recycle class="icon-md"/>
                        <span>{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s4_title))}</span>
                    </li>
                </ol>
            </section>

            // ===== Organizers: one line (F1-d) =====
            // Organizers go straight to /admin. Everyone else gets the same
            // line as a disclosure over the waitlist: /admin only checks sign-in,
            // not role, so for a would-be organizer it would be a dead end.
            <section id="waitlist" class="landing-host-line">
                {move || {
                    let role = user_role.get();
                    match auth_state.get() {
                        AuthState::SignedIn(_) if is_admin_role(&role) || role == "organizer" => view! {
                            <A href="/admin" attr:class="landing-host-link">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.host_cta))}
                            </A>
                        }.into_any(),
                        signed_in => {
                            let signed_in = matches!(signed_in, AuthState::SignedIn(_));
                            view! {
                                <details class="landing-host-details">
                                    <summary class="landing-host-link">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.host_cta))}
                                    </summary>
                                    <div class="landing-waitlist-inner">
                                        {match signed_in {
                                            true => view! {
                                                <div class="landing-waitlist-signed-in">
                                                    <p class="landing-faq-a">
                                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.signed_in))}
                                                    </p>
                                                    <a
                                                        href="https://x.com/ozoneRatchapon"
                                                        target="_blank"
                                                        rel="noopener noreferrer"
                                                        class="btn btn-outline btn-sm"
                                                    >
                                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.dm))}
                                                    </a>
                                                </div>
                                            }.into_any(),
                                            false => view! { <WaitlistForm /> }.into_any(),
                                        }}
                                    </div>
                                </details>
                            }.into_any()
                        }
                    }
                }}
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
                        <a href="#how-it-works">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.how))}</a>
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
