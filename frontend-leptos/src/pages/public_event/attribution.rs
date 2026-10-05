//! Who is behind the event: the organizer line under the title (F7-a) and the
//! sponsor logo row above the footer (F7-b). Both render nothing when empty.
//!
//! Sponsor URLs are `https://`-only at the write (`normalize_sponsors`); the
//! check is repeated here because a value can reach the page without passing
//! that write (a KV entry written before the column, a spreadsheet import).

use event_checkin_domain::models::event::Sponsor;
use leptos::prelude::*;

/// "Organized by {name}" as a quiet line. Plain text, no link.
pub fn organizer_line(name: &str) -> AnyView {
    let name = name.trim().to_string();
    if name.is_empty() {
        return ().into_any();
    }
    view! {
        <p class="pe-organizer">
            {crate::locale::tr(|l| crate::i18n::td_string!(l, event.organized_by))}" "{name}
        </p>
    }
    .into_any()
}

/// Sponsor logos, greyscale until hovered. A sponsor without a logo shows its
/// name; one without a link is not a link.
pub fn sponsor_row(sponsors: &[Sponsor]) -> AnyView {
    let items: Vec<AnyView> = sponsors
        .iter()
        .filter(|s| !s.name.trim().is_empty())
        .map(sponsor_item)
        .collect();
    if items.is_empty() {
        return ().into_any();
    }
    view! {
        <section class="pe-sponsors" aria-labelledby="pe-sponsors-title">
            <h2 id="pe-sponsors-title" class="pe-sponsors-title">
                {crate::locale::tr(|l| crate::i18n::td_string!(l, event.sponsors_title))}
            </h2>
            <ul class="pe-sponsors-list">{items}</ul>
        </section>
    }
    .into_any()
}

fn sponsor_item(s: &Sponsor) -> AnyView {
    let name = s.name.trim().to_string();
    let mark = match https_only(&s.logo_url) {
        Some(src) => view! {
            <img class="pe-sponsor-logo" src=src alt=name.clone() loading="lazy" decoding="async" />
        }
        .into_any(),
        None => view! { <span class="pe-sponsor-name">{name.clone()}</span> }.into_any(),
    };
    let inner = match https_only(&s.link) {
        Some(href) => view! {
            <a class="pe-sponsor" href=href target="_blank" rel="noopener noreferrer sponsored">{mark}</a>
        }
        .into_any(),
        None => view! { <span class="pe-sponsor">{mark}</span> }.into_any(),
    };
    view! { <li>{inner}</li> }.into_any()
}

fn https_only(url: &str) -> Option<String> {
    let url = url.trim();
    url.strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty())
        .then(|| url.to_string())
}
