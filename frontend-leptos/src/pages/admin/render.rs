//! Admin dashboard read-only panels: stats, QR result, walk-in sync result,
//! check-in velocity and recent check-ins.

use std::collections::HashMap;

use event_checkin_domain::models::attendee::RECENT_CHECK_INS_PER_TYPE;
use leptos::prelude::*;

use super::types::DashboardTab;
use crate::api::{self, AttendeeListItem, EventFormat, GenerateQrData, StatsResponse};
use crate::pages::admin_duplicate_hint::duplicate_hint;
use crate::utils;

// ===== Render Functions =====

/// Render tab-aware stats cards and progress bar.
pub(super) fn render_stats(
    stats: &Option<StatsResponse>,
    attendees: &[AttendeeListItem],
    tab: DashboardTab,
    event_format: &EventFormat,
) -> AnyView {
    match stats {
        Some(_s) => {
            // Compute counts for this tab
            let tab_attendees: Vec<_> = attendees
                .iter()
                .filter(|a| tab.matches(&a.participation_type))
                .collect();

            let tab_total = tab_attendees.len();
            let tab_checked_in = tab_attendees
                .iter()
                .filter(|a| a.checked_in_at.is_some())
                .count();
            let tab_remaining = tab_total.saturating_sub(tab_checked_in);
            let tab_percentage = if tab_total > 0 {
                (tab_checked_in as f64 / tab_total as f64) * 100.0
            } else {
                0.0
            };
            let remaining_percentage = if tab_total > 0 {
                (tab_remaining as f64 / tab_total as f64) * 100.0
            } else {
                0.0
            };

            // Also show the other tab count as a summary line
            let other_tab = match tab {
                DashboardTab::InPerson => DashboardTab::Online,
                DashboardTab::Online => DashboardTab::InPerson,
            };
            let other_count = attendees
                .iter()
                .filter(|a| other_tab.matches(&a.participation_type))
                .count();

            view! {
                <div class="stats-grid">
                    <div class="stat-card info">
                        <div class="stat-value">{tab_total}</div>
                        <div class="stat-label">{format!("{} Total", tab.label())}</div>
                    </div>
                    <div class="stat-card success">
                        <div class="stat-value">{tab_checked_in}</div>
                        <div class="stat-label">"Checked In"</div>
                        <div class="stat-progress">
                            <div class="stat-progress-fill" style=format!("width: {tab_percentage:.1}%")></div>
                        </div>
                    </div>
                    <div class="stat-card warning">
                        <div class="stat-value">{tab_remaining}</div>
                        <div class="stat-label">"Remaining"</div>
                        <div class="stat-progress">
                            <div class="stat-progress-fill" style=format!("width: {remaining_percentage:.1}%")></div>
                        </div>
                    </div>
                </div>

                // Cross-tab summary — only for hybrid events with both tracks
                {if event_format == &EventFormat::Hybrid {
                    view! {
                        <div class="admin-cross-tab-summary">
                            {format!("{} {} attendee{}", other_count, other_tab.label(), if other_count != 1 { "s" } else { "" })}
                            // Plain text: this was styled as a link with an empty
                            // click handler, so it looked like it switched tabs
                            // and did nothing (.plans/037 §6).
                            " — use the sidebar to switch track"
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}

                // Progress bar
                <div class="card mb-2">
                    <div class="admin-progress-header">
                        // Named by its denominator: this card counts one track,
                        // the one below counts every registrant. Side by side,
                        // "81.2% (26/32)" and "37% (26/69)" read as a
                        // contradiction (.plans/037 §4).
                        <span class="admin-progress-title">
                            {format!("{} checked in", tab.label())}
                        </span>
                        <span class="admin-progress-pct">
                            {format!(
                                "{tab_percentage:.1}% · {tab_checked_in} of {tab_total} {} registrants",
                                tab.label().to_lowercase()
                            )}
                        </span>
                    </div>
                    <div class="progress-bar">
                        <div
                            class="progress-fill"
                            style=move || format!("width: {tab_percentage}%")
                        ></div>
                    </div>
                </div>
            }
                .into_any()
        }
        None => view! { <div></div> }.into_any(),
    }
}

/// Render QR generation result summary.
pub(super) fn render_qr_result(data: &Option<GenerateQrData>) -> AnyView {
    match data {
        Some(d) => {
            let generated = d.generated;
            let skipped = d.skipped;
            let has_skipped = skipped > 0;
            view! {
                <div class="card mb-2 admin-qr-result-card">
                    <div class="admin-qr-result-header">

                        <span class="admin-qr-result-title">
                            "QR Codes Generated"
                        </span>
                    </div>
                    <div class="admin-qr-stats-row">
                        <div>
                            <span class="admin-qr-count-success">{generated}</span>
                            <span class="admin-qr-count-label">" created"</span>
                        </div>
                        <Show when=move || has_skipped fallback=|| view! { <div></div> }>
                            <div>
                                <span class="admin-qr-count-warning">{skipped}</span>
                                <span class="admin-qr-count-label">" skipped (already exist)"</span>
                            </div>
                        </Show>
                    </div>
                </div>
            }
            .into_any()
        }
        None => view! { <div></div> }.into_any(),
    }
}

/// Render the recent check-ins panel, filtered by tab.
pub(super) fn render_recent_check_ins(
    stats: &Option<StatsResponse>,
    attendees: &[AttendeeListItem],
    tab: DashboardTab,
) -> AnyView {
    match stats {
        Some(s) if !s.recent_check_ins.is_empty() => {
            // Roster rows by api_id: the stats feed carries only name and time,
            // so participation type and the duplicate hint come from here.
            let roster: HashMap<&str, &AttendeeListItem> =
                attendees.iter().map(|a| (a.api_id.as_str(), a)).collect();

            let recent: Vec<_> = {
                let mut r = s.recent_check_ins.clone();
                r.sort_by(|a, b| {
                    let a_time = js_sys::Date::parse(&a.checked_in_at);
                    let b_time = js_sys::Date::parse(&b.checked_in_at);
                    b_time
                        .partial_cmp(&a_time)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                // Filter by active tab
                r.into_iter()
                    .filter(|ci| {
                        let p_type = roster
                            .get(ci.api_id.as_str())
                            .map_or("", |a| a.participation_type.as_str());
                        tab.matches(p_type)
                    })
                    .take(RECENT_CHECK_INS_PER_TYPE)
                    .collect()
            };

            if recent.is_empty() {
                return view! {
                    <div class="card mt-3">
                        <h3 class="admin-section-heading">"Recent Check-Ins"</h3>
                        <div class="admin-empty-state-sm">
                            {format!("No recent {} check-ins", tab.label().to_lowercase())}
                        </div>
                    </div>
                }
                .into_any();
            }

            view! {
                <div class="card mt-3">
                    <h3 class="admin-section-heading">
                        {format!("Recent {} Check-Ins", tab.label())}
                    </h3>
                    <div class="attendee-list">
                        {recent.iter().map(|check_in| {
                            let name = check_in.name.clone();
                            let api_id = check_in.api_id.clone();
                            let at = check_in.checked_in_at.clone();
                            let formatted = utils::format_timestamp(&at);
                            let by_suffix = check_in.checked_in_by.as_ref().map_or(String::new(), |by| {
                                if by.is_empty() { String::new() } else { format!(" by {by}") }
                            });

                            let row = roster.get(api_id.as_str());
                            let p_type = row.map_or("", |a| a.participation_type.as_str());
                            let participation = utils::get_participation_badge(p_type);
                            let p_class = participation.css_class.to_string();
                            let p_label = participation.label;
                            let duplicate_badge =
                                row.and_then(|a| duplicate_hint(&a.possible_duplicates));

                            view! {
                                <div class="attendee-item">
                                    <div class="attendee-row-top">
                                        <div class="attendee-name">{name.clone()}</div>
                                        <span class=format!("{p_class} admin-badge-inline")>
                                            {p_label.clone()}
                                        </span>
                                        {duplicate_badge}
                                    </div>
                                    <div class="attendee-row-bottom">
                                        <div class="attendee-meta">
                                            <span class="attendee-email-inline admin-recent-email">
                                                {api_id.clone()}
                                            </span>
                                        </div>
                                        <div class="admin-checkin-time">
                                            {formatted}{by_suffix}
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                </div>
            }
                .into_any()
        }
        _ => view! { <div></div> }.into_any(),
    }
}

/// Render the walk-in sync result card.
pub(super) fn render_walkin_sync_result(result: Option<api::WalkinSyncResponse>) -> AnyView {
    match result {
        Some(r) => {
            let errors = r.errors.clone();
            let has_errors = !errors.is_empty();
            view! {
                <div class="admin-info-card">
                    <div class="admin-info-card-header">"Walk-in Sync Result"</div>
                    <div class="admin-info-card-body">
                        <div>"Synced: "<strong>{r.synced}</strong></div>
                        <div>"Skipped (already synced): "<strong>{r.skipped}</strong></div>
                        <div>"Total walk-ins: "<strong>{r.total_walkins}</strong></div>
                        <Show
                            when=move || has_errors
                            fallback=|| view! { <div></div> }
                        >
                            <div class="admin-sync-errors">
                                <strong>"Errors:"</strong>
                                <ul>
                                    {errors.iter().map(|e| view! {
                                        <li>{e.clone()}</li>
                                    }).collect_view()}
                                </ul>
                            </div>
                        </Show>
                    </div>
                </div>
            }
            .into_any()
        }
        None => view! { <div></div> }.into_any(),
    }
}

/// Render the all-tracks check-in bar. Online registrants are in the
/// denominator, so it is lower than the per-track card above it.
pub(super) fn render_velocity(list: &[AttendeeListItem]) -> impl IntoView + use<> {
    let total = list.len();
    let checked_in = list.iter().filter(|a| a.checked_in_at.is_some()).count();
    let pct = if total > 0 {
        (checked_in as f64 / total as f64 * 100.0) as u32
    } else {
        0
    };
    view! {
        <div style="background: rgba(19, 20, 28, 0.6); border: 1px solid rgba(153, 69, 255, 0.25); border-radius: 14px; padding: 14px 18px; margin: 16px 0; backdrop-filter: blur(12px);">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                <span style="font-weight: 700; color: #fff; font-size: 0.88rem; display: flex; align-items: center; gap: 6px;">
                    <span style="color: #14F195;">"⚡"</span>" All tracks checked in"
                </span>
                <span style="font-weight: 800; color: #14F195; font-size: 0.88rem;">
                    {pct}"% · "{checked_in}" of "{total}" registrants, online included"
                </span>
            </div>
            <div style="width: 100%; height: 8px; background: rgba(255,255,255,0.08); border-radius: 8px; overflow: hidden;">
                <div style=format!("width: {pct}%; height: 100%; background: linear-gradient(90deg, #9945FF, #14F195); border-radius: 8px; transition: width 0.4s ease;")></div>
            </div>
        </div>
    }
}
