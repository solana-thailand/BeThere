//! First-visit privacy notice on attendee pages (.plans/038 P2-d).
//!
//! It states what the site actually does, per `docs/pdpa_ropa.md`: one
//! sign-in cookie (`event_checkin_token`, HttpOnly, 24 h) and cookie-free
//! Cloudflare page analytics; no tracking cookies. There is nothing to opt
//! into, so it is a notice with a link, not a consent prompt. Dismissal is
//! remembered in localStorage. Fixed to the bottom of the viewport, so showing
//! or hiding it never shifts the page (no CLS).

use leptos::prelude::*;

use crate::locale::is_attendee_path;

const DISMISSED_KEY: &str = "bethere.privacy_notice";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn dismissed() -> bool {
    storage()
        .and_then(|s| s.get_item(DISMISSED_KEY).ok().flatten())
        .is_some()
}

#[component]
pub fn AttendeePrivacyNotice() -> impl IntoView {
    let location = leptos_router::hooks::use_location();
    let (open, set_open) = signal(!dismissed());
    let dismiss = move |_| {
        if let Some(s) = storage() {
            let _ = s.set_item(DISMISSED_KEY, "1");
        }
        set_open.set(false);
    };
    view! {
        <Show
            when=move || open.get() && is_attendee_path(&location.pathname.get())
            fallback=|| ()
        >
            <aside
                class="privacy-notice"
                aria-label=crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.notice_label))
            >
                <p class="privacy-notice-text">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.notice_body))}
                    " "
                    <a href="/privacy">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.notice_link))}</a>
                </p>
                <button class="btn btn-outline btn-sm privacy-notice-ok" on:click=dismiss>
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.notice_ok))}
                </button>
            </aside>
        </Show>
    }
}
