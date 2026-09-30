//! Campaign detail: info header plus the Events, Progress and Stats tabs.

use leptos::prelude::*;

use super::state::CampaignsState;
use super::types::{CampaignView, DetailTab, status_badge_class};
use crate::api;
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};

pub(super) fn detail_view(s: CampaignsState) -> impl IntoView {
    let CampaignsState {
        set_toast,
        current_view,
        detail_tab,
        set_detail_tab,
        selected_id,
        campaign_detail,
        set_campaign_detail,
        progress,
        stats,
        add_event_id,
        set_add_event_id,
        add_seq_order,
        set_add_seq_order,
        add_is_required,
        set_add_is_required,
        events_list,
        draft_nudge,
        set_draft_nudge,
        ..
    } = s;
    let handle_back = move |_: web_sys::MouseEvent| s.back();
    let handle_status_change = move |id: String, status: String| s.change_status(id, status);

    let handle_add_event = {
        move |_: web_sys::MouseEvent| {
            let id = selected_id.get().unwrap_or_default();
            let new_event_id = add_event_id.get();
            if new_event_id.trim().is_empty() {
                components::show_toast(&set_toast, "Select an event to add", ToastType::Warning);
                return;
            }

            let detail = campaign_detail.get();
            let mut events: Vec<api::CampaignEventInput> = detail
                .as_ref()
                .map(|d| {
                    d.events
                        .iter()
                        .map(|e| api::CampaignEventInput {
                            event_id: e.event_id.clone(),
                            sequence_order: e.sequence_order,
                            is_required: e.is_required,
                        })
                        .collect()
                })
                .unwrap_or_default();

            events.push(api::CampaignEventInput {
                event_id: new_event_id,
                sequence_order: add_seq_order.get(),
                is_required: add_is_required.get(),
            });

            let set_toast = set_toast;
            leptos::task::spawn_local(async move {
                match api::set_campaign_events(&id, events).await {
                    Ok(()) => {
                        components::show_toast(&set_toast, "Event added", ToastType::Success);
                        set_add_event_id.set(String::new());
                        set_add_seq_order.set(0);
                        set_add_is_required.set(true);
                        // Reload detail
                        if let Ok(d) = api::get_campaign(&id).await {
                            set_campaign_detail.set(Some(d))
                        }
                    }
                    Err(e) => {
                        components::show_toast(
                            &set_toast,
                            &format!("Failed to add event: {e}"),
                            ToastType::Error,
                        );
                    }
                }
            });
        }
    };

    let handle_remove_event = {
        move |event_id: String| {
            let id = selected_id.get().unwrap_or_default();
            let detail = campaign_detail.get();
            let events: Vec<api::CampaignEventInput> = detail
                .as_ref()
                .map(|d| {
                    d.events
                        .iter()
                        .filter(|e| e.event_id != event_id)
                        .map(|e| api::CampaignEventInput {
                            event_id: e.event_id.clone(),
                            sequence_order: e.sequence_order,
                            is_required: e.is_required,
                        })
                        .collect()
                })
                .unwrap_or_default();

            let set_toast = set_toast;
            leptos::task::spawn_local(async move {
                match api::set_campaign_events(&id, events).await {
                    Ok(()) => {
                        components::show_toast(&set_toast, "Event removed", ToastType::Success);
                        if let Ok(d) = api::get_campaign(&id).await {
                            set_campaign_detail.set(Some(d))
                        }
                    }
                    Err(e) => {
                        components::show_toast(
                            &set_toast,
                            &format!("Failed to remove event: {e}"),
                            ToastType::Error,
                        );
                    }
                }
            });
        }
    };

    view! {
        // === DETAIL VIEW ===
        <Show when=move || current_view.get() == CampaignView::Detail fallback=|| view! { <div></div> }>
            <div class="events-header-row">
                <h2 class="admin-section-heading">
                    {move || campaign_detail.get().map(|d| d.campaign.title.clone()).unwrap_or_default()}
                </h2>
                <button class="btn btn-outline btn-sm" on:click=handle_back>
                    "← Back"
                </button>
            </div>
            // Draft-status warning banner (events linked but campaign not active)
            {move || {
                let detail = campaign_detail.get();
                match detail {
                    Some(d) if !d.events.is_empty() && d.campaign.status == "draft" => {
                        let id_for_activate = d.campaign.id.clone();
                        view! {
                            <div class="campaign-status-banner campaign-status-banner-warning">
                                <div class="campaign-status-banner-text">
                                    <strong>"Draft — not active"</strong>
                                    "This campaign has events linked but isn't activated. Activate it so check-ins count toward progress and leaderboard scoring."
                                </div>
                                <button
                                    class="btn btn-primary btn-sm"
                                    on:click=move |_: web_sys::MouseEvent| {
                                        handle_status_change(id_for_activate.clone(), "active".to_string());
                                    }
                                >
                                    "Activate now"
                                </button>
                            </div>
                        }.into_any()
                    }
                    _ => view! { <div></div> }.into_any(),
                }
            }}
            // Campaign info header
            <div class="card">
                <div class="card-body">
                    {move || {
                        let detail = campaign_detail.get();
                        detail.map(|d| {
                            let c = &d.campaign;
                            view! {
                                <div class="campaign-detail-info">
                                    <div class="campaign-detail-row">
                                        <strong>"ID:"</strong> <span>{c.id.clone()}</span>
                                    </div>
                                    <div class="campaign-detail-row">
                                        <strong>"Status:"</strong>
                                        <span class=status_badge_class(&c.status)>
                                            {c.status.clone()}
                                        </span>
                                    </div>
                                    <div class="campaign-detail-row">
                                        <strong>"Description:"</strong>
                                        <span>{c.description.clone()}</span>
                                    </div>
                                    <div class="campaign-detail-row">
                                        <strong>"Reward Type:"</strong>
                                        <span>{c.reward_type.clone()}</span>
                                    </div>
                                    <div class="campaign-detail-row">
                                        <strong>"Organization:"</strong>
                                        <span>{c.organization_id.clone()}</span>
                                    </div>
                                </div>
                            }
                        })
                    }}
                </div>
            </div>
            // Tabs
            <div class="tabs">
                <button
                    class=move || if detail_tab.get() == DetailTab::Events { "tab active" } else { "tab" }
                    on:click=move |_| set_detail_tab.set(DetailTab::Events)
                >
                    <Icon icon=IconName::Calendar class="icon-sm" />
                    " Events"
                </button>
                <button
                    class=move || if detail_tab.get() == DetailTab::Progress { "tab active" } else { "tab" }
                    on:click=move |_| set_detail_tab.set(DetailTab::Progress)
                >
                    <Icon icon=IconName::Target class="icon-sm" />
                    " Progress"
                </button>
                <button
                    class=move || if detail_tab.get() == DetailTab::Stats { "tab active" } else { "tab" }
                    on:click=move |_| set_detail_tab.set(DetailTab::Stats)
                >
                    <Icon icon=IconName::Chart class="icon-sm" />
                    " Stats"
                </button>
            </div>
            // --- Events Tab ---
            <Show when=move || detail_tab.get() == DetailTab::Events fallback=|| view! { <div></div> }>
                // One-shot "add events to activate" nudge shown right after
                // a fresh (non-promote) create. Dismissable and auto-cleared
                // on any navigation away from this detail view.
                <Show
                    when=move || draft_nudge.get().is_some()
                    fallback=|| view! { <div></div> }
                >
                    <div class="campaign-nudge">
                        {move || {
                            // Describe the status actually chosen on create.
                            // An Active campaign announced as a draft would
                            // send the organizer looking for an Activate
                            // button that no longer applies.
                            let created = draft_nudge.get().unwrap_or_default();
                            match created.as_str() {
                                "active" => {
                                    view! {
                                        <strong>"Campaign created and active."</strong>
                                        " Add events so attendees can make progress."
                                    }
                                }
                                _ => {
                                    view! {
                                        <strong>"Campaign created as draft."</strong>
                                        " Add events to activate it."
                                    }
                                }
                            }
                        }}
                        <button
                            class="btn btn-sm btn-outline"
                            style="margin-left: 0.75rem; padding: 0.15rem 0.5rem; font-size: 0.75rem;"
                            on:click=move |_: web_sys::MouseEvent| set_draft_nudge.set(None)
                        >
                            "Dismiss"
                        </button>
                    </div>
                </Show>
                <div class="card">
                    <div class="card-header">
                        <h3>"Campaign Events"</h3>
                    </div>
                    <div class="card-body">
                        // Add event form
                        <div class="form-row">
                            <div class="form-group form-group-sm">
                                <select
                                    class="form-select"
                                    prop:value=move || add_event_id.get()
                                    on:change=move |ev| set_add_event_id.set(event_target_value(&ev))
                                >
                                    <option value="">"Select an event..."</option>
                                    {move || {
                                        // Exclude events already linked to this campaign so
                                        // they can't be added twice.
                                        let linked: std::collections::HashSet<String> = campaign_detail
                                            .get()
                                            .map(|d| d.events.iter().map(|e| e.event_id.clone()).collect())
                                            .unwrap_or_default();
                                        let mut available: Vec<api::EventMeta> = events_list
                                            .get()
                                            .into_iter()
                                            .filter(|e| !linked.contains(&e.id))
                                            .collect();
                                        available.sort_by(|a, b| a.name.cmp(&b.name));
                                        available.into_iter().map(|e| {
                                            let id = e.id.clone();
                                            let label = if e.name.trim().is_empty() {
                                                e.id.clone()
                                            } else {
                                                e.name.clone()
                                            };
                                            view! {
                                                <option value=id>{label}</option>
                                            }
                                        }).collect::<Vec<_>>()
                                    }}
                                </select>
                            </div>
                            <div class="form-group form-group-sm">
                                <input
                                    class="form-input"
                                    type="number"
                                    placeholder="Order"
                                    prop:value=move || add_seq_order.get().to_string()
                                    on:input=move |ev| {
                                        let v = event_target_value(&ev);
                                        set_add_seq_order.set(v.parse().unwrap_or(0));
                                    }
                                />
                            </div>
                            <div class="form-group form-group-sm">
                                <label class="form-label-inline">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || add_is_required.get()
                                        on:change=move |ev| {
                                            let checked = event_target_checked(&ev);
                                            set_add_is_required.set(checked);
                                        }
                                    />
                                    " Required"
                                </label>
                            </div>
                            <button class="btn btn-primary btn-sm" on:click=handle_add_event>
                                "Add Event"
                            </button>
                        </div>
                        // Events table
                        <table class="table">
                            <thead>
                                <tr class="table-header">
                                    <th>"#"</th>
                                    <th>"Event ID"</th>
                                    <th>"Required"</th>
                                    <th>"Actions"</th>
                                </tr>
                            </thead>
                            <tbody>
                                <For
                                    each=move || {
                                        campaign_detail
                                            .get()
                                            .map(|d| d.events)
                                            .unwrap_or_default()
                                    }
                                    key=|e| (e.event_id.clone(), e.sequence_order)
                                    children=move |e: api::CampaignEventItem| {
                                        let eid = e.event_id.clone();
                                        view! {
                                            <tr class="table-row">
                                                <td>{e.sequence_order}</td>
                                                <td>{e.event_id.clone()}</td>
                                                <td>
                                                    {if e.is_required {
                                                        view! {
                                                            <span class="badge badge-success">
                                                                "Required"
                                                            </span>
                                                        }
                                                    } else {
                                                        view! {
                                                            <span class="badge badge-warning">
                                                                "Optional"
                                                            </span>
                                                        }
                                                    }}
                                                </td>
                                                <td>
                                                    <button
                                                        class="btn btn-danger btn-sm"
                                                        on:click=move |_: web_sys::MouseEvent| {
                                                            handle_remove_event(eid.clone());
                                                        }
                                                    >
                                                        "Remove"
                                                    </button>
                                                </td>
                                            </tr>
                                        }
                                    }
                                />
                            </tbody>
                        </table>
                        <Show
                            when=move || {
                                campaign_detail
                                    .get()
                                    .map(|d| d.events.is_empty())
                                    .unwrap_or(true)
                            }
                            fallback=|| view! { <div></div> }
                        >
                            <div class="admin-empty-state">
                                <p>"No events in this campaign yet."</p>
                            </div>
                        </Show>
                    </div>
                </div>
            </Show>
            // --- Progress Tab ---
            <Show when=move || detail_tab.get() == DetailTab::Progress fallback=|| view! { <div></div> }>
                <div class="card">
                    <div class="card-header">
                        <h3>"Developer Progress"</h3>
                    </div>
                    <div class="card-body">
                        <Show when=move || progress.get().is_empty() fallback=|| view! { <div></div> }>
                            <div class="admin-empty-state">
                                <p>"No developer progress yet."</p>
                            </div>
                        </Show>
                        <Show when=move || !progress.get().is_empty() fallback=|| view! { <div></div> }>
                            <table class="table">
                                <thead>
                                    <tr class="table-header">
                                        <th>"Email"</th>
                                        <th>"Completed"</th>
                                        <th>"Events"</th>
                                        <th>"Status"</th>
                                        <th>"Reward Claimed"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    <For
                                        each=move || progress.get()
                                        key=|p| p.developer_email.clone()
                                        children=move |p: api::DeveloperProgressItem| {
                                            let is_complete = p.is_complete;
                                            let reward_claimed = p.reward_claimed_at.is_some();
                                            view! {
                                                <tr class="table-row">
                                                    <td>{p.developer_email.clone()}</td>
                                                    <td>
                                                        {format!(
                                                            "{}/{}",
                                                            p.events_completed,
                                                            p.total_required,
                                                        )}
                                                    </td>
                                                    <td>
                                                        {if p.events.is_empty() {
                                                            view! {
                                                                <span class="hint-xs">"—"</span>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <div class="attendance-chips">
                                                                    {p.events.iter().map(|ev| {
                                                                        let cls = if ev.attended {
                                                                            "attendance-chip attendance-chip-done"
                                                                        } else {
                                                                            "attendance-chip attendance-chip-pending"
                                                                        };
                                                                        let req_cls = if ev.is_required {
                                                                            "attendance-chip-name attendance-chip-required"
                                                                        } else {
                                                                            "attendance-chip-name"
                                                                        };
                                                                        let icon = if ev.attended {
                                                                            IconName::Check
                                                                        } else {
                                                                            IconName::Circle
                                                                        };
                                                                        let icon_cls = if ev.attended {
                                                                            "icon-sm icon-success"
                                                                        } else {
                                                                            "icon-sm icon-muted"
                                                                        };
                                                                        let display_name = if ev.event_name.trim().is_empty() {
                                                                            ev.event_id.clone()
                                                                        } else {
                                                                            ev.event_name.clone()
                                                                        };
                                                                        let title_text = display_name.clone();
                                                                        view! {
                                                                            <span class=cls title=title_text.clone()>
                                                                                <Icon icon=icon class=icon_cls />
                                                                                <span class=req_cls>{display_name.clone()}</span>
                                                                            </span>
                                                                        }
                                                                    }).collect::<Vec<_>>()}
                                                                </div>
                                                            }.into_any()
                                                        }}
                                                    </td>
                                                    <td>
                                                        {if is_complete {
                                                            view! {
                                                                <span class="badge badge-success">
                                                                    "Complete"
                                                                </span>
                                                            }
                                                        } else {
                                                            view! {
                                                                <span class="badge badge-warning">
                                                                    "In Progress"
                                                                </span>
                                                            }
                                                        }}
                                                    </td>
                                                    <td>
                                                        {if reward_claimed {
                                                            view! {
                                                                <span class="badge badge-info">
                                                                    "Claimed"
                                                                </span>
                                                            }
                                                        } else {
                                                            view! { <span class="">"—"</span> }
                                                        }}
                                                    </td>
                                                </tr>
                                            }
                                        }
                                    />
                                </tbody>
                            </table>
                        </Show>
                    </div>
                </div>
            </Show>
            // --- Stats Tab ---
            <Show when=move || detail_tab.get() == DetailTab::Stats fallback=|| view! { <div></div> }>
                <div class="card">
                    <div class="card-header">
                        <h3>"Campaign Statistics"</h3>
                    </div>
                    <div class="card-body">
                        {move || {
                            stats.get().map(|s| {
                                let rate = s.completion_rate;
                                let rate_pct = format!("{:.1}%", rate * 100.0);
                                view! {
                                    <div class="stats-grid">
                                        <div class="stat-card">
                                            <div class="stat-value">{s.total_enrolled.to_string()}</div>
                                            <div class="stat-label">"Enrolled"</div>
                                        </div>
                                        <div class="stat-card">
                                            <div class="stat-value">{s.total_completed.to_string()}</div>
                                            <div class="stat-label">"Completed"</div>
                                        </div>
                                        <div class="stat-card">
                                            <div class="stat-value">{rate_pct.clone()}</div>
                                            <div class="stat-label">"Completion Rate"</div>
                                        </div>
                                    </div>
                                    <div class="progress-bar">
                                        <div
                                            class="progress-fill"
                                            style=format!("width: {}%", (rate * 100.0).min(100.0))
                                        ></div>
                                    </div>
                                    <h4>"Per-Event Drop-off"</h4>
                                    <table class="table">
                                        <thead>
                                            <tr class="table-header">
                                                <th>"#"</th>
                                                <th>"Event ID"</th>
                                                <th>"Attended"</th>
                                                <th>"Total"</th>
                                                <th>"Rate"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            <For
                                                each=move || s.events.clone()
                                                key=|e| (e.event_id.clone(), e.sequence_order)
                                                children=move |e: api::EventDropOffItem| {
                                                    let attended = e.attended;
                                                    let total = e.total_in_campaign;
                                                    let pct = if total > 0 {
                                                        (attended as f64 / total as f64) * 100.0
                                                    } else {
                                                        0.0
                                                    };
                                                    view! {
                                                        <tr class="table-row">
                                                            <td>{e.sequence_order}</td>
                                                            <td>{e.event_id.clone()}</td>
                                                            <td>{attended}</td>
                                                            <td>{total}</td>
                                                            <td>{format!("{pct:.1}%")}</td>
                                                        </tr>
                                                    }
                                                }
                                            />
                                        </tbody>
                                    </table>
                                }
                            })
                        }}
                    </div>
                </div>
            </Show>
        </Show>
    }
}
