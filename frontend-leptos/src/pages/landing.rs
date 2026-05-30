//! Landing page — public marketing page for BeThere.
//!
//! Showcases the platform with hero, problem/solution, how-it-works steps,
//! organizer and attendee pitches, and footer branding.
//! No backend calls — purely static marketing content with SPA navigation.

use leptos::prelude::*;
use leptos_router::components::A;
use serde::Deserialize;

use crate::api::ApiResponse;
use crate::components::is_admin_role;
use crate::icons::{Icon, IconName};

// ---------------------------------------------------------------------------
// Auth state (same pattern as public_event.rs)
// ---------------------------------------------------------------------------

/// Tracks whether the user is signed in on the landing page.
#[derive(Clone, Debug)]
enum AuthState {
    Checking,
    SignedIn(String),
    NotSignedIn,
}

/// Fetches the Google OAuth URL with a redirect back to `/`, then navigates.
fn trigger_landing_oauth() {
    leptos::task::spawn_local(async move {
        let window = web_sys::window().expect("no window");
        let origin = window
            .location()
            .origin()
            .unwrap_or_else(|_| "http://localhost:8787".to_string());
        let api_url = format!(
            "{origin}/api/auth/url?redirect={}",
            urlencoding::encode("/")
        );
        match crate::api::fetch::get(&api_url, &[]).await {
            Ok(resp) => {
                if let Ok(body) = crate::api::fetch::response_text(&resp).await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(auth_url) =
                            json.get("data").and_then(|d| d.get("auth_url")).and_then(|u| u.as_str())
                        {
                            let _ = window.location().set_href(auth_url);
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("[landing] failed to get auth URL: {e}");
            }
        }
        // Fallback: navigate to /login
        let _ = window.location().set_href("/login");
    });
}

/// Sign out: clear cookie + reload.
fn trigger_landing_signout() {
    leptos::task::spawn_local(async move {
        let _ = crate::api::fetch::post("/api/auth/logout", &[], None).await;
        let window = web_sys::window().expect("no window");
        let _ = window.location().reload();
    });
}

/// Waitlist signup form component.
#[component]
fn WaitlistForm() -> impl IntoView {
    let (email, set_email) = signal(String::new());
    let (submitted, set_submitted) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (submitting, set_submitting) = signal(false);
    let (already_registered, set_already_registered) = signal(false);

    let handle_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let email_val = email.get().trim().to_string();

        if email_val.is_empty() || !email_val.contains('@') || !email_val.contains('.') {
            set_error.set(Some("Please enter a valid email".to_string()));
            return;
        }

        set_error.set(None);
        set_submitting.set(true);

        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window.location().origin().unwrap_or("http://localhost:8787".to_string());
            let url = format!("{origin}/api/waitlist");

            let body = serde_json::json!({ "email": email_val });
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            let hdrs = [("Content-Type", "application/json")];

            match crate::api::fetch::post(&url, &hdrs, Some(body_str)).await {
                Ok(response) => {
                    // Parse JSON body regardless of HTTP status
                    let status = response.status();
                    match crate::api::fetch::response_json::<serde_json::Value>(&response).await {
                        Ok(body) => {
                            if body.get("success").and_then(|v| v.as_bool()) == Some(true) {
                                set_submitted.set(true);
                            } else {
                                let error_msg = body
                                    .get("error")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Something went wrong");
                                // Duplicate email — backend returns 400 with "already on the waitlist"
                                if error_msg.contains("already on the waitlist") {
                                    set_already_registered.set(true);
                                } else {
                                    set_error.set(Some(error_msg.to_string()));
                                }
                            }
                        }
                        Err(_) => {
                            if (200..300).contains(&status) {
                                set_submitted.set(true);
                            } else {
                                set_error.set(Some("Something went wrong. Please try again.".to_string()));
                            }
                        }
                    }
                }
                Err(e) => {
                    set_error.set(Some(format!("Network error: {e}")));
                }
            }
            set_submitting.set(false);
        });
    };

    view! {
        <Show
            when=move || submitted.get() || already_registered.get()
            fallback=|| view! { <div></div> }
        >
            <div class="landing-waitlist-success">
                <div class="landing-waitlist-success-icon"><Icon icon=IconName::Check class="icon-md"/></div>
                <div class="landing-waitlist-success-title">
                    {move || if already_registered.get() { "You're already on the list!" } else { "You're on the list!" }}
                </div>
                <div class="landing-waitlist-success-desc">"We'll reach out when we're ready to onboard new events."</div>
            </div>
        </Show>
        <Show
            when=move || !submitted.get() && !already_registered.get()
            fallback=|| view! { <div></div> }
        >
            <form on:submit=handle_submit class="landing-waitlist-form">
                <input
                    type="email"
                    placeholder="your@email.com"
                    prop:value=move || email.get()
                    on:input=move |ev| set_email.set(event_target_value(&ev))
                    disabled=move || submitting.get()
                    class="landing-waitlist-input"
                />
                <button
                    type="submit"
                    disabled=move || submitting.get() || email.get().trim().is_empty()
                    class="btn btn-primary landing-waitlist-submit"
                >
                    {move || if submitting.get() { "Joining..." } else { "Join Waitlist" }}
                </button>
            </form>
            <Show
                when=move || error.get().is_some()
                fallback=|| view! { <div></div> }
            >
                <p class="landing-waitlist-error">
                    {move || error.get().unwrap_or_default()}
                </p>
            </Show>
        </Show>
    }
}

// ── Swimlane Types ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum SwimlaneRole {
    Organizer,
    Staff,
    Attendee,
}

impl SwimlaneRole {
    fn label(self) -> &'static str {
        match self {
            Self::Organizer => "Organizer",
            Self::Staff => "Staff",
            Self::Attendee => "Attendee",
        }
    }

    fn icon_name(self) -> IconName {
        match self {
            Self::Organizer => IconName::Target,
            Self::Staff => IconName::Phone,
            Self::Attendee => IconName::Ticket,
        }
    }

    fn accent(self) -> &'static str {
        match self {
            Self::Organizer => "#6366f1",
            Self::Staff => "#f59e0b",
            Self::Attendee => "#22c55e",
        }
    }

    fn accent_bg(self) -> &'static str {
        match self {
            Self::Organizer => "rgba(99,102,241,0.12)",
            Self::Staff => "rgba(245,158,11,0.12)",
            Self::Attendee => "rgba(34,197,94,0.12)",
        }
    }

    fn accent_border(self) -> &'static str {
        match self {
            Self::Organizer => "rgba(99,102,241,0.35)",
            Self::Staff => "rgba(245,158,11,0.35)",
            Self::Attendee => "rgba(34,197,94,0.35)",
        }
    }

    fn steps(self) -> &'static [SwimlaneStep] {
        static ORG: &[SwimlaneStep] = &[
            SwimlaneStep { icon: IconName::Copy, title: "Create Event", desc: "Set name, capacity, and deposit" },
            SwimlaneStep { icon: IconName::Coin, title: "150 Registered", desc: "Deposits pool to 1.5 SOL + $1,950" },
            SwimlaneStep { icon: IconName::Chart, title: "Live Dashboard", desc: "Track check-ins & no-shows" },
            SwimlaneStep { icon: IconName::MoneyWings, title: "Auto Payout", desc: "Refund attendees, keep no-shows" },
        ];
        static STAFF: &[SwimlaneStep] = &[
            SwimlaneStep { icon: IconName::Camera, title: "Open Scanner", desc: "Point camera at attendee QR" },
            SwimlaneStep { icon: IconName::Check, title: "Instant Confirm", desc: "Verified in < 2 seconds" },
            SwimlaneStep { icon: IconName::Party, title: "Session Done", desc: "142 checked in, all smooth" },
        ];
        static ATT: &[SwimlaneStep] = &[
            SwimlaneStep { icon: IconName::Ticket, title: "Register & Deposit", desc: "Lock 0.01 SOL + $13 USDC" },
            SwimlaneStep { icon: IconName::Phone, title: "Show QR Code", desc: "At venue, display check-in code" },
            SwimlaneStep { icon: IconName::Check, title: "Get Scanned", desc: "Staff scans — instant confirm" },
            SwimlaneStep { icon: IconName::Brain, title: "Quick Quiz", desc: "Prove you paid attention" },
            SwimlaneStep { icon: IconName::Coin, title: "Claim Refund + Badge", desc: "Deposit back + cNFT forever" },
        ];
        match self {
            Self::Organizer => ORG,
            Self::Staff => STAFF,
            Self::Attendee => ATT,
        }
    }
}

struct SwimlaneStep {
    icon: IconName,
    title: &'static str,
    desc: &'static str,
}

/// Render the mockup card for a given role + step index.
fn swimlane_mockup(role: SwimlaneRole, step: usize) -> impl IntoView {
    match role {
        SwimlaneRole::Organizer => match step {
            // Create Event — form card
            0 => view! {
                <div class="landing-mock-card">
                    <div class="landing-mock-h">"New Event"</div>
                    <div class="landing-mock-flex-col">
                        <div class="landing-mock-input">"Solana Bangkok Meetup 2026"</div>
                        <div class="landing-mock-flex-row">
                            <div class="landing-mock-input landing-mock-input--flex landing-mock-grow">
                                <Icon icon=IconName::Pin class="icon-xs"/>" Bangkok"
                            </div>
                            <div class="landing-mock-input landing-mock-grow">"Cap: 200"</div>
                        </div>
                        <div class="landing-mock-input landing-mock-input--accent">
                            "Deposit: 0.01 SOL + $13 USDC"
                        </div>
                    </div>
                    <div class="landing-mock-cta landing-mock-cta--accent">"Create Event"</div>
                </div>
            }.into_any(),
            // Registrations — deposit pool
            1 => view! {
                <div class="landing-mock-card">
                    <div class="landing-mock-h landing-mock-h--icon">
                        <span><Icon icon=IconName::Coin class="icon-sm"/></span>
                        <span>"Deposit Pool"</span>
                    </div>
                    <div class="landing-mock-flex-row-lg">
                        <div>
                            <div class="landing-mock-val--accent">"1.5 SOL"</div>
                            <div class="landing-mock-sub">"+ $1,950 USDC"</div>
                        </div>
                        <div class="landing-mock-grow"></div>
                        <div class="landing-mock-right">
                            <div class="landing-mock-val--heading">"150"</div>
                            <div class="landing-mock-sub">"attendees"</div>
                        </div>
                    </div>
                    <div class="landing-mock-bar landing-mock-bar--accent">
                        <div class="landing-mock-bar-fill--accent"></div>
                    </div>
                    <div class="landing-mock-sub-right">"75% of capacity"</div>
                </div>
            }.into_any(),
            // Dashboard — live stats
            2 => view! {
                <div class="landing-mock-card">
                    <div class="landing-mock-h landing-mock-h--icon">
                        <Icon icon=IconName::Chart class="icon-sm"/>" Live Dashboard"
                    </div>
                    <div class="landing-mock-grid-3">
                        <div class="landing-mock-stat landing-mock-stat--accent">
                            <div class="landing-mock-val--accent">"150"</div>
                            <div class="landing-mock-sub-xs">"registered"</div>
                        </div>
                        <div class="landing-mock-stat landing-mock-stat--success">
                            <div class="landing-mock-val--success">"142"</div>
                            <div class="landing-mock-sub-xs">"checked in"</div>
                        </div>
                        <div class="landing-mock-stat landing-mock-stat--danger">
                            <div class="landing-mock-val--danger">"8"</div>
                            <div class="landing-mock-sub-xs">"no-show"</div>
                        </div>
                    </div>
                    <div class="landing-mock-bar landing-mock-bar--success">
                        <div class="landing-mock-bar-fill--success"></div>
                    </div>
                    <div class="landing-mock-sub-after">"95% attendance"</div>
                </div>
            }.into_any(),
            // Payout — refund + received
            _ => view! {
                <div class="landing-mock-card">
                    <div class="landing-mock-h landing-mock-h--icon">
                        <Icon icon=IconName::MoneyWings class="icon-sm"/>" Payout Summary"
                    </div>
                    <div class="landing-mock-grid-2">
                        <div class="landing-mock-payout landing-mock-payout--success">
                            <div class="landing-mock-payout-label landing-mock-payout-label--success">
                                <Icon icon=IconName::Check class="icon-xs"/>" Refunded"
                            </div>
                            <div class="landing-mock-val">"142"</div>
                            <div class="landing-mock-sub">"1.42 SOL + $1,846"</div>
                        </div>
                        <div class="landing-mock-payout landing-mock-payout--warning">
                            <div class="landing-mock-payout-label landing-mock-payout-label--warning">
                                <Icon icon=IconName::Coin class="icon-xs"/>" You Received"
                            </div>
                            <div class="landing-mock-val">"8"</div>
                            <div class="landing-mock-sub">"0.08 SOL + $104"</div>
                        </div>
                    </div>
                </div>
            }.into_any(),
        },
        SwimlaneRole::Staff => match step {
            // Scan — camera frame
            0 => view! {
                <div class="landing-mock-card landing-mock-card--center">
                    <div class="landing-mock-scan-frame">
                        <div class="landing-mock-scan-corner landing-mock-scan-corner--tl"></div>
                        <div class="landing-mock-scan-corner landing-mock-scan-corner--tr"></div>
                        <div class="landing-mock-scan-corner landing-mock-scan-corner--bl"></div>
                        <div class="landing-mock-scan-corner landing-mock-scan-corner--br"></div>
                        <div class="landing-mock-scan-label">
                            <Icon icon=IconName::Camera class="icon-xs"/>" Point at attendee QR code"
                        </div>
                    </div>
                    <div class="landing-mock-scan-status">"Scanning..."</div>
                </div>
            }.into_any(),
            // Confirmed — success card
            1 => view! {
                <div class="landing-mock-card landing-mock-card--center landing-mock-card--success">
                    <div class="landing-mock-circle">
                        <Icon icon=IconName::Check class="icon-md"/>
                    </div>
                    <div class="landing-mock-val--success landing-mock-mb-xs">"Checked In!"</div>
                    <div class="landing-mock-val">"Alex Chen"</div>
                    <div class="landing-mock-sub">"Solana Bangkok 2026"</div>
                    <div class="landing-mock-sub landing-mock-mt-xs">
                        "Jul 15 \u{00b7} 2:03 PM"
                    </div>
                </div>
            }.into_any(),
            // Done — summary
            _ => view! {
                <div class="landing-mock-card landing-mock-card--center">
                    <div class="landing-mock-circle landing-mock-circle--empty">
                        <Icon icon=IconName::Party class="icon-md"/>
                    </div>
                    <div class="landing-mock-val--heading landing-mock-mb-xs">
                        "Session Complete"
                    </div>
                    <div class="landing-mock-flex-center">
                        <div>
                            <div class="landing-mock-val--warning">"142"</div>
                            <div class="landing-mock-sub-xs">"checked in"</div>
                        </div>
                        <div>
                            <div class="landing-mock-val--heading">"< 2s"</div>
                            <div class="landing-mock-sub-xs">"avg time"</div>
                        </div>
                    </div>
                    <div class="landing-mock-sub">
                        "Lost QR? Search by name \u{2192}"
                    </div>
                </div>
            }.into_any(),
        },
        SwimlaneRole::Attendee => match step {
            // Register & Deposit
            0 => view! {
                <div class="landing-mock-card">
                    <div class="landing-mock-h landing-mock-h--sm">"Solana Bangkok Meetup 2026"</div>
                    <div class="landing-mock-sub landing-mock-mb-md">
                        "Jul 15 \u{00b7} Bangkok, Thailand"
                    </div>
                    <div class="landing-mock-wallet">
                        <span class="landing-mock-wallet-dot">"\u{25cf}"</span>
                        <span class="landing-mock-sub">"Phantom \u{2014} 7xK9...f3Pz"</span>
                    </div>
                    <div class="landing-mock-input landing-mock-input--success">
                        <div class="landing-mock-sub landing-mock-sub--success-bold">
                            "Deposit to lock"
                        </div>
                        <div class="landing-mock-val">"0.01 SOL + $13 USDC"</div>
                    </div>
                    <div class="landing-mock-cta landing-mock-cta--success">"Confirm Deposit"</div>
                </div>
            }.into_any(),
            // Show QR
            1 => view! {
                <div class="landing-mock-card landing-mock-card--center">
                    <div class="landing-mock-qr-frame">
                        <div class="landing-mock-qr-grid">
                            {(0..64).map(|i| view! {
                                <div
                                    class="landing-mock-qr-pixel"
                                    style=format!(
                                        "background:{};",
                                        if i % 3 == 0 { "#000" } else if i % 5 == 0 { "#333" } else { "#fff" }
                                    )
                                ></div>
                            }).collect_view()}
                        </div>
                    </div>
                    <div class="landing-mock-val">"Alex Chen"</div>
                    <div class="landing-mock-sub">"Solana Bangkok 2026"</div>
                </div>
            }.into_any(),
            // Get Scanned
            2 => view! {
                <div class="landing-mock-card landing-mock-card--center landing-mock-card--success">
                    <div class="landing-mock-circle">
                        <Icon icon=IconName::Check class="icon-md"/>
                    </div>
                    <div class="landing-mock-val--success landing-mock-mb-xs">
                        "Checked In!"
                    </div>
                    <div class="landing-mock-sub">"Jul 15, 2026 \u{00b7} 2:03 PM"</div>
                    <div class="landing-mock-sub landing-mock-mt-xxs">
                        "Solana Bangkok Meetup"
                    </div>
                </div>
            }.into_any(),
            // Quiz
            3 => view! {
                <div class="landing-mock-card">
                    <div class="landing-mock-h landing-mock-h--sm landing-mock-h--icon">
                        <Icon icon=IconName::Brain class="icon-sm"/>" Event Quiz"
                    </div>
                    <div class="landing-mock-sub landing-mock-mb-sm">
                        "What does BeThere use to prove attendance?"
                    </div>
                    <div class="landing-mock-flex-col">
                        <div class="landing-mock-quiz-opt">"\u{25cb} PDF certificate"</div>
                        <div class="landing-mock-quiz-opt landing-mock-quiz-opt--selected">
                            "\u{25cf} Compressed NFT badge"
                        </div>
                        <div class="landing-mock-quiz-opt">"\u{25cb} Email receipt"</div>
                        <div class="landing-mock-quiz-opt">"\u{25cb} Paper ticket"</div>
                    </div>
                </div>
            }.into_any(),
            // Claim Refund + Badge
            _ => view! {
                <div class="landing-mock-card landing-mock-card--center">
                    <div class="landing-mock-badge">
                        <div class="landing-mock-badge-title">
                            <Icon icon=IconName::Ticket class="icon-sm"/>" BeThere"
                        </div>
                        <div class="landing-mock-badge-sub">"cNFT \u{00b7} Solana"</div>
                    </div>
                    <div class="landing-mock-claim-title">
                        <Icon icon=IconName::Coin class="icon-sm"/>" Refund Claimed!"
                    </div>
                    <div class="landing-mock-sub">"0.01 SOL + $13 USDC returned"</div>
                    <div class="landing-mock-cta landing-mock-cta--success">"Claim to Wallet"</div>
                </div>
            }.into_any(),
        },
    }
}

/// Lightweight event item from the public events API.
#[derive(Clone, Deserialize)]
struct PublicEventItem {
    name: String,
    slug: String,
    event_start_ms: i64,
    #[serde(default)]
    time_tba: bool,
    deposit_enabled: bool,
    #[serde(default)]
    tagline: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    nft_image_url: String,
}

#[derive(Clone, Deserialize)]
#[derive(Default)]
struct PublicEventsResponse {
    events: Vec<PublicEventItem>,
}

/// Upcoming Events section — fetches active events and displays them.
#[component]
fn UpcomingEvents() -> impl IntoView {
    let (events, set_events) = signal(Vec::<PublicEventItem>::new());
    let (loaded, set_loaded) = signal(false);

    // Fetch events on mount
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window
                .location()
                .origin()
                .unwrap_or_else(|_| "http://localhost:8787".to_string());
            let url = format!("{origin}/api/public/events");

            match crate::api::fetch::get(&url, &[]).await {
                Ok(resp) if resp.status() == 200 => {
                    match crate::api::fetch::response_json::<ApiResponse<PublicEventsResponse>>(&resp).await {
                        Ok(wrapper) => {
                            if let Some(data) = wrapper.data {
                                set_events.set(data.events);
                            }
                        }
                        Err(e) => {
                            log::warn!("[landing] failed to parse events: {e}");
                        }
                    }
                }
                Ok(_) => {
                    log::warn!("[landing] events API returned non-200");
                }
                Err(e) => {
                    log::warn!("[landing] events fetch error: {e}");
                }
            }
            set_loaded.set(true);
        });
    });

    view! {
        {move || {
            let evts = events.get();
            let is_loaded = loaded.get();
            let heading = view! {
                <div class="landing-section-header-sm">
                    <h2 class="landing-h2">
                        <Icon icon=IconName::Party class="icon-sm"/>" Upcoming Events"
                    </h2>
                    <p class="landing-subtitle">
                        "Reserve your spot with a deposit. Show up. Get refunded."
                    </p>
                </div>
            };
            if !is_loaded {
                // Still loading — show heading + spinner
                view! {
                    <section id="events" class="landing-section-sm">
                        {heading}
                        <div class="landing-events-loading">
                            <span class="landing-events-loading-spinner"></span>
                            <p class="landing-events-loading-text">"Loading events..."</p>
                        </div>
                    </section>
                }.into_any()
            } else if evts.is_empty() {
                // No events — show heading + empty state message
                view! {
                    <section id="events" class="landing-section-sm">
                        {heading}
                        <div class="landing-events-loading">
                            <p class="landing-events-empty">"No upcoming events right now. Check back soon!"</p>
                        </div>
                    </section>
                }.into_any()
            } else {
                view! {
                    <section id="events" class="landing-section-sm">
                        {heading}
                        <div class="landing-events-grid">
                            {evts.into_iter().map(|evt| {
                                let event_url = format!("/e/{}", evt.slug);
                                let date_str = if evt.event_start_ms > 0 {
                                    let d = js_sys::Date::new_with_year_month_day(0, 0, 0);
                                    d.set_time(evt.event_start_ms as f64);
                                    if evt.time_tba {
                                        // Date only — show just the date, time is TBA
                                        let opts = js_sys::Object::new();
                                        let _ = js_sys::Reflect::set(&opts, &"year".into(), &"numeric".into());
                                        let _ = js_sys::Reflect::set(&opts, &"month".into(), &"short".into());
                                        let _ = js_sys::Reflect::set(&opts, &"day".into(), &"numeric".into());
                                        let date_part = d.to_locale_string("en-US", &opts).as_string().unwrap_or_default();
                                        format!("{date_part} · Time TBA")
                                    } else {
                                        d.to_locale_string("en-US", &js_sys::Object::new()).as_string().unwrap_or_default()
                                    }
                                } else {
                                    "Date TBA".to_string()
                                };
                                let deposit_badge = if evt.deposit_enabled {
                                    view! { <span class="landing-inline-icon"><Icon icon=IconName::Coin class="icon-xs"/>" Deposit required"</span> }.into_any()
                                } else {
                                    view! { <span class="landing-inline-icon"><Icon icon=IconName::TicketFree class="icon-xs"/>" Free entry"</span> }.into_any()
                                };

                                let badge_img = if !evt.nft_image_url.is_empty() {
                                    view! {
                                        <div class="landing-event-badge-img">
                                            <img
                                                src=evt.nft_image_url.clone()
                                                alt="Event badge"
                                            />
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                };

                                let tagline_html = if !evt.tagline.is_empty() {
                                    view! {
                                        <p class="landing-event-tagline">
                                            {evt.tagline.clone()}
                                        </p>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                };

                                let location_html = if !evt.location.is_empty() {
                                    view! {
                                        <p class="landing-event-location">
                                            <span class="landing-inline-icon"><Icon icon=IconName::Pin class="icon-xs"/>" "{evt.location.clone()}</span>
                                        </p>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                };

                                view! {
                                    <a
                                        href=event_url
                                        class="event-card-link"
                                    >
                                        <div
                                            class="card event-card landing-event-card"
                                        >
                                            {badge_img}
                                            <h3 class="landing-event-name">
                                                {evt.name}
                                            </h3>
                                            {tagline_html}
                                            <p class="landing-event-meta">
                                                <span class="landing-inline-icon"><Icon icon=IconName::Calendar class="icon-xs"/>" "{date_str}</span>
                                            </p>
                                            {location_html}
                                            <p class="landing-event-deposit">
                                                {deposit_badge}
                                            </p>
                                        </div>
                                    </a>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    </section>
                }.into_any()
            }
        }}
    }
}

// ---------------------------------------------------------------------------
// My Registrations — signed-in attendees see their registered events
// ---------------------------------------------------------------------------

/// Response item from GET /api/my-registrations.
#[derive(Clone, Deserialize)]
struct MyRegistrationItem {
    event_name: String,
    event_slug: String,
    #[serde(default)]
    event_start_ms: i64,
    #[allow(dead_code)]
    attendee_id: String,
    /// Human-readable status: "registered", "deposit pending", "deposit confirmed",
    /// "checked in", "nft claimed".
    status: String,
    next_step: NextStepData,
}

#[derive(Clone, Deserialize)]
struct NextStepData {
    #[serde(rename = "type")]
    step_type: String,
    url: String,
}

/// Component that shows the user's event registrations when signed in.
/// If not signed in, renders nothing.
#[component]
fn MyRegistrations() -> impl IntoView {
    let (registrations, set_registrations) = signal(None::<Vec<MyRegistrationItem>>);
    let (email, set_email) = signal(None::<String>);

    // Check auth and fetch registrations on mount
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window
                .location()
                .origin()
                .unwrap_or_else(|_| "http://localhost:8787".to_string());

            // Check auth status
            let auth_url = format!("{origin}/api/auth/me");
            let auth_resp = match crate::api::fetch::get(&auth_url, &[]).await {
                Ok(r) => r,
                Err(_) => return,
            };

            if auth_resp.status() != 200 {
                return;
            }

            let auth_data: serde_json::Value = match crate::api::fetch::response_json(&auth_resp).await {
                Ok(d) => d,
                Err(_) => return,
            };
            let user_email = auth_data["data"]["email"].as_str().unwrap_or("").to_string();
            if user_email.is_empty() {
                return;
            }
            set_email.set(Some(user_email));

            // Fetch my registrations
            let regs_url = format!("{origin}/api/my-registrations");
            match crate::api::fetch::get(&regs_url, &[]).await {
                Ok(resp) if resp.status() == 200 => {
                    if let Ok(data) =
                        crate::api::fetch::response_json::<ApiResponse<Vec<MyRegistrationItem>>>(&resp)
                            .await
                    {
                        set_registrations.set(Some(data.data.unwrap_or_default()));
                    }
                }
                _ => {
                    set_registrations.set(Some(vec![]));
                }
            }
        });
    });

    move || {
        let regs = registrations.get();
        let user_email = email.get();

        match (regs, user_email) {
            (None, _) | (Some(_), None) => ().into_any(),
            (Some(refs), Some(user)) if refs.is_empty() => {
                view! {
                    <section class="landing-reg-section">
                        <div class="landing-reg-empty">
                            <p class="landing-reg-empty-user">
                                {format!("👤 {user}")}
                            </p>
                            <p class="landing-reg-empty-text">
                                "You haven't registered for any events yet. Check out upcoming events above!"
                            </p>
                            <button
                                class="btn btn-outline btn-xs landing-reg-signout-btn"
                                on:click=move |_| {
                                    leptos::task::spawn_local(async move {
                                        let _ = crate::api::fetch::post("/api/auth/logout", &[], None).await;
                                        let window = web_sys::window().expect("no window");
                                        let _ = window.location().reload();
                                    });
                                }
                            >
                                "Sign out"
                            </button>
                        </div>
                    </section>
                }.into_any()
            }
            (Some(refs), Some(user)) => {
                view! {
                    <section class="landing-reg-section">
                        <div class="landing-reg-header">
                            <h2 class="landing-reg-title">
                                "Your Events"
                            </h2>
                            <div class="landing-reg-user">
                                <span class="landing-email-text">{format!("\u{1f464} {user}")}</span>
                                <button
                                    class="btn btn-outline btn-xs"
                                    on:click=move |_| {
                                        leptos::task::spawn_local(async move {
                                            let _ = crate::api::fetch::post("/api/auth/logout", &[], None).await;
                                            let window = web_sys::window().expect("no window");
                                            let _ = window.location().reload();
                                        });
                                    }
                                >
                                    "Sign out"
                                </button>
                            </div>
                        </div>
                        <div class="landing-reg-grid">
                            {refs.into_iter().map(|reg| {
                                let event_url = format!("/e/{}", reg.event_slug);
                                let step_label = match reg.next_step.step_type.as_str() {
                                    "deposit" => "Complete Deposit",
                                    "quest" => "Start Quest",
                                    _ => "View",
                                };
                                let date_str = if reg.event_start_ms > 0 {
                                    let d = js_sys::Date::new_with_year_month_day(0, 0, 0);
                                    d.set_time(reg.event_start_ms as f64);
                                    d.to_locale_string("en-US", &js_sys::Object::new()).as_string().unwrap_or_default()
                                } else {
                                    "TBA".to_string()
                                };
                                let next_url = reg.next_step.url.clone();
                                let status_color = match reg.status.as_str() {
                                    "nft claimed" => "#4ade80",
                                    "checked in" => "#4ade80",
                                    "deposit confirmed" => "#22c55e",
                                    "deposit pending" => "#facc15",
                                    _ => "var(--text-secondary)",
                                };
                                view! {
                                    <div class="landing-reg-card">
                                        <div>
                                            <a href=event_url class="landing-reg-event-name">
                                                {reg.event_name}
                                            </a>
                                            <p class="landing-reg-event-date">{date_str}</p>
                                            <p class="landing-reg-event-status" style=format!(
                                                "color:{status_color};"
                                            )>
                                                {reg.status.clone()}
                                            </p>
                                        </div>
                                        <a href=next_url class="btn btn-primary btn-sm landing-reg-action">
                                            {step_label}" →"
                                        </a>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    </section>
                }.into_any()
            }
        }
    }
}

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
    let (active_role, set_active_role) = signal(SwimlaneRole::Attendee);
    let (active_step, set_active_step) = signal(0usize);
    let (mobile_menu_open, set_mobile_menu_open) = signal(false);

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
                        <a href="#features">"Features"</a>
                        <a href="#how-it-works">"How it works"</a>
                        <a href="#faq">"FAQ"</a>
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
                                    view! {
                                        <span class="landing-email-text hide-mobile">
                                            {email.clone()}
                                        </span>
                                        {if is_admin_role(&role) {
                                            view! {
                                                <A href="/admin" attr:class="btn btn-outline btn-sm">
                                                    "Dashboard"
                                                </A>
                                            }.into_any()
                                        } else if role == "staff" {
                                            view! {
                                                <A href="/staff" attr:class="btn btn-outline btn-sm">
                                                    "Scanner"
                                                </A>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                        <button
                                            class="btn btn-outline btn-sm"
                                            on:click=move |_| trigger_landing_signout()
                                        >
                                            "Sign Out"
                                        </button>
                                    }.into_any()
                                }
                                AuthState::Checking => ().into_any(),
                            }
                        }}
                    </div>
                </div>
                // Mobile dropdown menu
                {move || {
                    let open = mobile_menu_open.get();
                    if open {
                        view! {
                            <div class="landing-nav-mobile-menu">
                                <a href="#features" on:click=move |_| set_mobile_menu_open.set(false)>"Features"</a>
                                <a href="#how-it-works" on:click=move |_| set_mobile_menu_open.set(false)>"How it works"</a>
                                <a href="#faq" on:click=move |_| set_mobile_menu_open.set(false)>"FAQ"</a>
                                <a href="#waitlist" on:click=move |_| set_mobile_menu_open.set(false)>"Join Waitlist"</a>
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
                                            <div class="landing-mobile-divider">
                                                <span class="landing-email-text">{email}</span>
                                            </div>
                                            {if is_admin_role(&role) {
                                                view! {
                                                    <A href="/admin" on:click=move |_| set_mobile_menu_open.set(false)>"Dashboard"</A>
                                                }.into_any()
                                            } else if role == "staff" {
                                                view! {
                                                    <A href="/staff" on:click=move |_| set_mobile_menu_open.set(false)>"Scanner"</A>
                                                }.into_any()
                                            } else {
                                                ().into_any()
                                            }}
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
                // Solana pill badge
                <div class="solana-pill">
                    <Icon icon=IconName::Solana />
                    "Built on Solana"
                </div>

                // BeThere name + tagline
                <div class="landing-hero-brand landing-brand-gradient">
                    "BeThere"
                </div>

                <h1 class="landing-hero-h1">
                    "Commit. Show up."
                    <br />
                    <span class="landing-hero-gradient">
                        "Get your money back."
                    </span>
                </h1>
                <p class="landing-hero-desc">
                    "Put down a deposit to reserve your spot. Show up, take a quick quiz, and get every cent back — plus a digital badge you own forever."
                </p>

                // Compact 3-step flow visual
                <div class="landing-steps">
                    <div class="landing-step-item">
                        <div class="landing-step-circle landing-step-circle--indigo">
                            <Icon icon=IconName::Coin class="icon-sm"/>
                        </div>
                        <span class="landing-step-title">"Lock Deposit"</span>
                        <span class="landing-step-subtitle">"0.01 SOL + $13 USDC"</span>
                    </div>
                    // Arrow
                    <div class="landing-step-arrow">
                        <svg width="24" height="12" viewBox="0 0 24 12" fill="none"><path d="M1 6h20m-4-4l4 4-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
                    </div>
                    // Step 2: Show Up
                    <div class="landing-step-item">
                        <div class="landing-step-circle landing-step-circle--amber">
                            <Icon icon=IconName::Camera class="icon-sm"/>
                        </div>
                        <span class="landing-step-title">"Check In"</span>
                        <span class="landing-step-subtitle">"Scan QR · < 2 sec"</span>
                    </div>
                    // Arrow
                    <div class="landing-step-arrow">
                        <svg width="24" height="12" viewBox="0 0 24 12" fill="none"><path d="M1 6h20m-4-4l4 4-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
                    </div>
                    // Step 3: Refund + Badge
                    <div class="landing-step-item">
                        <div class="landing-step-circle landing-step-circle--green">
                            <Icon icon=IconName::Ticket class="icon-sm"/>
                        </div>
                        <span class="landing-step-title">"Get Refund + Badge"</span>
                        <span class="landing-step-subtitle">"Deposit back + cNFT"</span>
                    </div>
                </div>

                <div class="landing-ctas">
                    <a href="#waitlist" class="btn btn-primary landing-cta-link">
                        "Join Waitlist →"
                    </a>
                    <a href="#events" class="btn btn-outline landing-cta-link">
                        "Find Events ↓"
                    </a>
                </div>
            </section>

            // ===== Upcoming Events =====
            <UpcomingEvents />

            // ===== My Registrations (visible when signed in) =====
            <MyRegistrations />

            // ===== Social Proof =====
            // Social proof — real users + CTA for organizers
            <section class="social-proof">
                <div class="social-proof-label">"Alpha · Building with"</div>
                <div class="social-proof-logos">
                    <a
                        href="https://github.com/solana-thailand"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="social-proof-pill landing-social-pill-solana"
                    >
                        "Solana Developer Thailand"
                    </a>
                    <a href="#waitlist" class="social-proof-pill landing-social-pill-cta">
                        "Want to join? → Get in touch"
                    </a>
                </div>
            </section>

            // ===== Problem / Features =====
            <section id="features" class="landing-section">
                <div class="landing-section-header">
                    <h2 class="landing-h2">
                        "Events have a no-show problem"
                    </h2>
                    <p class="landing-subtitle">
                        "Up to 40% of registered attendees don't show up. Organizers pay for empty seats."
                    </p>
                </div>
                <div class="landing-features-grid">

                    <div class="card landing-feature-card">
                        <div class="landing-svg-icon icon-clipboard">
                            <svg viewBox="0 0 24 24">
                                <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
                                <rect x="8" y="2" width="8" height="4" rx="1" ry="1"/>
                            </svg>
                        </div>
                        <h3 class="landing-feature-title">
                            "Paper wristbands"
                        </h3>
                        <p class="landing-feature-desc">
                            "Tear, rip, disappear. No proof you attended a week later."
                        </p>
                    </div>

                    <div class="card landing-feature-card">
                        <div class="landing-svg-icon icon-chart">
                            <svg viewBox="0 0 24 24">
                                <line x1="18" y1="20" x2="18" y2="10"/>
                                <line x1="12" y1="20" x2="12" y2="4"/>
                                <line x1="6" y1="20" x2="6" y2="14"/>
                            </svg>
                        </div>
                        <h3 class="landing-feature-title">
                            "Spreadsheets"
                        </h3>
                        <p class="landing-feature-desc">
                            "Manual entry, typos, and data that lives on someone's laptop."
                        </p>
                    </div>

                    <div class="card landing-feature-card">
                        <div class="landing-svg-icon icon-proof">
                            <svg viewBox="0 0 24 24">
                                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                            </svg>
                        </div>
                        <h3 class="landing-feature-title">
                            "No-shows waste money"
                        </h3>
                        <p class="landing-feature-desc">
                            "Registered attendees who don't show up cost organizers real money — food, swag, venue. There's no accountability."
                        </p>
                    </div>

                </div>
            </section>

            // ===== How It Works — Swimlane =====
            <section id="how-it-works" class="landing-section-how">
                <div class="landing-section-header-md">
                    <h2 class="landing-h2">
                        "How it works"
                    </h2>
                    <p class="landing-subtitle">
                        "Three perspectives. One seamless event."
                    </p>
                </div>

                <div class="landing-role-tabs">
                    {move || {
                        let active = active_role.get();
                        [SwimlaneRole::Organizer, SwimlaneRole::Staff, SwimlaneRole::Attendee].into_iter().map(|r| {
                            let is_active = r == active;
                            let accent = r.accent();
                            let bg = r.accent_bg();
                            let border = r.accent_border();
                            view! {
                                <button
                                    class="landing-role-tab"
                                    style=format!(
                                        "border:1px solid {};background:{};color:{};",
                                        if is_active { border } else { "var(--border)" },
                                        if is_active { bg } else { "transparent" },
                                        if is_active { accent } else { "var(--text-secondary)" },
                                    )
                                    on:click=move |_| set_active_role.set(r)
                                >
                                    <Icon icon=r.icon_name() class="icon-sm"/>
                                    <span>{r.label()}</span>
                                </button>
                            }
                        }).collect_view()
                    }}
                </div>

                <div class="landing-step-flow">
                    {move || {
                        let role = active_role.get();
                        let step = active_step.get();
                        let steps = role.steps();
                        let accent = role.accent();
                        steps.iter().enumerate().map(|(i, s)| {
                            let is_active = i == step;
                            let is_past = i < step;
                            let steps_len = steps.len();
                            view! {
                                <button class="landing-step-flow-btn"
                                    on:click=move |_| set_active_step.set(i)
                                >
                                    <div class="landing-step-flow-dot" style=format!(
                                        "border:2px solid {};background:{};",
                                        if is_active { accent } else if is_past { accent } else { "var(--border)" },
                                        if is_active { role.accent_bg().to_string() } else if is_past { "rgba(255,255,255,0.04)".to_string() } else { "transparent".to_string() }
                                    )>
                                        <span class="landing-step-flow-dot-inner" style=format!("color:{};", if is_active || is_past { accent } else { "var(--text-secondary)" })><Icon icon=s.icon class="icon-sm"/></span>
                                    </div>
                                    <span class="landing-step-flow-label" style=format!(
                                        "font-weight:{};color:{};",
                                        if is_active { "600" } else { "500" },
                                        if is_active { "#fff" } else { "var(--text-secondary)" },
                                    )>{s.title}</span>
                                </button>
                                {if i < steps_len - 1 {
                                    let line_color = if is_past { accent } else { "var(--border)" }.to_string();
                                    view! {
                                        <div class="landing-step-flow-line" style=format!("background:{};", line_color)></div>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }}
                            }
                        }).collect_view()
                    }}
                </div>

                <div class="landing-mockup-wrapper">
                    {move || {
                        let role = active_role.get();
                        let step_idx = active_step.get();
                        let steps = role.steps();
                        let step_data = steps.get(step_idx);
                        view! {
                            <div class="landing-mockup">
                                {swimlane_mockup(role, step_idx)}
                                <div class="landing-mockup-desc">
                                    {step_data.map(|s| s.desc).unwrap_or("")}
                                </div>
                            </div>
                        }
                    }}
                </div>

                <div class="landing-swimlane-mini-list">
                    {move || {
                        let active = active_role.get();
                        [SwimlaneRole::Organizer, SwimlaneRole::Staff, SwimlaneRole::Attendee].into_iter().map(|r| {
                            let is_active = r == active;
                            let steps = r.steps();
                            let accent = r.accent();
                            let bg = r.accent_bg();
                            let border = r.accent_border();
                            view! {
                                <button
                                    class="landing-swimlane-mini-btn"
                                    style=format!(
                                        "border:1px solid {};background:{};",
                                        if is_active { border } else { "var(--border)" },
                                        if is_active { bg } else { "transparent" },
                                    )
                                    on:click=move |_| {
                                        set_active_role.set(r);
                                        set_active_step.set(0);
                                    }
                                >
                                    <span class="landing-swimlane-mini-label"><Icon icon=r.icon_name() class="icon-sm"/><span>{r.label()}</span></span>
                                    <div class="landing-swimlane-mini-dots">
                                        {steps.iter().enumerate().map(|(i, s)| {
                                            view! {
                                                <>
                                                    <div class="landing-swimlane-mini-dot" style=format!(
                                                        "background:{};",
                                                        if is_active { accent } else { "var(--border)" },
                                                    ) title=s.title></div>
                                                    {if i < steps.len() - 1 {
                                                        view! {
                                                            <div class="landing-swimlane-mini-arrow"></div>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                </>
                                            }
                                        }).collect_view()}
                                    </div>
                                    {if is_active {
                                        view! {
                                            <span class="landing-swimlane-mini-viewing" style=format!("color:{};", accent)>"viewing"</span>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                </button>
                            }
                        }).collect_view()
                    }}
                </div>

            </section>

            // ===== Waitlist =====
            <section id="waitlist" class="landing-section">
                <div class="landing-waitlist-inner">
                    <h2 class="landing-h2">
                        "Ready to end no-shows?"
                    </h2>
                    <p class="landing-faq-a">
                        "Join the waitlist to bring deposit-backed events to your community."
                    </p>
                    <WaitlistForm />
                </div>
            </section>

            // ===== FAQ =====
            <section id="faq" class="landing-section-narrow">
                <div class="landing-section-header">
                    <h2 class="landing-h2">
                        "Frequently asked questions"
                    </h2>
                    <p class="landing-subtitle">
                        "Everything you need to know about BeThere."
                    </p>
                </div>

                <div class="landing-faq-grid">

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "What is BeThere?"
                        </h3>
                        <p class="landing-faq-a">
                            "BeThere is a deposit-backed event check-in platform built on Solana. Attendees put down a small deposit when they register. If they show up and complete a short quiz, they get their deposit back automatically — plus a compressed NFT badge as proof of attendance. If they don't show up, the organizer keeps the deposit."
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "Do attendees need a crypto wallet?"
                        </h3>
                        <p class="landing-faq-a">
                            "Not to check in! QR scanning works with any phone — no wallet required at the door. The wallet is only needed when claiming the NFT badge and deposit refund afterward. We support Phantom, Solflare, Backpack, or you can just paste your wallet address."
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "How does the deposit work?"
                        </h3>
                        <p class="landing-faq-a">
                            "Organizers set a deposit amount (e.g., 500 THB / ~$15). Attendees pay it when registering. After check-in and completing the quiz, the deposit is refunded on-chain as SOL + USDC directly to the attendee's wallet. No-shows forfeit their deposit to the organizer."
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "What is a compressed NFT badge?"
                        </h3>
                        <p class="landing-faq-a">
                            "It's a digital collectible on Solana that proves you attended an event. Unlike regular NFTs, compressed NFTs cost a fraction of a cent to mint (~$0.001) using Merkle trees. Each badge is unique to the event and lives in your wallet forever — think of it as a digital ticket stub that can't be faked."
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "What's the quiz?"
                        </h3>
                        <p class="landing-faq-a">
                            "Organizers can set a short quiz (e.g., 3-5 questions) about the event content. Attendees answer after check-in. It proves they actually paid attention — not just physically showed up. The passing threshold is configurable by the organizer."
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "How much does it cost for organizers?"
                        </h3>
                        <p class="landing-faq-a">
                            "BeThere is free during beta. We cover cNFT minting costs (fractions of a cent per badge). Future pricing will be per-event with a generous free tier. No per-attendee charge during beta."
                        </p>
                    </div>

                    <div class="landing-faq-card">
                        <h3 class="landing-faq-q">
                            "Is BeThere only for crypto events?"
                        </h3>
                        <p class="landing-faq-a">
                            "It works great for any event! The deposit + check-in flow solves no-shows for meetups, workshops, conferences, and hackathons. The Solana/NFT part happens behind the scenes — attendees don't need to know anything about crypto."
                        </p>
                    </div>

                </div>

                <div class="landing-faq-cta">
                    <a href="#waitlist" class="btn btn-outline landing-faq-cta-link">
                        "Ready to try? Join the waitlist →"
                    </a>
                </div>
            </section>

            // ===== Footer =====
            <footer class="landing-footer">
                <div class="landing-footer-grid">

                    // Column 1 — Brand
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
                    </div>

                    // Column 2 — Product
                    <div class="landing-footer-col">
                        <h4>"Product"</h4>
                        <a href="#features">"Features"</a>
                        <a href="#how-it-works">"How It Works"</a>
                        <a href="#faq">"FAQ"</a>
                        <A href="/login">"Staff Portal"</A>
                    </div>

                    // Column 3 — Community
                    <div class="landing-footer-col">
                        <h4>"Community"</h4>
                        <a href="https://x.com/ozoneRatchapon" target="_blank" rel="noopener noreferrer">"X / Twitter"</a>
                        <a href="https://github.com/solana-thailand" target="_blank" rel="noopener noreferrer">"GitHub"</a>
                        <a href="https://github.com/solana-thailand/BeThere" target="_blank" rel="noopener noreferrer">"Source Code"</a>
                    </div>

                </div>

                // Bottom row
                <div class="landing-footer-bottom">
                    <span class="landing-footer-copy">"© 2026 BeThere. All rights reserved."</span>
                    <span class="landing-footer-powered">
                        <Icon icon=IconName::Solana />
                        "Built on Solana"
                    </span>
                </div>
            </footer>

        </div>
    }
}
