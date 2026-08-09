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

/// Navigates to the unified login page (/login) for Google or Solana Wallet authentication.
fn trigger_landing_oauth() {
    let window = web_sys::window().expect("no window");
    let _ = window.location().set_href("/login");
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
                // No events — show heading + sandbox demo card
                view! {
                    <section id="events" class="landing-section-sm">
                        {heading}
                        <div class="landing-sandbox-card">
                            <div class="landing-sandbox-icon">{"🎟️"}</div>
                            <div class="landing-sandbox-title">"No live events right now"</div>
                            <div class="landing-sandbox-desc">
                                "BeThere is a deposit-backed check-in platform. Try the flow below or host your own event."
                            </div>
                            <a href="#how-it-works" class="btn btn-primary btn-sm landing-sandbox-btn">
                                "See how it works ↓"
                            </a>
                        </div>
                        <div class="landing-sandbox-secondary">
                            <a href="#waitlist" class="btn btn-outline btn-sm">
                                "Organize an Event"
                            </a>
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
                                    "claim" => "Claim Badge",
                                    "deposit" => "Complete Deposit",
                                    "quest" => "Start Quest",
                                    "ticket" => "View Ticket",
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
                                        // Column 1: Event title & date
                                        <div class="landing-reg-info">
                                            <a href=event_url class="landing-reg-event-name">
                                                {reg.event_name}
                                            </a>
                                            <p class="landing-reg-event-date">{date_str}</p>
                                        </div>
                                        // Column 2: User identity
                                        <div class="landing-reg-identity">
                                            <span class="landing-reg-identity-label">{user.clone()}</span>
                                        </div>
                                        // Column 3: Status badge
                                        <div class="landing-reg-status-badge" style=format!(
                                            "background:{}; color:#000;",
                                            if status_color == "var(--text-secondary)" { "rgba(148,163,184,0.15)".to_string() } else { format!("{status_color}22") }
                                        )>
                                            <span class="landing-reg-status-dot" style=format!("background:{status_color};")></span>
                                            {reg.status.clone()}
                                        </div>
                                        // Column 4: Action button
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
                                <a href="#how-it-works" on:click=move |_| set_mobile_menu_open.set(false)>"How it works"</a>
                                <a href="#faq" on:click=move |_| set_mobile_menu_open.set(false)>"FAQ"</a>
                                <a href="#waitlist" on:click=move |_| set_mobile_menu_open.set(false)>"For Organizers"</a>
                                <a href="/past-events" on:click=move |_| set_mobile_menu_open.set(false)>"Past Events"</a>
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
                                // Attendee persona — primary = find events, secondary = Solana wallet sign in
                                view! {
                                    <a href="#events" class="btn btn-primary landing-cta-link">
                                        "Find Events ↓"
                                    </a>
                                    <a
                                        href="/login"
                                        class="btn btn-outline landing-cta-link"
                                        style="background: linear-gradient(135deg, rgba(153, 69, 255, 0.18) 0%, rgba(20, 241, 149, 0.12) 100%); border-color: rgba(153, 69, 255, 0.5); color: #fff;"
                                    >
                                        "💜 Sign in with Solana Wallet"
                                    </a>
                                }.into_any()
                            }
                        }
                    }}
                    {move || match auth_state.get() {
                        AuthState::SignedIn(_) => ().into_any(),
                        _ if persona.get() == 0 => view! {
                            <button
                                class="btn btn-outline landing-cta-link"
                                on:click=move |_| trigger_landing_oauth()
                            >
                                "Create an Event"
                            </button>
                        }.into_any(),
                        _ => ().into_any(),
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
