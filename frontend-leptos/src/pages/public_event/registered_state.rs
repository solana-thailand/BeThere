use super::types::*;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

pub fn registered_state(
    reg_data: &MyRegistrationData,
    email: &str,
    current_slug: &str,
) -> AnyView {
    let next_url = reg_data.next_step.url.clone();
    let reg_name = reg_data.name.clone();
    let redirect_url = next_url.clone();
    let share_slug = current_slug.to_string();
    let email_display = email.to_string();

    // Countdown for auto-redirect
    let (countdown, set_countdown) = signal(5u32);
    let redirect_url_for_timer = redirect_url.clone();
    leptos::task::spawn_local(async move {
        for i in (1..=5).rev() {
            gloo_timers::future::TimeoutFuture::new(1000).await;
            set_countdown.set(i - 1);
        }
        navigateTo(&redirect_url_for_timer);
    });

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
                <p class="pe-detail-secondary pe-mt-05">
                    {move || format!("Continuing in {}...", countdown.get())}
                </p>
                <div class="pe-btn-row-center">
                    <button
                        class="btn btn-primary btn-sm"
                        on:click=move |_| navigateTo(&redirect_url)
                    >
                        "Continue now →"
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
