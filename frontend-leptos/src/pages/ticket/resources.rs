//! Resources section — the organizer's slides, source code and downloads,
//! shown on the ticket as soon as they are added.
//!
//! These are `CommunityLink` entries whose platform is one of
//! [`LEARNING_RESOURCE_PLATFORMS`]. The published recap shows the same links
//! after the event; the ticket shows them before and during it, so an
//! attendee can open the slides in the room. They stay out of
//! "Join the Community", which is for social channels.

use leptos::prelude::*;

use crate::api::CommunityLink;
use crate::i18n::{t_string, use_i18n};
use crate::icons::{Icon, IconName};

/// Platforms that mark a `CommunityLink` as a learning resource. The Worker's
/// recap filter (`public_learning_resources`) uses the same four.
pub const LEARNING_RESOURCE_PLATFORMS: &[&str] = &["resource", "slides", "source", "download"];

/// Same arrow as the Access & Logistics rows.
const ARROW_SVG: &str = r#"<svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3l5 5-5 5"/></svg>"#;

/// The learning-resource links worth a row, in the organizer's order. Only
/// `https://` URLs: the field is free text, and the recap applies the same rule
/// on the Worker side.
pub fn learning_resources(links: &[CommunityLink]) -> Vec<CommunityLink> {
    links
        .iter()
        .filter(|l| LEARNING_RESOURCE_PLATFORMS.contains(&l.platform.as_str()))
        .filter(|l| l.url.trim().starts_with("https://"))
        .cloned()
        .collect()
}

/// Render the "Resources" card, or nothing when the event has none.
pub fn resources_section(links: Vec<CommunityLink>) -> impl IntoView {
    let resources = learning_resources(&links);
    if resources.is_empty() {
        return ().into_any();
    }

    let i18n = use_i18n();
    let items: Vec<_> = resources
        .into_iter()
        .map(|link| {
            // The organizer's label as written, then the kind, as on the recap.
            let platform = link.platform.clone();
            let label = link.label.clone();
            let display_label = move || {
                let kind = match platform.as_str() {
                    "slides" => t_string!(i18n, recap.kind.slides),
                    "source" => t_string!(i18n, recap.kind.source),
                    "download" => t_string!(i18n, recap.kind.download),
                    _ => t_string!(i18n, recap.kind.resource),
                };
                match label.trim().is_empty() {
                    true => kind.to_string(),
                    false => format!("{label} · {kind}"),
                }
            };
            view! {
                <a
                    href=link.url.trim().to_string()
                    target="_blank"
                    rel="noopener noreferrer"
                    class="access-logistics-item"
                >
                    <span class="access-logistics-label">{display_label}</span>
                    <span class="access-logistics-arrow" inner_html=ARROW_SVG />
                </a>
            }
        })
        .collect();

    view! {
        <div class="ticket-action-card ticket-action-card--info access-logistics-card">
            <div class="access-logistics-inner">
                <div class="access-logistics-title">
                    <Icon icon=IconName::Link class="icon-sm" />
                    <span>{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.resources.title))}</span>
                </div>
                <p class="access-logistics-hint">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.resources.hint))}
                </p>
                <div class="access-logistics-list">
                    {items}
                </div>
            </div>
        </div>
    }
    .into_any()
}
