//! Access & Logistics section — shows organizer-curated guide links
//! (building access, ID exchange procedure, transportation) to confirmed
//! in-person attendees.
//!
//! These are `CommunityLink` entries with `platform == "guide"`, separated
//! from the social/community links so they don't appear under
//! "Join the Community". The `label` field carries the display name
//! (e.g. "Building Access Guide", "ID Exchange Procedure", "Transportation").
//!
//! Rendered only on the in-person ticket view — online attendees don't need
//! physical access info. The wording of the section title matches the
//! promise made in the attendee calendar/email comms so attendees find what
//! they were told to look for.

use leptos::prelude::*;

use crate::api::CommunityLink;
use crate::icons::{Icon, IconName};

/// Platform tag that marks a `CommunityLink` as a logistics guide rather than
/// a social/community link. Filtered out of `community_links_section` and
/// routed here instead.
pub const GUIDE_PLATFORM: &str = "guide";

/// Arrow icon shown on each guide link row — matches the community-links
/// pattern for visual consistency.
const ARROW_SVG: &str = r#"<svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3l5 5-5 5"/></svg>"#;

/// Render the "Access & Logistics" card from the guide-tagged subset of
/// `community_links`. Returns nothing if there are no guide links, so the
/// section is invisible for events that haven't configured any.
pub fn access_logistics_section(links: Vec<CommunityLink>) -> impl IntoView {
    // Filter to guide links with non-empty URLs.
    let guides: Vec<_> = links
        .into_iter()
        .filter(|l| l.platform == GUIDE_PLATFORM && !l.url.is_empty())
        .collect();

    if guides.is_empty() {
        return ().into_any();
    }

    let items: Vec<_> = guides
        .into_iter()
        .map(|link| {
            // label drives the display; fall back to a generic term if blank.
            let display_label = if link.label.is_empty() {
                "View Guide".to_string()
            } else {
                link.label.clone()
            };
            let url = link.url.clone();
            view! {
                <a
                    href=url
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
                    <Icon icon=IconName::Map class="icon-sm" />
                    <span>"Access & Logistics"</span>
                </div>
                <p class="access-logistics-hint">
                    "Review before you arrive — building access, ID exchange, and transportation."
                </p>
                <div class="access-logistics-list">
                    {items}
                </div>
            </div>
        </div>
    }
    .into_any()
}
