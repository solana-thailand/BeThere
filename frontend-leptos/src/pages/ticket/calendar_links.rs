//! Organization calendar subscribe link.

use crate::icons::{Icon, IconName};
use leptos::prelude::*;

/// Calendar subscribe link — opens the organization's Google Calendar.
///
/// Only shown when `subscribe_url` is set on the event config.
#[component]
pub fn CalendarLinks(
    /// Google Calendar embed/subscribe URL from event config.
    subscribe_url: String,
) -> impl IntoView {
    if subscribe_url.is_empty() {
        return view! { <div></div> }.into_any();
    }

    let url = subscribe_url.clone();

    view! {
        <div class="ticket-calendar-links">
            <a
                href=url
                target="_blank"
                rel="noopener noreferrer"
                class="ticket-calendar-link"
            >
                <Icon icon=IconName::Calendar class="icon-sm" />
                "📅 Our Event Calendar"
            </a>
        </div>
    }
    .into_any()
}
