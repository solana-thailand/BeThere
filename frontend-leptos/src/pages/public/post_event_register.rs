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
use crate::i18n::{t_string, use_i18n};
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
    Error(RegError),
}

/// Why the form cannot go on. Local causes are typed so they render in the
/// reader's language; a server message is passed through as sent.
#[derive(Debug, Clone, PartialEq)]
enum RegError {
    MissingSlug,
    NeedName,
    NeedConsent,
    Server(String),
}

/// Public post-event registration form component.
/// Field keys for the post-event question set.
///
/// The `post.` prefix is load-bearing, not cosmetic: the Worker routes these
/// answers to `registration_responses` scoped to the event and skips the
/// developer-profile upsert entirely (`.issues/082`). Renaming them without
/// the prefix would start writing survey answers onto people's profiles.
const KEY_SATISFACTION: &str = "post.satisfaction.overall";
const KEY_NPS: &str = "post.nps";
const KEY_WOULD_RETURN: &str = "post.would_return";
const KEY_COMMENT: &str = "post.comment";

/// Collect the answered questions into the map the Worker expects.
///
/// Unanswered questions are omitted rather than sent empty: the Worker drops
/// empty values anyway, and an absent key is honestly "not answered" where an
/// empty string reads as "answered with nothing".
fn collected_answers(
    satisfaction: &str,
    nps: &str,
    would_return: &str,
    comment: &str,
) -> Option<std::collections::HashMap<String, String>> {
    let mut answers = std::collections::HashMap::new();
    for (key, value) in [
        (KEY_SATISFACTION, satisfaction),
        (KEY_NPS, nps),
        (KEY_WOULD_RETURN, would_return),
        (KEY_COMMENT, comment),
    ] {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            answers.insert(key.to_string(), trimmed.to_string());
        }
    }
    match answers.is_empty() {
        true => None,
        false => Some(answers),
    }
}

#[component]
#[allow(non_snake_case)]
pub fn PostEventRegister() -> impl IntoView {
    let i18n = use_i18n();
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
    // Post-event satisfaction (.issues/087). The three keys are the ones DevRel
    // specified; they live in the `post.` namespace so they are recorded
    // against the event and never mistaken for profile data (.issues/082).
    let (sat_overall, set_sat_overall) = signal(String::new());
    let (nps, set_nps) = signal(String::new());
    let (would_return, set_would_return) = signal(String::new());
    let (comment, set_comment) = signal(String::new());
    let (consent_given, set_consent_given) = signal(false);
    // Unticked, as on the registration form (`public_event/page.rs`): a
    // pre-ticked box is not consent under PDPA s.19, and now that an unticked
    // box is sent as a stated no, a pre-tick would record a yes nobody gave.
    let (consent_marketing, set_consent_marketing) = signal(false);

    // Auth gate on mount — same pattern as the dev profile editor.
    {
        let slug = slug_val.clone();
        leptos::task::spawn_local(async move {
            // Empty slug = bad link — surface an error rather than a form.
            if slug.is_empty() {
                set_state.set(RegState::Error(RegError::MissingSlug));
                return;
            }
            // Verify auth via cookie-based API call (HttpOnly cookie may be
            // valid even when localStorage token is absent).
            match crate::api::get_me().await {
                Ok(_) => set_state.set(RegState::Form),
                Err(_) => {
                    let next = format!("/events/{slug}/post-event-register");
                    let login_url = format!("/login?next={}", urlencoding::encode(&next));
                    let _ = web_sys::window().map(|w| w.location().set_href(&login_url));
                }
            }
        });
    }

    // Wrap the slug in a signal so the reactive view closure + the on:click
    // submit handler can both read it without moving the owned String.
    let (slug_sig, set_slug_sig) = signal(slug_val.clone());
    set_slug_sig.set(slug_val.clone());

    // The completed-event gateway is always available for a public completed
    // event. A recap may intentionally be unpublished, so it cannot be this
    // form's recovery destination.
    let back_href = format!("/e/{slug}", slug = slug_val);

    let is_submitting = move || matches!(state.get(), RegState::Submitting);

    view! {
        <Title text=crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.page_title)) />
        <div class="center-page">
            <div class="container layout-col-center">
                // ---------- Back link ----------
                <div class="flex-row-gap" style="margin-bottom:1rem;width:100%;justify-content:flex-start;">
                    <A href=back_href.clone() attr:class="btn btn-outline btn-sm">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.back))}
                    </A>
                </div>

                // ---------- Auth / submit / error states ----------
                {move || match state.get() {
                    RegState::CheckingAuth => view! {
                        <div class="page-loading">
                            <span class="spinner spinner-lg"></span>
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.loading))}
                        </div>
                    }.into_any(),
                    RegState::Error(msg) => view! {
                        <div class="card layout-col-center">
                            <span style="margin-bottom:1rem;opacity:0.6;">
                                <Icon icon=IconName::Warning class="icon-2xl icon-warning" />
                            </span>
                            <h2 style="margin:0 0 0.5rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.error_title))}</h2>
                            <p class="subtitle" style="margin:0 0 1rem;text-align:center;">
                                {match msg {
                                    RegError::MissingSlug => crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.missing_slug)).into_any(),
                                    RegError::NeedName => crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.need_name)).into_any(),
                                    RegError::NeedConsent => crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.need_consent)).into_any(),
                                    RegError::Server(msg) => msg.into_any(),
                                }}
                            </p>
                            <A href=back_href.clone() attr:class="btn btn-outline btn-sm">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.back))}
                            </A>
                        </div>
                    }.into_any(),
                    RegState::Done => view! {
                        <div class="card layout-col-center">
                            <span style="margin-bottom:1rem;color:#16a34a;">
                                <Icon icon=IconName::Check class="icon-2xl" />
                            </span>
                            <h2 style="margin:0 0 0.5rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.done_title))}</h2>
                            <p class="subtitle" style="margin:0 0 1rem;text-align:center;">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.done_body))}
                            </p>
                            <A href="/past-events" attr:class="btn btn-primary">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.browse_past))}
                            </A>
                        </div>
                    }.into_any(),
                    RegState::Form | RegState::Submitting => view! {
                        <div class="card" style="width:100%;max-width:540px;">
                            <h1 style="margin:0 0 0.5rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.title))}</h1>
                            <p class="subtitle" style="margin:0 0 1.5rem;">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.intro))}
                            </p>

                            // Name
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.name))}</label>
                                <input
                                    class="dev-profile-input"
                                    type="text"
                                    placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.name_placeholder))
                                    prop:value=name.get()
                                    on:input=move |ev| set_name.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                />
                            </div>

                            // Contact channel
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.contact))}</label>
                                <select
                                    class="dev-profile-select"
                                    prop:value=contact_channel.get()
                                    on:change=move |ev| set_contact_channel.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                >
                                    <option value="">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.select))}</option>
                                    <option value="telegram">"Telegram"</option>
                                    <option value="line">"Line"</option>
                                    <option value="facebook">"Facebook"</option>
                                    <option value="x">"X (Twitter)"</option>
                                </select>
                            </div>

                            // Contact handle
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.handle))}</label>
                                <input
                                    class="dev-profile-input"
                                    type="text"
                                    placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.handle_placeholder))
                                    prop:value=contact_handle.get()
                                    on:input=move |ev| set_contact_handle.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                />
                            </div>

                            // Experience level
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.experience))}</label>
                                <select
                                    class="dev-profile-select"
                                    prop:value=experience_level.get()
                                    on:change=move |ev| set_experience_level.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                >
                                    <option value="">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.select))}</option>
                                    <option value="beginner">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.beginner))}</option>
                                    <option value="intermediate">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.intermediate))}</option>
                                    <option value="advanced">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.advanced))}</option>
                                </select>
                            </div>

                            // Tech stack
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.stack))}</label>
                                <input
                                    class="dev-profile-input"
                                    type="text"
                                    placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.stack_placeholder))
                                    prop:value=tech_stack.get()
                                    on:input=move |ev| set_tech_stack.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                />
                            </div>

                            // Interests
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.interests))}</label>
                                <textarea
                                    class="dev-profile-input"
                                    rows="3"
                                    placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.interests_placeholder))
                                    prop:value=interests.get()
                                    on:input=move |ev| set_interests.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                ></textarea>
                            </div>

                            // ── Post-event questions (.issues/087) ──────────
                            // Asked here rather than by a follow-up email: two
                            // Google Forms sent after past events got zero
                            // replies between them. The person is already on
                            // the page, so this is the moment they will answer.
                            <div class="dev-profile-field">
                                <label class="dev-profile-label">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.satisfaction_q))}
                                </label>
                                <select
                                    class="dev-profile-select"
                                    prop:value=sat_overall.get()
                                    on:change=move |ev| set_sat_overall.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                >
                                    <option value="">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.select))}</option>
                                    <option value="5">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.sat_5))}</option>
                                    <option value="4">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.sat_4))}</option>
                                    <option value="3">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.sat_3))}</option>
                                    <option value="2">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.sat_2))}</option>
                                    <option value="1">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.sat_1))}</option>
                                </select>
                            </div>

                            <div class="dev-profile-field">
                                <label class="dev-profile-label">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.nps_q))}
                                </label>
                                <select
                                    class="dev-profile-select"
                                    prop:value=nps.get()
                                    on:change=move |ev| set_nps.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                >
                                    <option value="">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.select))}</option>
                                    <option value="10">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.nps_10))}</option>
                                    <option value="9">"9"</option>
                                    <option value="8">"8"</option>
                                    <option value="7">"7"</option>
                                    <option value="6">"6"</option>
                                    <option value="5">"5"</option>
                                    <option value="4">"4"</option>
                                    <option value="3">"3"</option>
                                    <option value="2">"2"</option>
                                    <option value="1">"1"</option>
                                    <option value="0">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.nps_0))}</option>
                                </select>
                            </div>

                            <div class="dev-profile-field">
                                <label class="dev-profile-label">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.return_q))}
                                </label>
                                <select
                                    class="dev-profile-select"
                                    prop:value=would_return.get()
                                    on:change=move |ev| set_would_return.set(event_target_value(&ev))
                                    disabled=is_submitting()
                                >
                                    <option value="">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.select))}</option>
                                    <option value="yes">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.yes))}</option>
                                    <option value="maybe">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.maybe))}</option>
                                    <option value="no">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.no))}</option>
                                </select>
                            </div>

                            <div class="dev-profile-field">
                                <label class="dev-profile-label">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.change_q))}
                                </label>
                                <textarea
                                    class="dev-profile-input"
                                    rows="3"
                                    placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.change_placeholder))
                                    prop:value=comment.get()
                                    on:input=move |ev| set_comment.set(event_target_value(&ev))
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
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.consent))}
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
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join.marketing))}
                                </label>
                            </div>

                            // Submit
                            <button
                                class="btn btn-primary"
                                style="width:100%;margin-top:0.5rem;"
                                on:click=move |_| {
                                    let n = name.get();
                                    if n.trim().is_empty() {
                                        set_state.set(RegState::Error(RegError::NeedName));
                                        return;
                                    }
                                    if !consent_given.get() {
                                        set_state.set(RegState::Error(RegError::NeedConsent));
                                        return;
                                    }
                                    let body = PostEventRegisterBody {
                                        name: n.trim().to_string(),
                                        contact_channel: opt_str(contact_channel.get()),
                                        contact_handle: opt_str(contact_handle.get()),
                                        consent_given: true,
                                        consent_marketing: Some(consent_marketing.get()),
                                        experience_level: opt_str(experience_level.get()),
                                        tech_stack: opt_str(tech_stack.get()),
                                        interests: opt_str(interests.get()),
                                        profile_fields: collected_answers(
                                            &sat_overall.get(),
                                            &nps.get(),
                                            &would_return.get(),
                                            &comment.get(),
                                        ),
                                    };
                                    set_state.set(RegState::Submitting);
                                    let slug = slug_sig.get();
                                    leptos::task::spawn_local(async move {
                                        match api::register_post_event(&slug, &body).await {
                                            Ok(_) => set_state.set(RegState::Done),
                                            Err(e) => set_state.set(RegState::Error(RegError::Server(e.message))),
                                        }
                                    });
                                }
                                disabled=is_submitting()
                            >
                                {move || match is_submitting() {
                                    true => t_string!(i18n, recap.join.submitting),
                                    false => t_string!(i18n, recap.join.submit),
                                }}
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
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::{KEY_COMMENT, KEY_NPS, KEY_SATISFACTION, KEY_WOULD_RETURN, collected_answers};

    /// The `post.` prefix is what routes an answer to the event instead of to
    /// the person's profile (`.issues/082`). Losing it would start writing
    /// survey answers onto `developer_profiles`.
    #[test]
    fn every_key_stays_in_the_event_scoped_namespace() {
        for key in [KEY_SATISFACTION, KEY_NPS, KEY_WOULD_RETURN, KEY_COMMENT] {
            assert!(
                key.starts_with("post."),
                "{key} must stay in the post. namespace"
            );
        }
    }

    #[test]
    fn an_untouched_form_sends_nothing() {
        assert!(collected_answers("", "", "", "   ").is_none());
    }

    #[test]
    fn only_answered_questions_are_sent() {
        let answers = collected_answers("5", "", "yes", "").expect("two answers");
        assert_eq!(answers.len(), 2);
        assert_eq!(answers.get(KEY_SATISFACTION).map(String::as_str), Some("5"));
        assert_eq!(
            answers.get(KEY_WOULD_RETURN).map(String::as_str),
            Some("yes")
        );
        assert!(
            !answers.contains_key(KEY_NPS),
            "an unanswered question must be absent, not empty — absent reads \
             as 'not answered', empty reads as 'answered with nothing'"
        );
    }

    #[test]
    fn free_text_is_trimmed_but_kept() {
        let answers = collected_answers("", "", "", "  more coffee  ").expect("one answer");
        assert_eq!(
            answers.get(KEY_COMMENT).map(String::as_str),
            Some("more coffee")
        );
    }
}
