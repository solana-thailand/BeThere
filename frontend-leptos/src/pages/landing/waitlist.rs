//! Waitlist signup form.

use leptos::prelude::*;

use crate::i18n::{t, use_i18n};
use crate::icons::{Icon, IconName};

/// Why the form did not go through. Local causes are typed so they render in
/// the reader's language; a server message is passed through as sent.
#[derive(Clone)]
enum WaitlistError {
    InvalidEmail,
    /// The server said no without a reason.
    Unspecified,
    /// A non-2xx response whose body was not JSON.
    Retry,
    Network(String),
    Server(String),
}

/// Waitlist signup form component.
#[component]
pub(super) fn WaitlistForm() -> impl IntoView {
    let (email, set_email) = signal(String::new());
    let (submitted, set_submitted) = signal(false);
    let i18n = use_i18n();
    let (error, set_error) = signal(None::<WaitlistError>);
    let (submitting, set_submitting) = signal(false);
    let (already_registered, set_already_registered) = signal(false);

    let handle_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let email_val = email.get().trim().to_string();

        if email_val.is_empty() || !email_val.contains('@') || !email_val.contains('.') {
            set_error.set(Some(WaitlistError::InvalidEmail));
            return;
        }

        set_error.set(None);
        set_submitting.set(true);

        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window
                .location()
                .origin()
                .unwrap_or("http://localhost:8787".to_string());
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
                                let error_msg = body.get("error").and_then(|v| v.as_str());
                                // Duplicate email — backend returns 400 with "already on the waitlist"
                                match error_msg {
                                    Some(msg) if msg.contains("already on the waitlist") => {
                                        set_already_registered.set(true);
                                    }
                                    Some(msg) => {
                                        set_error.set(Some(WaitlistError::Server(msg.to_string())))
                                    }
                                    None => set_error.set(Some(WaitlistError::Unspecified)),
                                }
                            }
                        }
                        Err(_) => {
                            if (200..300).contains(&status) {
                                set_submitted.set(true);
                            } else {
                                set_error.set(Some(WaitlistError::Retry));
                            }
                        }
                    }
                }
                Err(e) => {
                    set_error.set(Some(WaitlistError::Network(e.to_string())));
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
                    {move || match already_registered.get() {
                        true => crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.already)).into_any(),
                        false => crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.on_list)).into_any(),
                    }}
                </div>
                <div class="landing-waitlist-success-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.reach_out))}</div>
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
                    {move || match submitting.get() {
                        true => crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.joining)).into_any(),
                        false => crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.join)).into_any(),
                    }}
                </button>
            </form>
            <Show
                when=move || error.get().is_some()
                fallback=|| view! { <div></div> }
            >
                <p class="landing-waitlist-error">
                    {move || match error.get() {
                        Some(WaitlistError::InvalidEmail) => crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.invalid_email)).into_any(),
                        Some(WaitlistError::Unspecified) => crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.server_error)).into_any(),
                        Some(WaitlistError::Retry) => crate::locale::tr(|l| crate::i18n::td_string!(l, landing.waitlist.retry_error)).into_any(),
                        Some(WaitlistError::Network(error)) => t!(i18n, landing.waitlist.network_error, error).into_any(),
                        Some(WaitlistError::Server(msg)) => msg.into_any(),
                        None => ().into_any(),
                    }}
                </p>
            </Show>
        </Show>
    }
}
