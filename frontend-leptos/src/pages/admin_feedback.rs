//! Admin Post-Event Feedback & Survey Analytics Dashboard (Issue #113).
//!
//! Visualizes attendee satisfaction ratings (Content, Venue, Catering, Promotion),
//! online viewing habits, continuation interest, individual qualitative feedback,
//! and provides one-click CSV export.

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

    // Load feedback data when active event or refresh counter changes
    let tracked_event_id = active_event_id;
    Effect::new(move |_| {
        let _ = refresh_counter.get();
        let eid = tracked_event_id.get();

        if eid.is_none() {
            set_feedback_data.set(None);
            return;
        }

        let eid_val = eid.unwrap();
        set_loading.set(true);

        leptos::task::spawn_local(async move {
            match api::get_admin_feedback(Some(&eid_val)).await {
                Ok(data) => {
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

    let has_event = move || active_event_id.get().is_some();

    view! {
        <div class="admin-feedback-page" style="padding-bottom: 3rem;">
            // Section Header
            <div class="admin-section-header" style="display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 1.5rem;">
                <div>
                    <h3 style="display: flex; align-items: center; gap: 0.5rem; margin: 0; font-size: 1.25rem;">
                        <Icon icon=IconName::Star />
                        "Post-Event Survey & Feedback"
                    </h3>
                    <p class="admin-section-subtitle" style="color: var(--color-text-muted, #94a3b8); margin-top: 0.25rem; font-size: 0.875rem;">
                        "Review verified attendee satisfaction ratings, online viewing engagement, and session comments."
                    </p>
                </div>

                <div style="display: flex; gap: 0.5rem; align-items: center;">
                    <button
                        class="btn btn-outline btn-sm"
                        on:click=handle_refresh
                        disabled=move || loading.get()
                        style="display: flex; align-items: center; gap: 0.35rem;"
                    >
                        <Icon icon=IconName::Refresh />
                        {move || if loading.get() { "Loading..." } else { "Refresh" }}
                    </button>

                    <button
                        class="btn btn-primary btn-sm"
                        on:click=handle_download_csv
                        disabled=move || feedback_data.get().is_none() || loading.get()
                        style="display: flex; align-items: center; gap: 0.35rem;"
                    >
                        <Icon icon=IconName::Clip />
                        "Export CSV"
                    </button>
                </div>
            </div>

            // No event selected state
            <Show when=move || !has_event() fallback=|| view! { <div></div> }>
                <div class="admin-empty-state" style="padding: 3rem 1rem; text-align: center; background: rgba(255,255,255,0.02); border: 1px dashed rgba(255,255,255,0.1); border-radius: 0.75rem;">
                    <p style="color: var(--color-text-muted, #94a3b8);">"Select an event from the top selector to inspect survey responses."</p>
                </div>
            </Show>

            // Loading state
            <Show when=move || loading.get() && feedback_data.get().is_none() fallback=|| view! { <div></div> }>
                <div class="page-loading" style="display: flex; justify-content: center; align-items: center; gap: 0.75rem; padding: 4rem 1rem;">
                    <span class="spinner spinner-sm"></span>
                    <span>"Loading post-event survey data..."</span>
                </div>
            </Show>

            // Loaded Content
            <Show when=move || feedback_data.get().is_some() fallback=|| view! { <div></div> }>
                {move || {
                    let data = feedback_data.get().unwrap_or_default();
                    let total = data.total_respondents;

                    if total == 0 {
                        return view! {
                            <div class="admin-empty-state" style="padding: 3.5rem 1rem; text-align: center; background: rgba(255,255,255,0.02); border: 1px dashed rgba(255,255,255,0.1); border-radius: 0.75rem;">
                                <div style="font-size: 2rem; margin-bottom: 0.5rem;">"📝"</div>
                                <h4 style="margin: 0 0 0.5rem 0;">"No Survey Responses Yet"</h4>
                                <p style="color: var(--color-text-muted, #94a3b8); max-width: 480px; margin: 0 auto; font-size: 0.875rem;">
                                    "No attendees have submitted post-event feedback for "<strong style="color: #fff;">{data.event_name.clone()}</strong>" yet. Responses will appear here in real-time as attendees fill out the survey."
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
                                || r.comment.as_deref().unwrap_or("").to_lowercase().contains(&q)
                                || r.next_topics.as_deref().unwrap_or("").to_lowercase().contains(&q)
                        }).collect::<Vec<_>>()
                    };

                    view! {
                        <div style="display: flex; flex-direction: column; gap: 1.5rem;">
                            // Top KPI Overview Cards
                            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); gap: 1rem;">
                                // Total Card
                                <div style="background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 0.75rem; padding: 1.25rem;">
                                    <div style="font-size: 0.75rem; font-weight: 600; text-transform: uppercase; color: var(--color-text-muted, #94a3b8); margin-bottom: 0.35rem;">
                                        "Respondents"
                                    </div>
                                    <div style="font-size: 1.75rem; font-weight: 700; color: #fff;">
                                        {total}
                                    </div>
                                    <div style="font-size: 0.75rem; color: var(--color-text-muted, #94a3b8); margin-top: 0.35rem; display: flex; gap: 0.5rem;">
                                        <span class="badge badge-sm badge-neutral">{format!("Onsite: {}", data.onsite_respondents)}</span>
                                        <span class="badge badge-sm badge-neutral">{format!("Online: {}", data.online_respondents)}</span>
                                    </div>
                                </div>

                                // Content Satisfaction Card
                                <div style="background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 0.75rem; padding: 1.25rem;">
                                    <div style="font-size: 0.75rem; font-weight: 600; text-transform: uppercase; color: #38bdf8; margin-bottom: 0.35rem;">
                                        "Content Rating"
                                    </div>
                                    <div style="font-size: 1.75rem; font-weight: 700; color: #fff;">
                                        {content_dim.map(|d| format!("{:.2}", d.average_score)).unwrap_or_else(|| "-".to_string())}
                                        <span style="font-size: 0.875rem; font-weight: 400; color: #94a3b8; margin-left: 0.25rem;">"/ 3.00"</span>
                                    </div>
                                    <div style="font-size: 0.75rem; color: #4ade80; margin-top: 0.35rem;">
                                        {content_dim.map(|d| format!("{:.0}% satisfied ({} ratings)", d.positive_percentage, d.total_answers)).unwrap_or_default()}
                                    </div>
                                </div>

                                // Venue Rating Card
                                <div style="background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 0.75rem; padding: 1.25rem;">
                                    <div style="font-size: 0.75rem; font-weight: 600; text-transform: uppercase; color: #a78bfa; margin-bottom: 0.35rem;">
                                        "Venue (Onsite)"
                                    </div>
                                    <div style="font-size: 1.75rem; font-weight: 700; color: #fff;">
                                        {venue_dim.map(|d| format!("{:.2}", d.average_score)).unwrap_or_else(|| "-".to_string())}
                                        <span style="font-size: 0.875rem; font-weight: 400; color: #94a3b8; margin-left: 0.25rem;">"/ 3.00"</span>
                                    </div>
                                    <div style="font-size: 0.75rem; color: #4ade80; margin-top: 0.35rem;">
                                        {venue_dim.map(|d| format!("{:.0}% satisfied ({} ratings)", d.positive_percentage, d.total_answers)).unwrap_or_default()}
                                    </div>
                                </div>

                                // Catering Rating Card
                                <div style="background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 0.75rem; padding: 1.25rem;">
                                    <div style="font-size: 0.75rem; font-weight: 600; text-transform: uppercase; color: #f472b6; margin-bottom: 0.35rem;">
                                        "Catering (Onsite)"
                                    </div>
                                    <div style="font-size: 1.75rem; font-weight: 700; color: #fff;">
                                        {catering_dim.map(|d| format!("{:.2}", d.average_score)).unwrap_or_else(|| "-".to_string())}
                                        <span style="font-size: 0.875rem; font-weight: 400; color: #94a3b8; margin-left: 0.25rem;">"/ 3.00"</span>
                                    </div>
                                    <div style="font-size: 0.75rem; color: #4ade80; margin-top: 0.35rem;">
                                        {catering_dim.map(|d| format!("{:.0}% satisfied ({} ratings)", d.positive_percentage, d.total_answers)).unwrap_or_default()}
                                    </div>
                                </div>

                                // Promotion Rating Card
                                <div style="background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 0.75rem; padding: 1.25rem;">
                                    <div style="font-size: 0.75rem; font-weight: 600; text-transform: uppercase; color: #fbbf24; margin-bottom: 0.35rem;">
                                        "Promotion"
                                    </div>
                                    <div style="font-size: 1.75rem; font-weight: 700; color: #fff;">
                                        {promo_dim.map(|d| format!("{:.2}", d.average_score)).unwrap_or_else(|| "-".to_string())}
                                        <span style="font-size: 0.875rem; font-weight: 400; color: #94a3b8; margin-left: 0.25rem;">"/ 3.00"</span>
                                    </div>
                                    <div style="font-size: 0.75rem; color: #4ade80; margin-top: 0.35rem;">
                                        {promo_dim.map(|d| format!("{:.0}% satisfied ({} ratings)", d.positive_percentage, d.total_answers)).unwrap_or_default()}
                                    </div>
                                </div>
                            </div>

                            // 4-Dimension Breakdown Section
                            <div style="background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.08); border-radius: 0.75rem; padding: 1.5rem;">
                                <h4 style="margin: 0 0 1rem 0; font-size: 1rem; color: #fff;">"Satisfaction Dimensions Breakdown"</h4>
                                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 1.25rem;">
                                    {data.dimensions.iter().map(|dim| {
                                        let high = dim.distribution.iter().find(|d| d.label == "พึงพอใจมาก").map(|d| (d.count, d.percentage)).unwrap_or((0, 0.0));
                                        let med = dim.distribution.iter().find(|d| d.label == "พึงพอใจ").map(|d| (d.count, d.percentage)).unwrap_or((0, 0.0));
                                        let low = dim.distribution.iter().find(|d| d.label == "ไม่พึงพอใจ").map(|d| (d.count, d.percentage)).unwrap_or((0, 0.0));

                                        view! {
                                            <div style="background: rgba(0,0,0,0.2); border: 1px solid rgba(255,255,255,0.05); border-radius: 0.5rem; padding: 1rem;">
                                                <div style="display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 0.5rem;">
                                                    <span style="font-weight: 600; font-size: 0.875rem; color: #fff;">{dim.title.clone()}</span>
                                                    <span style="font-size: 0.75rem; color: #94a3b8;">
                                                        {format!("Score: {:.2} ({} total)", dim.average_score, dim.total_answers)}
                                                    </span>
                                                </div>

                                                // Multi-colored bar
                                                <div style="height: 10px; border-radius: 999px; overflow: hidden; display: flex; background: rgba(255,255,255,0.05); margin-bottom: 0.75rem;">
                                                    <div style=format!("width: {}%; background: #22c55e;", high.1) title=format!("พึงพอใจมาก: {} ({:.1}%)", high.0, high.1)></div>
                                                    <div style=format!("width: {}%; background: #3b82f6;", med.1) title=format!("พึงพอใจ: {} ({:.1}%)", med.0, med.1)></div>
                                                    <div style=format!("width: {}%; background: #ef4444;", low.1) title=format!("ไม่พึงพอใจ: {} ({:.1}%)", low.0, low.1)></div>
                                                </div>

                                                // Distribution labels
                                                <div style="display: flex; justify-content: space-between; font-size: 0.75rem; color: #94a3b8;">
                                                    <span style="display: flex; align-items: center; gap: 0.25rem;">
                                                        <span style="display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: #22c55e;"></span>
                                                        {format!("มาก: {} ({:.0}%)", high.0, high.1)}
                                                    </span>
                                                    <span style="display: flex; align-items: center; gap: 0.25rem;">
                                                        <span style="display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: #3b82f6;"></span>
                                                        {format!("พอใจ: {} ({:.0}%)", med.0, med.1)}
                                                    </span>
                                                    <span style="display: flex; align-items: center; gap: 0.25rem;">
                                                        <span style="display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: #ef4444;"></span>
                                                        {format!("ไม่พอใจ: {} ({:.0}%)", low.0, low.1)}
                                                    </span>
                                                </div>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            </div>

                            // Secondary Analytics (Online Watch Habits & Continuation)
                            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 1.25rem;">
                                // Online Watch Habit Card
                                <Show when=move || has_online_watched fallback=|| view! { <div></div> }>
                                    <div style="background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.08); border-radius: 0.75rem; padding: 1.25rem;">
                                        <h4 style="margin: 0 0 1rem 0; font-size: 0.9375rem; color: #fff; display: flex; align-items: center; gap: 0.5rem;">
                                            <span>"📺"</span> "Online Attendance & Viewing Habit"
                                        </h4>
                                        <div style="display: flex; flex-direction: column; gap: 0.75rem;">
                                            {data.online_watched.iter().map(|item| {
                                                view! {
                                                    <div>
                                                        <div style="display: flex; justify-content: space-between; font-size: 0.8125rem; margin-bottom: 0.25rem;">
                                                            <span style="color: #e2e8f0;">{item.option.clone()}</span>
                                                            <span style="font-weight: 600; color: #38bdf8;">{format!("{} ({:.1}%)", item.count, item.percentage)}</span>
                                                        </div>
                                                        <div style="height: 6px; border-radius: 999px; background: rgba(255,255,255,0.06); overflow: hidden;">
                                                            <div style=format!("width: {}%; height: 100%; background: #38bdf8;", item.percentage)></div>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </div>
                                </Show>

                                // Series Continuation Card
                                <Show when=move || has_latent_space_continue fallback=|| view! { <div></div> }>
                                    <div style="background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.08); border-radius: 0.75rem; padding: 1.25rem;">
                                        <h4 style="margin: 0 0 1rem 0; font-size: 0.9375rem; color: #fff; display: flex; align-items: center; gap: 0.5rem;">
                                            <span>"🚀"</span> "Series Continuation Interest"
                                        </h4>
                                        <div style="display: flex; flex-direction: column; gap: 0.75rem;">
                                            {data.latent_space_continue.iter().map(|item| {
                                                view! {
                                                    <div>
                                                        <div style="display: flex; justify-content: space-between; font-size: 0.8125rem; margin-bottom: 0.25rem;">
                                                            <span style="color: #e2e8f0;">{item.option.clone()}</span>
                                                            <span style="font-weight: 600; color: #a78bfa;">{format!("{} ({:.1}%)", item.count, item.percentage)}</span>
                                                        </div>
                                                        <div style="height: 6px; border-radius: 999px; background: rgba(255,255,255,0.06); overflow: hidden;">
                                                            <div style=format!("width: {}%; height: 100%; background: #a78bfa;", item.percentage)></div>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </div>
                                </Show>
                            </div>

                            // Attendee Submissions & Qualitative Feedback Table
                            <div style="background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.08); border-radius: 0.75rem; padding: 1.25rem;">
                                // Search & filter bar
                                <div style="display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 0.75rem; margin-bottom: 1rem;">
                                    <h4 style="margin: 0; font-size: 1rem; color: #fff;">
                                        {format!("Responses & Feedback ({})", filtered_rows.len())}
                                    </h4>

                                    <div style="display: flex; gap: 0.5rem; flex-wrap: wrap;">
                                        <div style="display: flex; border-radius: 0.375rem; overflow: hidden; border: 1px solid rgba(255,255,255,0.1);">
                                            <button
                                                class="btn btn-sm"
                                                style=move || if filter_type.get() == "all" { "background: rgba(255,255,255,0.15);" } else { "background: transparent;" }
                                                on:click=move |_| set_filter_type.set("all".to_string())
                                            >
                                                "All"
                                            </button>
                                            <button
                                                class="btn btn-sm"
                                                style=move || if filter_type.get() == "onsite" { "background: rgba(255,255,255,0.15);" } else { "background: transparent;" }
                                                on:click=move |_| set_filter_type.set("onsite".to_string())
                                            >
                                                "Onsite"
                                            </button>
                                            <button
                                                class="btn btn-sm"
                                                style=move || if filter_type.get() == "online" { "background: rgba(255,255,255,0.15);" } else { "background: transparent;" }
                                                on:click=move |_| set_filter_type.set("online".to_string())
                                            >
                                                "Online"
                                            </button>
                                        </div>

                                        <input
                                            type="text"
                                            placeholder="Search by name, email, or comments..."
                                            prop:value=move || search_query.get()
                                            on:input=move |ev| set_search_query.set(event_target_value(&ev))
                                            class="form-control"
                                            style="padding: 0.35rem 0.75rem; font-size: 0.8125rem; border-radius: 0.375rem; width: 240px; background: rgba(0,0,0,0.3); border: 1px solid rgba(255,255,255,0.15); color: #fff;"
                                        />
                                    </div>
                                </div>

                                // Table
                                <div style="overflow-x: auto;">
                                    <table class="table" style="width: 100%; border-collapse: collapse; font-size: 0.8125rem;">
                                        <thead>
                                            <tr style="border-bottom: 1px solid rgba(255,255,255,0.1); text-align: left; color: #94a3b8;">
                                                <th style="padding: 0.6rem 0.75rem;">"Attendee"</th>
                                                <th style="padding: 0.6rem 0.75rem;">"Mode"</th>
                                                <th style="padding: 0.6rem 0.75rem;">"Content"</th>
                                                <th style="padding: 0.6rem 0.75rem;">"Venue"</th>
                                                <th style="padding: 0.6rem 0.75rem;">"Catering"</th>
                                                <th style="padding: 0.6rem 0.75rem;">"Promo"</th>
                                                <th style="padding: 0.6rem 0.75rem;">"Comments / Topics"</th>
                                                <th style="padding: 0.6rem 0.75rem;">"Answered"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {filtered_rows.into_iter().map(|r| {
                                                let is_onsite = r.participation_type.to_lowercase().contains("in_person") || r.participation_type.to_lowercase().contains("in-person") || r.participation_type.to_lowercase().contains("onsite");
                                                let badge_cls = if is_onsite { "badge badge-sm badge-success" } else { "badge badge-sm badge-neutral" };
                                                let mode_label = if is_onsite { "Onsite" } else { "Online" };

                                                view! {
                                                    <tr style="border-bottom: 1px solid rgba(255,255,255,0.05); vertical-align: top;">
                                                        <td style="padding: 0.6rem 0.75rem;">
                                                            <div style="font-weight: 600; color: #fff;">{r.name}</div>
                                                            <div style="font-size: 0.75rem; color: #94a3b8;">{r.email}</div>
                                                        </td>
                                                        <td style="padding: 0.6rem 0.75rem; white-space: nowrap;">
                                                            <span class=badge_cls>{mode_label}</span>
                                                            {r.is_checked_in.then(|| view! { <span class="badge badge-sm badge-neutral" style="margin-left: 0.25rem;">"Checked In"</span> })}
                                                        </td>
                                                        <td style="padding: 0.6rem 0.75rem;">
                                                            {r.satisfaction_content.clone().unwrap_or_else(|| "-".to_string())}
                                                        </td>
                                                        <td style="padding: 0.6rem 0.75rem;">
                                                            {r.satisfaction_venue.clone().unwrap_or_else(|| "-".to_string())}
                                                        </td>
                                                        <td style="padding: 0.6rem 0.75rem;">
                                                            {r.satisfaction_catering.clone().unwrap_or_else(|| "-".to_string())}
                                                        </td>
                                                        <td style="padding: 0.6rem 0.75rem;">
                                                            {r.satisfaction_promotion.clone().unwrap_or_else(|| "-".to_string())}
                                                        </td>
                                                        <td style="padding: 0.6rem 0.75rem; max-width: 320px;">
                                                            {if let Some(ref comment) = r.comment {
                                                                let text = format!("“{}”", comment);
                                                                view! {
                                                                    <div style="background: rgba(255,255,255,0.03); border-left: 2px solid #38bdf8; padding: 0.35rem 0.5rem; margin-bottom: 0.35rem; border-radius: 0 0.25rem 0.25rem 0; font-style: italic; color: #e2e8f0;">
                                                                        {text}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}
                                                            {if let Some(ref topics) = r.next_topics {
                                                                let text = topics.clone();
                                                                view! {
                                                                    <div style="font-size: 0.75rem; color: #fbbf24;">
                                                                        <strong style="color: #94a3b8;">"Topics: "</strong>{text}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}
                                                            {if let Some(ref w) = r.online_watched {
                                                                let text = w.clone();
                                                                view! {
                                                                    <div style="font-size: 0.75rem; color: #a78bfa; margin-top: 0.2rem;">
                                                                        <strong style="color: #94a3b8;">"Watched: "</strong>{text}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}
                                                            {if r.comment.is_none() && r.next_topics.is_none() && r.online_watched.is_none() {
                                                                view! { <span style="color: #64748b;">"-"</span> }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}
                                                        </td>
                                                        <td style="padding: 0.6rem 0.75rem; white-space: nowrap; color: #94a3b8; font-size: 0.75rem;">
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
