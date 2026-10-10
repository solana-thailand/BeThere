//! "Email me when it opens" (.plans/045 R4.12): the form in the `/events`
//! empty state. One email per newly opened public event, sent by the
//! worker's hourly announcer from bethere.sol@gmail.com; the answer is the
//! same for a new or a known address. The mail is in the page's language.

use leptos::prelude::*;

use crate::bot_check::{BotCheck, BotCheckSlot};
use crate::i18n::{Locale, t_string, td_string, use_i18n};
use crate::locale::{fill, tr};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SubState {
    Idle,
    Sending,
    Done,
    Invalid,
    Bot,
    Retry,
}

#[component]
pub fn SubscribeForm() -> impl IntoView {
    let i18n = use_i18n();
    let email = RwSignal::new(String::new());
    let state = RwSignal::new(SubState::Idle);
    let saved = RwSignal::new(String::new());
    let bot_check = BotCheck::new();

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let value = email.get().trim().to_lowercase();
        if !event_checkin_domain::validation::is_plausible_email(&value) {
            state.set(SubState::Invalid);
            return;
        }
        state.set(SubState::Sending);
        let locale = match i18n.get_locale_untracked() {
            Locale::th => "th",
            Locale::en => "en",
        };
        leptos::task::spawn_local(async move {
            let origin = web_sys::window()
                .and_then(|w| w.location().origin().ok())
                .unwrap_or_default();
            let body = serde_json::json!({ "email": value, "locale": locale }).to_string();
            let token = bot_check.header();
            let mut headers = vec![("Content-Type", "application/json")];
            if let Some((name, token)) = token.as_ref() {
                headers.push((name, token.as_str()));
            }
            let result =
                crate::api::fetch::post(&format!("{origin}/api/subscribe"), &headers, Some(body))
                    .await;
            // The token is spent whatever the answer was.
            bot_check.reset();
            let next = match result {
                Ok(resp) if (200..300).contains(&resp.status()) => {
                    saved.set(value);
                    SubState::Done
                }
                Ok(resp) => {
                    match crate::api::fetch::response_json::<serde_json::Value>(&resp).await {
                        Ok(b)
                            if b.get("error")
                                .and_then(|e| e.as_str())
                                .is_some_and(event_checkin_domain::turnstile::is_rejection) =>
                        {
                            SubState::Bot
                        }
                        _ => SubState::Retry,
                    }
                }
                Err(_) => SubState::Retry,
            };
            state.set(next);
        });
    };

    let consent = move || {
        let text = t_string!(i18n, landing.site.sub_consent);
        let (before, after) = text.split_once("{privacy}").unwrap_or((text, ""));
        view! {
            {before}
            <a href="/privacy">{tr(|l| td_string!(l, landing.site.sub_privacy))}</a>
            {after}
        }
    };

    view! {
        <Show
            when=move || state.get() == SubState::Done
            fallback=move || view! {
                <form class="lp-sub" on:submit=submit on:focusin=move |_| bot_check.activate()>
                    <label class="lp-sub-row">
                        <span class="lp-sr">{tr(|l| td_string!(l, landing.site.sub_label))}</span>
                        <input
                            type="email"
                            required
                            autocomplete="email"
                            placeholder=tr(|l| td_string!(l, landing.site.sub_placeholder))
                            prop:value=move || email.get()
                            on:input=move |ev| email.set(event_target_value(&ev))
                            disabled=move || state.get() == SubState::Sending
                        />
                    </label>
                    <button
                        class="lp-btn lp-btn-primary"
                        type="submit"
                        disabled=move || state.get() == SubState::Sending || !bot_check.ready()
                    >
                        {move || match state.get() {
                            SubState::Sending => tr(|l| td_string!(l, landing.site.sub_sending)).get(),
                            _ => tr(|l| td_string!(l, landing.site.sub_cta)).get(),
                        }}
                    </button>
                    <p class="lp-sub-consent">{consent}</p>
                    <BotCheckSlot check=bot_check />
                    <p class="lp-sub-error" role="alert">
                        {move || match state.get() {
                            SubState::Invalid => t_string!(i18n, landing.site.sub_invalid),
                            SubState::Bot => t_string!(i18n, landing.site.sub_bot),
                            SubState::Retry => t_string!(i18n, landing.site.sub_retry),
                            _ => "",
                        }}
                    </p>
                </form>
            }
        >
            <p class="lp-sub-ok" role="status">
                {move || fill(t_string!(i18n, landing.site.sub_done), &[("email", &saved.get())])}
            </p>
        </Show>
    }
}
