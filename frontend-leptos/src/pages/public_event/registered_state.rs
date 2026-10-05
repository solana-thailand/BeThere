use super::types::*;
use crate::i18n::{t, t_string, use_i18n};
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

pub fn registered_state(
    reg_data: &MyRegistrationData,
    email: &str,
    current_slug: &str,
    event_id: &str,
) -> AnyView {
    let i18n = use_i18n();
    let step_type = landing_step(&reg_data.next_step._step_type).to_string();
    let next_url = match reg_data.next_step._step_type.as_str() {
        "claim" => ticket_url(&reg_data.attendee_id, event_id),
        _ => reg_data.next_step.url.clone(),
    };
    let reg_name = reg_data.name.clone();
    let redirect_url = next_url.clone();
    let share_slug = current_slug.to_string();
    let email_display = email.to_string();
    let has_claim_token = !reg_data.claim_token.is_empty();

    // Auto-redirect for actionable steps so users don't get stuck on the event
    // page when they have a clear next action. A ready claim goes to the ticket
    // (see `landing_step`), whose claim card is one tap away.
    let auto_redirect_url = match step_type.as_str() {
        "claim" | "deposit" | "ticket" | "waiting" => Some(redirect_url.clone()),
        _ => None,
    };
    if let Some(ref url) = auto_redirect_url {
        let url = url.clone();
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(600).await;
            navigateTo(&url);
        });
    }

    // Smarter button label based on the next step type (the step type is a
    // server code; only the label is translated).
    let button_label = move || match step_type.as_str() {
        "claim" => t_string!(i18n, event.next_claim),
        "deposit" => t_string!(i18n, event.next_deposit),
        "ticket" => t_string!(i18n, event.next_ticket),
        _ => t_string!(i18n, event.continue_cta),
    };

    view! {
        <div class="pe-card">
            <div class="pe-text-center">
                <div class="pe-success-icon-lg">
                    <Icon icon=IconName::Check class="icon-2xl icon-success" />
                </div>
                <h2 class="pe-section-title pe-title-success">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, event.already_registered))}
                </h2>
                <p class="pe-detail-secondary pe-mb-025">
                    {t!(i18n, event.welcome_back, name = reg_name)}
                </p>
                <p class="pe-detail-secondary">
                    {t!(i18n, event.signed_in_as, email = email_display)}
                </p>
                {if has_claim_token {
                    view! {
                        <p class="pe-detail-secondary pe-mt-025" style="color: var(--success);">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, event.quest_complete))}
                        </p>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                <div class="pe-btn-row-center">
                    <button
                        class="btn btn-primary btn-sm"
                        on:click=move |_| navigateTo(&redirect_url)
                    >
                        {button_label}
                    </button>
                    <button
                        class="btn btn-outline btn-sm"
                        on:click=move |_| {
                            let window = web_sys::window().expect("no window");
                            let url = format!("{}/e/{share_slug}", window.location().origin().unwrap_or_default());
                            let _ = share_event_js("", &url);
                        }
                    >
                        <Icon icon=IconName::Link class="icon-sm" />
                        " "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.share_event))}
                    </button>
                </div>
            </div>
        </div>
    }.into_any()
}

/// The step the event page sends a registered attendee to. A ready claim lands
/// on the ticket, not the claim page: the ticket carries the organizer's slides
/// and links, and its claim card is one tap away.
pub fn landing_step(step_type: &str) -> &str {
    match step_type {
        "claim" => "ticket",
        other => other,
    }
}

/// The ticket page for one attendee of one event. `event_id` is required: without
/// it the ticket API answers for the active event instead.
pub fn ticket_url(attendee_id: &str, event_id: &str) -> String {
    format!("/ticket/{attendee_id}?event_id={event_id}")
}
