//! Post-event registration form (Plan 008 — Phase 3 §3.3.4).
//!
//! Route: `/events/:slug/post-event-register` (JWT-gated — the visitor must
//! sign in with Google first, same gate as normal registration).
//!
//! Lead capture for completed events: a stripped registration form (no deposit,
//! no participation type, no check-in). The primary value is the developer-
//! profile questions (experience / stack / interests) — that's what turns a
//! "missed the event" visitor into an actionable community lead.
//!
//! Data source: `POST /api/public/event/{slug}/register-post-event`.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

use crate::api::{self, PostEventRegisterBody};
use crate::icons::{Icon, IconName};

/// Route parameters for `/events/:slug/post-event-register`.
#[derive(Params, PartialEq, Clone, Debug)]
struct PostEventRegisterParams {
    slug: Option<String>,
}

/// Coarse page state.
#[derive(Debug, Clone, PartialEq)]
enum RegState {
    /// Verifying the JWT session.
    CheckingAuth,
    /// Ready to fill the form.
    Form,
    /// Submitting to the API.
    Submitting,
    /// Success — the lead was captured.
    Done,
    /// Error (409/410/404/network).
    Error(String),
}

/// Public post-event registration form component.
#[component]
#[allow(non_snake_case)]
pub fn PostEventRegister() -> impl IntoView {
    let params = use_params::<PostEventRegisterParams>();
    let slug_val: String = match params.get() {
        Ok(p) => p.slug.unwrap_or_default(),
        Err(_) => String::new(),
    };

    let (state, set_state) = signal(RegState::CheckingAuth);

    // Form-field signals (controlled inputs).
    let (name, set_name) = signal(String::new());
    let (contact_channel, set_contact_channel) = signal(String::new());
    let (contact_handle, set_contact_handle) = signal(String::new());
    let (experience_level, set_experience_level) = signal(String::new());
    let (tech_stack, set_tech_stack) = signal(String::new());
    let (interests, set_interests) = signal(String::new());
    let (consent_given, set_consent_given) = signal(false);
    let (consent_marketing, set_consent_marketing) = signal(true);

    // Auth gate on mount — same pattern as the dev profile editor.
    {
        let set_state = set_state;
        let slug = slug_val.clone();
        leptos::task::spawn_local(async move {
            // Empty slug = bad link — surface an error rather than a form.
            if slug.is_empty() {
                set_state.set(RegState::Error("Missing event slug.".to_string()));
                return;
            }
            // Verify auth via cookie-based API call (HttpOnly cookie may be
            // valid even when localStorage token is absent).
            match crate::api::get_me().await {
                Ok(_) => set_state.set(RegState::Form),
                Err(_) => {
                    let _ =
                        web_sys::window().map(|w| w.location().set_href("/login"));
                }
            }
        });
    }

    // Wrap the slug in a signal so the reactive view closure + the on:click
    // submit handler can both read it without moving the owned String.
    let (slug_sig, set_slug_sig) = signal(slug_val.clone());
    set_slug_sig.set(slug_val.clone());

    let back_href = format!("/events/{slug}/recap", slug = slug_val);

    let is_submitting = move || matches!(state.get(), RegState::Submitting);

    view! {
        <Title text="Join the Community — BeThere" />
        <div class="center-page">
            <div class="container layout-col-center">
                // ---------- Back link ----------
                <div class="flex-row-gap" style="margin-bottom:1rem;width:100%;justify-content:flex-start;">
                    <A href=back_href.clone() attr:class="btn btn-outline btn-sm">
                        "← Back to recap"
                    </A>
                </div>

                // ---------- Auth / submit / error states ----------
                {move || match state.get() {
                    RegState::CheckingAuth => view! {
                        <div class="page-loading">
                            <span class="spinner spinner-lg"></span>
                            "Loading..."
                        </div>
                    }.into_any(),
                    RegState::Error(msg) => view! {
                        <div class="card layout-col-center">
                            <span style="margin-bottom:1rem;opacity:0.6;">
                                <Icon icon=IconName::Warning class="icon-2xl icon-warning" />
                            </span>
                            <h2 style="margin:0 0 0.5rem;">"Couldn't register"</h2>
                            <p class="subtitle" style="margin:0 0 1rem;text-align:center;">{msg}</p>
                            <A href=back_href.clone() attr:class="btn btn-outline btn-sm">
                                "← Back to recap"
                            </A>
                        </div>
                    }.into_any(),
                    RegState::Done => view! {
                        <div class="card layout-col-center">
                            <span style="margin-bottom:1rem;color:#16a34a;">
                                <Icon icon=IconName::Check class="icon-2xl" />
                            </span>
                            <h2 style="margin:0 0 0.5rem;">"You're on the list!"</h2>
                            <p class="subtitle" style="margin:0 0 1rem;text-align:center;">
                                "We'll email you about the next event."
                            </p>
                            <A href="/past-events" attr:class="btn btn-primary">
                                "Browse past events"
                            </A>
                        </div>
                    }.into_any(),
                    RegState::Form | RegState::Submitting => view! {
                        <div class="card" style="width:100%;max-width:540px;">
                            <h1 style="margin:0 0 0.5rem;">"Join the community"</h1>
                            <p class="subtitle" style="margin:0 0 1.5rem;">
                                "Missed the event? No worries — tell us about yourself and we'll keep you in the loop."
                            </p>

                            // Name
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">"Name"</label>
                                <input
                                    class="dev-profile-input"
                                    type="text"
                                    placeholder="Your name"
                                    prop:value=name.get()
                                    on:input=move |ev| set_name.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                />
                            </div>

                            // Contact channel
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">"Preferred contact (optional)"</label>
                                <select
                                    class="dev-profile-select"
                                    prop:value=contact_channel.get()
                                    on:change=move |ev| set_contact_channel.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                >
                                    <option value="">"— Select —"</option>
                                    <option value="telegram">"Telegram"</option>
                                    <option value="line">"Line"</option>
                                    <option value="facebook">"Facebook"</option>
                                    <option value="x">"X (Twitter)"</option>
                                </select>
                            </div>

                            // Contact handle
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">"Username / profile link (optional)"</label>
                                <input
                                    class="dev-profile-input"
                                    type="text"
                                    placeholder="@handle or profile URL"
                                    prop:value=contact_handle.get()
                                    on:input=move |ev| set_contact_handle.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                />
                            </div>

                            // Experience level
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">"Experience level (optional)"</label>
                                <select
                                    class="dev-profile-select"
                                    prop:value=experience_level.get()
                                    on:change=move |ev| set_experience_level.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                >
                                    <option value="">"— Select —"</option>
                                    <option value="beginner">"Beginner"</option>
                                    <option value="intermediate">"Intermediate"</option>
                                    <option value="advanced">"Advanced"</option>
                                </select>
                            </div>

                            // Tech stack
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">"Tech stack (optional)"</label>
                                <input
                                    class="dev-profile-input"
                                    type="text"
                                    placeholder="e.g. Rust, React, Solana"
                                    prop:value=tech_stack.get()
                                    on:input=move |ev| set_tech_stack.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                />
                            </div>

                            // Interests
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">"Interests (optional)"</label>
                                <textarea
                                    class="dev-profile-input"
                                    rows="3"
                                    placeholder="What are you curious about?"
                                    prop:value=interests.get()
                                    on:input=move |ev| set_interests.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                ></textarea>
                            </div>

                            // Consents
                            <div class="dev-profile-field" style="flex-direction:row;align-items:flex-start;gap:0.5rem;">
                                <input
                                    type="checkbox"
                                    prop:checked=consent_given.get()
                                    on:change=move |ev| set_consent_given.set(event_target_checked(&ev))
                                    disabled=is_submitting()
                                />
                                <label class="dev-profile-label" style="font-weight:normal;">
                                    "I consent to BeThere collecting my data for this registration."
                                </label>
                            </div>
                            <div class="dev-profile-field" style="flex-direction:row;align-items:flex-start;gap:0.5rem;">
                                <input
                                    type="checkbox"
                                    prop:checked=consent_marketing.get()
                                    on:change=move |ev| set_consent_marketing.set(event_target_checked(&ev))
                                    disabled=is_submitting()
                                />
                                <label class="dev-profile-label" style="font-weight:normal;">
                                    "Send me updates about future events."
                                </label>
                            </div>

                            // Submit
                            <button
                                class="btn btn-primary"
                                style="width:100%;margin-top:0.5rem;"
                                on:click=move |_| {
                                    let n = name.get();
                                    if n.trim().is_empty() {
                                        set_state.set(RegState::Error("Please enter your name.".to_string()));
                                        return;
                                    }
                                    if !consent_given.get() {
                                        set_state.set(RegState::Error(
                                            "Please consent to data collection to continue.".to_string(),
                                        ));
                                        return;
                                    }
                                    let body = PostEventRegisterBody {
                                        name: n.trim().to_string(),
                                        contact_channel: opt_str(contact_channel.get()),
                                        contact_handle: opt_str(contact_handle.get()),
                                        consent_given: true,
                                        consent_marketing: if consent_marketing.get() { Some(true) } else { None },
                                        experience_level: opt_str(experience_level.get()),
                                        tech_stack: opt_str(tech_stack.get()),
                                        interests: opt_str(interests.get()),
                                    };
                                    set_state.set(RegState::Submitting);
                                    let slug = slug_sig.get();
                                    leptos::task::spawn_local(async move {
                                        match api::register_post_event(&slug, &body).await {
                                            Ok(_) => set_state.set(RegState::Done),
                                            Err(e) => set_state.set(RegState::Error(e.message)),
                                        }
                                    });
                                }
                                disabled=is_submitting()
                            >
                                {move || if is_submitting() { "Submitting..." } else { "Join the community" }}
                            </button>
                        </div>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}

/// Map an empty string to `None` so it's skipped during serialization.
fn opt_str(s: String) -> Option<String> {
    let trimmed = s.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}
