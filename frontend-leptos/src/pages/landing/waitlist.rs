//! Waitlist signup form.

use leptos::prelude::*;

use crate::icons::{Icon, IconName};


/// Waitlist signup form component.
#[component]
pub(super) fn WaitlistForm() -> impl IntoView {
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
