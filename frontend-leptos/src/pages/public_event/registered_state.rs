use super::types::*;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

pub fn registered_state(reg_data: &MyRegistrationData, email: &str, current_slug: &str) -> AnyView {
    let next_url = reg_data.next_step.url.clone();
    let step_type = reg_data.next_step._step_type.clone();
    let reg_name = reg_data.name.clone();
    let redirect_url = next_url.clone();
    let share_slug = current_slug.to_string();
    let email_display = email.to_string();
    let has_claim_token = !reg_data.claim_token.is_empty();

    // Auto-redirect for actionable steps (claim, deposit) so users don't get stuck
    // on the event page when they have a clear next action.
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

    // Smarter button label based on the next step type
    let button_label = match step_type.as_str() {
        "claim" => "Claim Your Badge →",
        "deposit" => "Complete Deposit →",
        "ticket" => "View My Ticket →",
        _ => "Continue →",
    };

    view! {
        <div class="pe-card">
            <div class="pe-text-center">
                <div class="pe-success-icon-lg">
                    <Icon icon=IconName::Check class="icon-2xl icon-success" />
                </div>
                <h2 class="pe-section-title pe-title-success">
                    "You're already registered!"
                </h2>
                <p class="pe-detail-secondary pe-mb-025">
                    {format!("Welcome back, {reg_name}!")}
                </p>
                <p class="pe-detail-secondary">
                    {format!("Signed in as {email_display}")}
                </p>
                {if has_claim_token {
                    view! {
                        <p class="pe-detail-secondary pe-mt-025" style="color: var(--success);">
                            "✅ Quest complete — ready to claim your badge!"
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
                        " Share Event"
                    </button>
                </div>
            </div>
        </div>
    }.into_any()
}
