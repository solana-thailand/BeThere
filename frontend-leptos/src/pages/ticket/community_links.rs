//! Community links section — shows Discord, Telegram, X, Facebook, Line, website links.

use leptos::prelude::*;

use crate::api::CommunityLink;
use crate::pages::ticket::access_logistics::GUIDE_PLATFORM;

/// Render a platform icon SVG.
fn platform_icon(platform: &str) -> &'static str {
    match platform {
        "discord" => {
            r#"<svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M20.317 4.37a19.791 19.791 0 00-4.885-1.515.074.074 0 00-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 00-5.487 0 12.64 12.64 0 00-.617-1.25.077.077 0 00-.079-.037A19.736 19.736 0 003.677 4.37a.07.07 0 00-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 00.031.057 19.9 19.9 0 005.993 3.03.078.078 0 00.084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 00-.041-.106 13.107 13.107 0 01-1.872-.892.077.077 0 01-.008-.128 10.2 10.2 0 00.372-.292.074.074 0 01.077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 01.078.01c.12.098.246.198.373.292a.077.077 0 01-.006.127 12.299 12.299 0 01-1.873.892.077.077 0 00-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 00.084.028 19.839 19.839 0 006.002-3.03.077.077 0 00.032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 00-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/></svg>"#
        }
        "telegram" => {
            r#"<svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M11.944 0A12 12 0 000 12a12 12 0 0012 12 12 12 0 0012-12A12 12 0 0012 0a12 12 0 00-.056 0zm4.962 7.224c.1-.002.321.023.465.14a.506.506 0 01.171.325c.016.093.036.306.02.472-.18 1.898-.962 6.502-1.36 8.627-.168.9-.499 1.201-.82 1.23-.696.065-1.225-.46-1.9-.902-1.056-.693-1.653-1.124-2.678-1.8-1.185-.78-.417-1.21.258-1.91.177-.184 3.247-2.977 3.307-3.23.007-.032.014-.15-.056-.212s-.174-.041-.249-.024c-.106.024-1.793 1.14-5.061 3.345-.479.33-.913.49-1.302.48-.428-.008-1.252-.241-1.865-.44-.752-.245-1.349-.374-1.297-.789.027-.216.325-.437.893-.663 3.498-1.524 5.83-2.529 6.998-3.014 3.332-1.386 4.025-1.627 4.476-1.635z"/></svg>"#
        }
        "x" => {
            r#"<svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/></svg>"#
        }
        "facebook" => {
            r#"<svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M24 12.073c0-6.627-5.373-12-12-12s-12 5.373-12 12c0 5.99 4.388 10.954 10.125 11.854v-8.385H7.078v-3.47h3.047V9.43c0-3.007 1.792-4.669 4.533-4.669 1.312 0 2.686.235 2.686.235v2.953H15.83c-1.491 0-1.956.925-1.956 1.874v2.25h3.328l-.532 3.47h-2.796v8.385C19.612 23.027 24 18.062 24 12.073z"/></svg>"#
        }
        "line" => {
            r#"<svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M19.365 9.863c.349 0 .63.285.63.631 0 .345-.281.63-.63.63H17.61v1.125h1.755c.349 0 .63.283.63.63 0 .344-.281.629-.63.629h-2.386c-.345 0-.627-.285-.627-.629V8.108c0-.345.282-.63.63-.63h2.386c.346 0 .627.285.627.63 0 .349-.281.63-.63.63H17.61v1.125h1.755zm-3.855 3.016c0 .27-.174.51-.432.596-.064.021-.133.031-.199.031-.211 0-.391-.09-.51-.25l-2.443-3.317v2.94c0 .344-.279.629-.631.629-.346 0-.626-.285-.626-.629V8.108c0-.27.173-.51.43-.595.06-.023.136-.033.194-.033.195 0 .375.104.495.254l2.462 3.33V8.108c0-.345.282-.63.63-.63.345 0 .63.285.63.63v4.141h1.756c.348 0 .629.283.629.63 0 .344-.282.629-.629.629M24 10.314C24 4.943 18.615.572 12 .572S0 4.943 0 10.314c0 4.811 4.27 8.842 10.035 9.608.391.082.923.258 1.058.59.12.301.079.766.038 1.08l-.164 1.02c-.045.301-.24 1.186 1.049.645 1.291-.539 6.916-4.078 9.436-6.975C23.176 14.393 24 12.458 24 10.314"/></svg>"#
        }
        _ => {
            r#"<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>"#
        }
    }
}

/// Platform display name for labels.
fn platform_name(platform: &str) -> &'static str {
    match platform {
        "discord" => "Discord",
        "telegram" => "Telegram",
        "x" => "X (Twitter)",
        "facebook" => "Facebook",
        "line" => "LINE",
        _ => "Website",
    }
}

/// CSS class suffix per platform for brand-colored icons on the public event page.
fn platform_class(platform: &str) -> &'static str {
    match platform {
        "discord" => "pe-cl-discord",
        "telegram" => "pe-cl-telegram",
        "x" => "pe-cl-x",
        "facebook" => "pe-cl-facebook",
        "line" => "pe-cl-line",
        _ => "pe-cl-website",
    }
}

/// Which page context the component renders in.
#[derive(Clone, Copy, PartialEq)]
pub enum CommunityLinksVariant {
    /// Ticket page — uses ticket-action-card styling
    Ticket,
    /// Public event page — uses pe-card styling with section title
    PublicEvent,
}

/// Community links section — renders a "Join the Community" card.
/// Adapts its styling based on page context.
pub fn community_links_section(
    links: Vec<CommunityLink>,
    variant: CommunityLinksVariant,
) -> impl IntoView {
    if links.is_empty() {
        return ().into_any();
    }

    let filtered: Vec<_> = links
        .into_iter()
        // Exclude guide links — they are logistics docs (building access,
        // ID exchange, transportation) rendered by `access_logistics_section`,
        // not social/community links.
        .filter(|l| !l.url.is_empty() && l.platform != GUIDE_PLATFORM)
        .collect();

    if filtered.is_empty() {
        return ().into_any();
    }

    match variant {
        CommunityLinksVariant::Ticket => render_ticket_variant(filtered).into_any(),
        CommunityLinksVariant::PublicEvent => render_public_event_variant(filtered).into_any(),
    }
}

/// Ticket page variant — compact card with accent border.
fn render_ticket_variant(links: Vec<CommunityLink>) -> impl IntoView {
    let items: Vec<_> = links
        .into_iter()
        .map(|link| {
            let icon = platform_icon(&link.platform);
            let display_label = if link.label.is_empty() {
                platform_name(&link.platform).to_string()
            } else {
                link.label.clone()
            };
            let url = link.url.clone();
            view! {
                <a
                    href=url
                    target="_blank"
                    rel="noopener noreferrer"
                    class="community-link-item"
                >
                    <span class="community-link-icon" inner_html=icon />
                    <span class="community-link-label">{display_label}</span>
                </a>
            }
        })
        .collect();

    view! {
        <div class="ticket-action-card ticket-action-card--community">
            <div class="community-links-inner">
                <div class="community-links-title">"Join the Community"</div>
                <div class="community-links-list">
                    {items}
                </div>
            </div>
        </div>
    }
}

/// Public event page variant — matches pe-card design system with section title.
fn render_public_event_variant(links: Vec<CommunityLink>) -> impl IntoView {
    let items: Vec<_> = links
        .into_iter()
        .map(|link| {
            let icon = platform_icon(&link.platform);
            let cls = platform_class(&link.platform);
            let display_label = if link.label.is_empty() {
                platform_name(&link.platform).to_string()
            } else {
                link.label.clone()
            };
            let url = link.url.clone();
            view! {
                <a
                    href=url
                    target="_blank"
                    rel="noopener noreferrer"
                    class="pe-community-link-item"
                >
                    <span class=format!("pe-community-link-icon {cls}") inner_html=icon />
                    <span class="pe-community-link-label">{display_label}</span>
                    <svg class="pe-community-link-arrow" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M6 3l5 5-5 5"/>
                    </svg>
                </a>
            }
        })
        .collect();

    view! {
        <div class="pe-card">
            <h2 class="pe-section-title">
                <span class="pe-community-title-icon" inner_html=r#"<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>"# />
                "Join the Community"
            </h2>
            <p class="pe-detail-secondary pe-mb-075">
                "Connect with fellow attendees before and after the event."
            </p>
            <div class="pe-community-links-list">
                {items}
            </div>
        </div>
    }
}
