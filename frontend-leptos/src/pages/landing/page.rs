//! The landing page component itself.

use crate::utils::deposit_copy::{never_forfeited, thb_refund_window};
use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::is_admin_role;
use crate::i18n::{t, t_string, use_i18n};
use crate::icons::{Icon, IconName};

use super::auth::{AuthState, trigger_landing_oauth};
use super::nav::SiteHeader;
use super::registrations::MyRegistrations;
use super::upcoming::UpcomingEvents;
use super::waitlist::WaitlistForm;

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
    let i18n = use_i18n();
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
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.persona.attendees))}
                    </button>
                    <button
                        class="landing-persona-btn"
                        class:landing-persona-btn--active=move || persona.get() == 1
                        on:click=move |_| set_persona.set(1)
                    >
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.persona.organizers))}
                    </button>
                    // Staff used to exist only in the "How it works" tabs, which
                    // meant the page carried two switchers for one axis that did
                    // not even agree on how many roles there are (`.issues/105`).
                    <button
                        class="landing-persona-btn"
                        class:landing-persona-btn--active=move || persona.get() == 2
                        on:click=move |_| set_persona.set(2)
                    >
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.persona.staff))}
                    </button>
                </div>

                <h1 class="landing-hero-h1">
                    {move || match persona.get() {
                        0 => view! {
                            <>
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.attendee_line1))}
                                <br />
                                <span class="landing-hero-gradient">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.attendee_line2))}
                                </span>
                            </>
                        }.into_any(),
                        1 => view! {
                            <>
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.organizer_line1))}
                                <br />
                                <span class="landing-hero-gradient">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.organizer_line2))}
                                </span>
                            </>
                        }.into_any(),
                        // Staff had no hero of its own before, because it was
                        // not a hero option. Falling through to the organizer
                        // pitch would sell a door scanner on payouts.
                        _ => view! {
                            <>
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.staff_line1))}
                                <br />
                                <span class="landing-hero-gradient">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.staff_line2))}
                                </span>
                            </>
                        }.into_any(),
                    }}
                </h1>
                <p class="landing-hero-desc">
                    {move || match persona.get() {
                        0 => t_string!(i18n, landing.hero.attendee_desc),
                        1 => t_string!(i18n, landing.hero.organizer_desc),
                        _ => t_string!(i18n, landing.hero.staff_desc),
                    }}
                </p>
                // Solana pill badge
                <div class="solana-pill">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.built_on_solana))}
                    <Icon icon=IconName::Solana />
                </div>

                // Platform stats — paper ticket stubs on the night ground
                <div class="landing-stat-stubs">
                    <div class="landing-stat-stub">
                        <div class="landing-stat-stub-value stub-green">"100%"</div>
                        <div class="landing-stat-stub-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.stats.back))}</div>
                    </div>
                    <div class="landing-stat-stub">
                        <div class="landing-stat-stub-value stub-poppy">"฿0"</div>
                        <div class="landing-stat-stub-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.stats.cost))}</div>
                    </div>
                    <div class="landing-stat-stub">
                        <div class="landing-stat-stub-value">"< 1s"</div>
                        <div class="landing-stat-stub-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.stats.qr))}</div>
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
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.cta.dashboard))}
                                    </A>
                                }.into_any()
                            }
                            AuthState::SignedIn(_) if role == "staff" => {
                                view! {
                                    <A href="/staff" attr:class="btn btn-primary landing-cta-link">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.cta.scanner))}
                                    </A>
                                }.into_any()
                            }
                            AuthState::SignedIn(_) => {
                                view! {
                                    <a href="#events" class="btn btn-primary landing-cta-link">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.cta.find_events))}
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
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.cta.create_event))}
                                    </button>
                                }.into_any()
                            }
                            _ => {
                                // Attendee persona — primary = find events, secondary = create event
                                view! {
                                    <a href="#events" class="btn btn-primary landing-cta-link">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.cta.find_events))}
                                    </a>
                                    <button
                                        class="btn btn-outline landing-cta-link"
                                        on:click=move |_| trigger_landing_oauth()
                                    >
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.cta.create_event))}
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
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.title))}
                    </h2>
                    <p class="landing-subtitle">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.subtitle))}
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
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s1_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s1_desc))}</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--amber">
                                    <Icon icon=IconName::QrCode class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s2_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s2_desc))}</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--indigo">
                                    <Icon icon=IconName::Puzzle class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s3_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s3_desc))}</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--green">
                                    <Icon icon=IconName::Recycle class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s4_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.attendee.s4_desc))}</div>
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
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.organizer.s1_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.organizer.s1_desc))}</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--indigo">
                                    <Icon icon=IconName::Chart class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.organizer.s2_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.organizer.s2_desc))}</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--amber">
                                    <Icon icon=IconName::Camera class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.organizer.s3_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.organizer.s3_desc))}</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--green">
                                    <Icon icon=IconName::Coin class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.organizer.s4_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.organizer.s4_desc))}</div>
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
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.staff.s1_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.staff.s1_desc))}</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--amber">
                                    <Icon icon=IconName::QrCode class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.staff.s2_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.staff.s2_desc))}</div>
                                </div>
                            </div>
                            <div class="landing-timeline-step">
                                <div class="landing-timeline-dot landing-timeline-dot--amber">
                                    <Icon icon=IconName::Chain class="icon-sm"/>
                                </div>
                                <div class="landing-timeline-body">
                                    <div class="landing-timeline-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.staff.s3_title))}</div>
                                    <div class="landing-timeline-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.how.staff.s3_desc))}</div>
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
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.title))}
                    </h2>
                    <p class="landing-subtitle">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.subtitle))}
                    </p>
                </div>

                <div class="landing-faq-grid">

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.what_q))}
                        </h3>
                        <p class="landing-faq-a">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.what_a))}
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.wallet_q))}
                        </h3>
                        <p class="landing-faq-a">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.wallet_a))}
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.deposit_q))}
                        </h3>
                        <p class="landing-faq-a">
                            {t!(
                                i18n,
                                landing.faq.deposit_a,
                                refund_window = move || thb_refund_window(i18n.get_locale()),
                                never_forfeited = move || never_forfeited(i18n.get_locale())
                            )}
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.crypto_q))}
                        </h3>
                        <p class="landing-faq-a">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.crypto_a))}
                        </p>
                    </div>

                </div>

                <div class="landing-faq-cta">
                    <a href="#waitlist" class="btn btn-outline landing-faq-cta-link">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.faq.host_cta))}
                    </a>
                </div>
            </section>

            // ===== Waitlist (organizer-focused) =====
            <section id="waitlist" class="landing-section">
                <div class="landing-waitlist-inner">
                    <h2 class="landing-h2">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.title))}
                    </h2>
                    <p class="landing-faq-a">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.desc))}
                    </p>
                    {move || {
                        let state = auth_state.get();
                        let role = user_role.get();
                        match &state {
                            AuthState::SignedIn(_) if is_admin_role(&role) || role == "organizer" => {
                                view! {
                                    <A href="/admin" attr:class="btn btn-primary landing-waitlist-submit">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.cta.dashboard))}
                                    </A>
                                }.into_any()
                            }
                            AuthState::SignedIn(_) => {
                                view! {
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
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.tagline))}
                        </div>
                        <div class="landing-footer-built-with">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.built_with))}
                            <span class="landing-footer-crab"><Icon icon=IconName::Crab class="icon-sm"/></span>
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.rust_solana))}
                        </div>
                        <div class="landing-footer-trust">
                            <span class="landing-footer-trust-icon"><Icon icon=IconName::Lock class="icon-xs"/></span>
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.non_custodial))}
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
                        <a href="#faq">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.faq))}</a>
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
                    <span class="landing-footer-powered">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.built_on_solana))}
                        <Icon icon=IconName::Solana />
                    </span>
                </div>
            </footer>

        </div>
    }
}
