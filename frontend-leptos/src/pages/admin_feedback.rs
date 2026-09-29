//! Admin Post-Event Feedback & Survey Analytics Dashboard (Issue #113).
//!
//! Visualizes attendee satisfaction ratings (Content, Venue, Catering, Promotion),
//! online viewing habits, continuation interest, individual qualitative feedback,
//! and provides one-click CSV export with cross-event and series aggregation.

use leptos::prelude::*;

use crate::api::{self, AdminFeedbackResponse};
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};

#[component]
pub fn AdminFeedback(
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    active_event_id: ReadSignal<Option<String>>,
) -> impl IntoView {
    let (feedback_data, set_feedback_data) = signal(None::<AdminFeedbackResponse>);
    let (loading, set_loading) = signal(false);
    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let (search_query, set_search_query) = signal(String::new());
    let (filter_type, set_filter_type) = signal("all".to_string());
    let (scope, set_scope) = signal("event".to_string()); // "event" | "series" | "all"
    let (series_name, set_series_name) = signal(None::<String>);

    // Reset scope to "event" whenever active event changes
    let tracked_event_for_reset = active_event_id;
    Effect::new(move |_| {
        let _ = tracked_event_for_reset.get();
        set_scope.set("event".to_string());
        set_series_name.set(None);
    });

    // Load feedback data when active event, scope, or refresh counter changes
    let tracked_event_id = active_event_id;
    Effect::new(move |_| {
        let _ = refresh_counter.get();
        let current_scope = scope.get();
        let eid = tracked_event_id.get();

        if current_scope != "all" && eid.is_none() {
            set_feedback_data.set(None);
            return;
        }

        let eid_val = eid.clone();
        set_loading.set(true);

        leptos::task::spawn_local(async move {
            let res = match current_scope.as_str() {
                "all" => api::get_admin_feedback(None, Some("all")).await,
                "series" => api::get_admin_feedback(eid_val.as_deref(), Some("series")).await,
                _ => api::get_admin_feedback(eid_val.as_deref(), Some("event")).await,
            };

            match res {
                Ok(data) => {
                    if data.series_name.is_some() {
                        set_series_name.set(data.series_name.clone());
                    }
                    set_feedback_data.set(Some(data));
                    set_loading.set(false);
                }
                Err(e) => {
                    log::warn!("[admin-feedback] Failed to load feedback: {e}");
                    set_toast.set(Some(components::ToastMessage {
                        text: format!("Failed to load feedback: {e}"),
                        toast_type: ToastType::Error,
                    }));
                    set_feedback_data.set(None);
                    set_loading.set(false);
                }
            }
        });
    });

    let handle_refresh = move |_| {
        set_refresh_counter.update(|c| *c += 1);
    };

    let handle_download_csv = move |_| {
        if let Some(ref data) = feedback_data.get() {
            if let Some(ref csv_text) = data.csv {
                let filename = data
                    .filename
                    .clone()
                    .unwrap_or_else(|| format!("feedback-{}.csv", data.event_id));
                crate::pages::admin::download_csv(&filename, csv_text);
                set_toast.set(Some(components::ToastMessage {
                    text: format!("Downloaded {filename}"),
                    toast_type: ToastType::Success,
                }));
            } else {
                set_toast.set(Some(components::ToastMessage {
                    text: "No CSV data available".to_string(),
                    toast_type: ToastType::Warning,
                }));
            }
        }
    };

    let has_event = move || scope.get() == "all" || active_event_id.get().is_some();

    view! {
        <div class="admin-feedback-page">
            // Section Header
            <div class="admin-section-header afb-head">
                <div>
                    <h3 class="afb-title">
                        <Icon icon=IconName::Star />
                        "Post-Event Survey & Feedback"
                    </h3>
                    <p class="admin-section-subtitle afb-subtitle">
                        "Review verified attendee satisfaction ratings, online viewing engagement, and session comments."
                    </p>
                </div>

                <div class="afb-row">
                    <button
                        class="btn btn-outline btn-sm afb-row-sm"
                        on:click=handle_refresh
                        disabled=move || loading.get()
                    >
                        <Icon icon=IconName::Refresh />
                        {move || if loading.get() { "Loading..." } else { "Refresh" }}
                    </button>

                    <button
                        class="btn btn-primary btn-sm afb-row-sm"
                        on:click=handle_download_csv
                        disabled=move || feedback_data.get().is_none() || loading.get()
                    >
                        <Icon icon=IconName::Clip />
                        "Export CSV"
                    </button>
                </div>
            </div>

            // Aggregation Scope Selector Bar
            <div class="afb-scopebar">
                <div class="afb-row-meta">
                    <span class="afb-strong">"Scope:"</span>
                    <div class="afb-segmented">
                        <button
                            class=move || match scope.get() == "event" {
                                true => "btn btn-sm afb-seg-btn is-active",
                                false => "btn btn-sm afb-seg-btn",
                            }
                            on:click=move |_| set_scope.set("event".to_string())
                        >
                            "This Event"
                        </button>
                        <Show when=move || series_name.get().is_some() fallback=|| view! { <div></div> }>
                            <button
                                class=move || match scope.get() == "series" {
                                    true => "btn btn-sm afb-seg-btn is-active",
                                    false => "btn btn-sm afb-seg-btn",
                                }
                                on:click=move |_| set_scope.set("series".to_string())
                                title="Aggregate feedback across all parts in this series"
                            >
                                {move || format!("Series: {}", series_name.get().unwrap_or_default())}
                            </button>
                        </Show>
                        <button
                            class=move || match scope.get() == "all" {
                                true => "btn btn-sm afb-seg-btn is-active",
                                false => "btn btn-sm afb-seg-btn",
                            }
                            on:click=move |_| set_scope.set("all".to_string())
                            title="Aggregate feedback across all events"
                        >
                            "All Events"
                        </button>
                    </div>
                </div>

                <div class="afb-small-dim">
                    {move || {
                        if let Some(ref data) = feedback_data.get() {
                            if data.events_included.len() > 1 {
                                format!("Aggregated {} events • {} total submissions", data.events_included.len(), data.total_respondents)
                            } else {
                                format!("Viewing: {}", data.event_name)
                            }
                        } else {
                            String::new()
                        }
                    }}
                </div>
            </div>

            // Included sessions pill list (when aggregating multiple events)
            <Show when=move || feedback_data.get().is_some_and(|d| d.events_included.len() > 1) fallback=|| view! { <div></div> }>
                {move || {
                    let events = feedback_data.get().map(|d| d.events_included).unwrap_or_default();
                    view! {
                        <div class="afb-callout">
                            <div class="afb-kicker-row afb-sky">
                                <span>"📊"</span> "Included Sessions in this Aggregation"
                            </div>
                            <div class="afb-wrap">
                                {events.into_iter().map(|ev| {
                                    view! {
                                        <span class="badge badge-sm afb-chip">
                                            <strong class="afb-quote-mark">{ev.respondent_count}</strong>
                                            {ev.event_name}
                                        </span>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>
                    }
                }}
            </Show>

            // No event selected state
            <Show when=move || !has_event() fallback=|| view! { <div></div> }>
                <div class="admin-empty-state afb-empty">
                    <p class="afb-muted">"Select an event from the top selector or choose 'All Events' above to inspect survey responses."</p>
                </div>
            </Show>

            // Loading state
            <Show when=move || loading.get() && feedback_data.get().is_none() fallback=|| view! { <div></div> }>
                <div class="page-loading afb-loading">
                    <span class="spinner spinner-sm"></span>
                    <span>"Loading post-event survey data..."</span>
                </div>
            </Show>

            // Loaded Content
            <Show when=move || feedback_data.get().is_some() fallback=|| view! { <div></div> }>
                {move || {
                    let data = feedback_data.get().unwrap_or_default();
                    let total = data.total_respondents;
                    let is_multi_event = data.events_included.len() > 1;

                    if total == 0 {
                        return view! {
                            <div class="admin-empty-state afb-empty-lg">
                                <div class="afb-empty-icon">"📝"</div>
                                <h4 class="afb-mb-sm">"No Survey Responses Yet"</h4>
                                <p class="afb-empty-text">
                                    "No attendees have submitted post-event feedback for "<strong class="afb-white">{data.event_name.clone()}</strong>" yet. Responses will appear here in real-time as attendees fill out the survey."
                                </p>
                            </div>
                        }.into_any();
                    }

                    // Extract dimension metrics
                    let content_dim = data.dimensions.iter().find(|d| d.key.contains("content"));
                    let venue_dim = data.dimensions.iter().find(|d| d.key.contains("venue"));
                    let catering_dim = data.dimensions.iter().find(|d| d.key.contains("catering"));
                    let promo_dim = data.dimensions.iter().find(|d| d.key.contains("promotion"));

                    let has_online_watched = !data.online_watched.is_empty();
                    let has_latent_space_continue = !data.latent_space_continue.is_empty();

                    let filtered_rows = {
                        let q = search_query.get().to_lowercase();
                        let filter = filter_type.get();
                        data.respondents.into_iter().filter(move |r| {
                            let match_type = match filter.as_str() {
                                "onsite" => r.participation_type.to_lowercase().contains("in_person") || r.participation_type.to_lowercase().contains("in-person") || r.participation_type.to_lowercase().contains("onsite"),
                                "online" => r.participation_type.to_lowercase().contains("online"),
                                _ => true,
                            };
                            if !match_type {
                                return false;
                            }
                            if q.is_empty() {
                                return true;
                            }
                            r.name.to_lowercase().contains(&q)
                                || r.email.to_lowercase().contains(&q)
                                || r.event_name.as_deref().unwrap_or("").to_lowercase().contains(&q)
                                || r.comment.as_deref().unwrap_or("").to_lowercase().contains(&q)
                                || r.next_topics.as_deref().unwrap_or("").to_lowercase().contains(&q)
                        }).collect::<Vec<_>>()
                    };

                    view! {
                        <div class="afb-page">
                            // Top KPI Overview Cards
                            <div class="afb-grid-sm">
                                // Total Card
                                <div class="afb-card">
                                    <div class="afb-kicker">
                                        "Respondents"
                                    </div>
                                    <div class="afb-stat-value">
                                        {total}
                                    </div>
                                    <div class="afb-stat-foot">
                                        <span class="badge badge-sm badge-neutral">{format!("Onsite: {}", data.onsite_respondents)}</span>
                                        <span class="badge badge-sm badge-neutral">{format!("Online: {}", data.online_respondents)}</span>
                                    </div>
                                </div>

                                // Content Satisfaction Card
                                <div class="afb-card">
                                    <div class="afb-kicker afb-sky">
                                        "Content Rating"
                                    </div>
                                    <div class="afb-stat-value">
                                        {content_dim.map(|d| format!("{:.2}", d.average_score)).unwrap_or_else(|| "-".to_string())}
                                        <span class="afb-stat-unit">"/ 3.00"</span>
                                    </div>
                                    <div class="afb-stat-note-good">
                                        {content_dim.map(|d| format!("{:.0}% satisfied ({} ratings)", d.positive_percentage, d.total_answers)).unwrap_or_default()}
                                    </div>
                                </div>

                                // Venue Rating Card
                                <div class="afb-card">
                                    <div class="afb-kicker afb-violet">
                                        "Venue (Onsite)"
                                    </div>
                                    <div class="afb-stat-value">
                                        {venue_dim.map(|d| format!("{:.2}", d.average_score)).unwrap_or_else(|| "-".to_string())}
                                        <span class="afb-stat-unit">"/ 3.00"</span>
                                    </div>
                                    <div class="afb-stat-note-good">
                                        {venue_dim.map(|d| format!("{:.0}% satisfied ({} ratings)", d.positive_percentage, d.total_answers)).unwrap_or_default()}
                                    </div>
                                </div>

                                // Catering Rating Card
                                <div class="afb-card">
                                    <div class="afb-kicker afb-pink">
                                        "Catering (Onsite)"
                                    </div>
                                    <div class="afb-stat-value">
                                        {catering_dim.map(|d| format!("{:.2}", d.average_score)).unwrap_or_else(|| "-".to_string())}
                                        <span class="afb-stat-unit">"/ 3.00"</span>
                                    </div>
                                    <div class="afb-stat-note-good">
                                        {catering_dim.map(|d| format!("{:.0}% satisfied ({} ratings)", d.positive_percentage, d.total_answers)).unwrap_or_default()}
                                    </div>
                                </div>

                                // Promotion Rating Card
                                <div class="afb-card">
                                    <div class="afb-kicker afb-amber">
                                        "Promotion"
                                    </div>
                                    <div class="afb-stat-value">
                                        {promo_dim.map(|d| format!("{:.2}", d.average_score)).unwrap_or_else(|| "-".to_string())}
                                        <span class="afb-stat-unit">"/ 3.00"</span>
                                    </div>
                                    <div class="afb-stat-note-good">
                                        {promo_dim.map(|d| format!("{:.0}% satisfied ({} ratings)", d.positive_percentage, d.total_answers)).unwrap_or_default()}
                                    </div>
                                </div>
                            </div>

                            // 4-Dimension Breakdown Section
                            <div class="afb-panel-lg">
                                <h4 class="afb-h3-spaced">"Satisfaction Dimensions Breakdown"</h4>
                                <div class="afb-grid-md">
                                    {data.dimensions.iter().map(|dim| {
                                        let high = dim.distribution.iter().find(|d| d.label == "พึงพอใจมาก").map(|d| (d.count, d.percentage)).unwrap_or((0, 0.0));
                                        let med = dim.distribution.iter().find(|d| d.label == "พึงพอใจ").map(|d| (d.count, d.percentage)).unwrap_or((0, 0.0));
                                        let low = dim.distribution.iter().find(|d| d.label == "ไม่พึงพอใจ").map(|d| (d.count, d.percentage)).unwrap_or((0, 0.0));

                                        view! {
                                            <div class="afb-inset">
                                                <div class="afb-head-sm">
                                                    <span class="afb-strong-sm">{dim.title.clone()}</span>
                                                    <span class="afb-caption">
                                                        {format!("Score: {:.2} ({} total)", dim.average_score, dim.total_answers)}
                                                    </span>
                                                </div>

                                                // Multi-colored bar
                                                <div class="afb-stack-bar">
                                                    <div class="afb-seg-high" style=format!("width: {}%;", high.1) title=format!("พึงพอใจมาก: {} ({:.1}%)", high.0, high.1)></div>
                                                    <div class="afb-seg-mid" style=format!("width: {}%;", med.1) title=format!("พึงพอใจ: {} ({:.1}%)", med.0, med.1)></div>
                                                    <div class="afb-seg-low" style=format!("width: {}%;", low.1) title=format!("ไม่พึงพอใจ: {} ({:.1}%)", low.0, low.1)></div>
                                                </div>

                                                // Distribution labels
                                                <div class="afb-legend">
                                                    <span class="afb-row-xs">
                                                        <span class="afb-dot afb-dot-high"></span>
                                                        {format!("มาก: {} ({:.0}%)", high.0, high.1)}
                                                    </span>
                                                    <span class="afb-row-xs">
                                                        <span class="afb-dot afb-dot-mid"></span>
                                                        {format!("พอใจ: {} ({:.0}%)", med.0, med.1)}
                                                    </span>
                                                    <span class="afb-row-xs">
                                                        <span class="afb-dot afb-dot-low"></span>
                                                        {format!("ไม่พอใจ: {} ({:.0}%)", low.0, low.1)}
                                                    </span>
                                                </div>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            </div>

                            // Secondary Analytics (Online Watch Habits & Continuation)
                            <div class="afb-grid-lg">
                                // Online Watch Habit Card
                                <Show when=move || has_online_watched fallback=|| view! { <div></div> }>
                                    <div class="afb-panel">
                                        <h4 class="afb-panel-title">
                                            <span>"📺"</span> "Online Attendance & Viewing Habit"
                                        </h4>
                                        <div class="afb-stack">
                                            {data.online_watched.iter().map(|item| {
                                                view! {
                                                    <div>
                                                        <div class="afb-bar-label">
                                                            <span class="afb-text-light">{item.option.clone()}</span>
                                                            <span class="afb-strong-sky">{format!("{} ({:.1}%)", item.count, item.percentage)}</span>
                                                        </div>
                                                        <div class="afb-bar-track">
                                                            <div class="afb-bar-fill afb-sky-bg" style=format!("width: {}%;", item.percentage)></div>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </div>
                                </Show>

                                // Series Continuation Card
                                <Show when=move || has_latent_space_continue fallback=|| view! { <div></div> }>
                                    <div class="afb-panel">
                                        <h4 class="afb-panel-title">
                                            <span>"🚀"</span> "Series Continuation Interest"
                                        </h4>
                                        <div class="afb-stack">
                                            {data.latent_space_continue.iter().map(|item| {
                                                view! {
                                                    <div>
                                                        <div class="afb-bar-label">
                                                            <span class="afb-text-light">{item.option.clone()}</span>
                                                            <span class="afb-strong-violet">{format!("{} ({:.1}%)", item.count, item.percentage)}</span>
                                                        </div>
                                                        <div class="afb-bar-track">
                                                            <div class="afb-bar-fill afb-violet-bg" style=format!("width: {}%;", item.percentage)></div>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </div>
                                </Show>
                            </div>

                            // Attendee Submissions & Qualitative Feedback Table
                            <div class="afb-panel">
                                // Search & filter bar
                                <div class="afb-toolbar">
                                    <h4 class="afb-h3">
                                        {format!("Responses & Feedback ({})", filtered_rows.len())}
                                    </h4>

                                    <div class="afb-wrap">
                                        <div class="afb-segmented-sm">
                                            <button
                                                class=move || match filter_type.get() == "all" {
                                                    true => "btn btn-sm afb-filter-btn is-active",
                                                    false => "btn btn-sm afb-filter-btn",
                                                }
                                                on:click=move |_| set_filter_type.set("all".to_string())
                                            >
                                                "All"
                                            </button>
                                            <button
                                                class=move || match filter_type.get() == "onsite" {
                                                    true => "btn btn-sm afb-filter-btn is-active",
                                                    false => "btn btn-sm afb-filter-btn",
                                                }
                                                on:click=move |_| set_filter_type.set("onsite".to_string())
                                            >
                                                "Onsite"
                                            </button>
                                            <button
                                                class=move || match filter_type.get() == "online" {
                                                    true => "btn btn-sm afb-filter-btn is-active",
                                                    false => "btn btn-sm afb-filter-btn",
                                                }
                                                on:click=move |_| set_filter_type.set("online".to_string())
                                            >
                                                "Online"
                                            </button>
                                        </div>

                                        <input
                                            type="text"
                                            placeholder="Search by name, email, session, or comments..."
                                            prop:value=move || search_query.get()
                                            on:input=move |ev| set_search_query.set(event_target_value(&ev))
                                            class="form-input afb-search"
                                        />
                                    </div>
                                </div>

                                // Table
                                <div class="afb-scroll-x">
                                    <table class="table afb-table">
                                        <thead>
                                            <tr class="afb-thead-row">
                                                <th class="afb-cell">"Attendee"</th>
                                                <th class="afb-cell">"Mode"</th>
                                                <th class="afb-cell">"Content"</th>
                                                <th class="afb-cell">"Venue"</th>
                                                <th class="afb-cell">"Catering"</th>
                                                <th class="afb-cell">"Promo"</th>
                                                <th class="afb-cell">"Comments / Topics"</th>
                                                <th class="afb-cell">"Answered"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {filtered_rows.into_iter().map(|r| {
                                                let is_onsite = r.participation_type.to_lowercase().contains("in_person") || r.participation_type.to_lowercase().contains("in-person") || r.participation_type.to_lowercase().contains("onsite");
                                                let badge_cls = if is_onsite { "badge badge-sm badge-success" } else { "badge badge-sm badge-neutral" };
                                                let mode_label = if is_onsite { "Onsite" } else { "Online" };
                                                let ev_name = r.event_name.clone();

                                                view! {
                                                    <tr class="afb-tbody-row">
                                                        <td class="afb-cell">
                                                            <Show when=move || is_multi_event && ev_name.is_some() fallback=|| view! { <div></div> }>
                                                                <div class="afb-mb-xs">
                                                                    <span class="badge badge-sm afb-tag">
                                                                        {r.event_name.clone().unwrap_or_default()}
                                                                    </span>
                                                                </div>
                                                            </Show>
                                                            <div class="afb-strong">{r.name}</div>
                                                            <div class="afb-caption">{r.email}</div>
                                                        </td>
                                                        <td class="afb-cell-nowrap">
                                                            <span class=badge_cls>{mode_label}</span>
                                                            {r.is_checked_in.then(|| view! { <span class="badge badge-sm badge-neutral afb-ml-xs">"Checked In"</span> })}
                                                        </td>
                                                        <td class="afb-cell">
                                                            {r.satisfaction_content.clone().unwrap_or_else(|| "-".to_string())}
                                                        </td>
                                                        <td class="afb-cell">
                                                            {r.satisfaction_venue.clone().unwrap_or_else(|| "-".to_string())}
                                                        </td>
                                                        <td class="afb-cell">
                                                            {r.satisfaction_catering.clone().unwrap_or_else(|| "-".to_string())}
                                                        </td>
                                                        <td class="afb-cell">
                                                            {r.satisfaction_promotion.clone().unwrap_or_else(|| "-".to_string())}
                                                        </td>
                                                        <td class="afb-cell-text">
                                                            {if let Some(ref comment) = r.comment {
                                                                let text = format!("“{}”", comment);
                                                                view! {
                                                                    <div class="afb-quote">
                                                                        {text}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}
                                                            {if let Some(ref topics) = r.next_topics {
                                                                let text = topics.clone();
                                                                view! {
                                                                    <div class="afb-caption afb-amber">
                                                                        <strong class="afb-text-dim">"Topics: "</strong>{text}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}
                                                            {if let Some(ref w) = r.online_watched {
                                                                let text = w.clone();
                                                                view! {
                                                                    <div class="afb-caption-violet">
                                                                        <strong class="afb-text-dim">"Watched: "</strong>{text}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}
                                                            {if r.comment.is_none() && r.next_topics.is_none() && r.online_watched.is_none() {
                                                                view! { <span class="afb-text-faint">"-"</span> }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}
                                                        </td>
                                                        <td class="afb-cell-meta">
                                                            {r.answered_at.unwrap_or_else(|| "-".to_string())}
                                                        </td>
                                                    </tr>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </tbody>
                                    </table>
                                </div>
                            </div>
                        </div>
                    }.into_any()
                }}
            </Show>
        </div>
    }
}
