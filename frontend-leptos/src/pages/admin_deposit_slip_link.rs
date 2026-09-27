//! "View Slip" link shared by the admin deposit tabs (deposits, refund queue,
//! refunded, held as credit), so the sentinel check cannot drift between them.

use leptos::prelude::*;

/// Whether a deposit's `slip_url` is a real serving path. Credit-covered and
/// staff-comp deposits store a sentinel (`ROLLING_CREDIT_AUTO_APPLIED` /
/// `STAFF_COMP_WAIVED`) in `slip_url` instead — not a URL, so no link.
pub fn is_viewable_slip_url(slip_url: Option<&str>) -> bool {
    slip_url.is_some_and(|u| u.starts_with("/api/") || u.starts_with("http"))
}

/// The "View Slip" row, or an empty span when there is no viewable slip.
pub fn slip_link(slip_url: Option<String>) -> AnyView {
    let has_slip_url = is_viewable_slip_url(slip_url.as_deref());
    view! {
        <Show when=move || has_slip_url fallback=|| view! { <span></span> }>
            <div class="admin-dep-slip-link-row">
                <a
                    href=slip_url.clone().unwrap_or_default()
                    target="_blank"
                    rel="noopener noreferrer"
                    class="link-accent"
                >
                    "View Slip"
                </a>
            </div>
        </Show>
    }
    .into_any()
}
