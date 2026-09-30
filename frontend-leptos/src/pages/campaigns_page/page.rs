//! The campaigns page shell: data loading, list view, and view routing.

use leptos::prelude::*;

use super::detail_view::detail_view;
use super::form_view::form_view;
use super::state::CampaignsState;
use super::types::{CampaignView, DetailTab, PromoteEventPayload, SlugStatus, status_badge_class};
use crate::api;
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};

#[component]
pub fn CampaignsPage(
    #[prop(name = "set_toast")] set_toast: WriteSignal<Option<components::ToastMessage>>,
    #[prop(name = "active_event_id")] _active_event_id: ReadSignal<Option<String>>,
    #[prop(name = "pending_promote_event")] pending_promote_event: ReadSignal<
        Option<PromoteEventPayload>,
    >,
    #[prop(name = "set_pending_promote_event")] set_pending_promote_event: WriteSignal<
        Option<PromoteEventPayload>,
    >,
) -> impl IntoView {
    let s = CampaignsState::new(set_toast);
    let CampaignsState {
        current_view,
        set_current_view,
        set_detail_tab,
        selected_id,
        set_selected_id,
        set_editing_id,
        campaigns,
        set_campaigns,
        set_campaign_detail,
        set_progress,
        set_stats,
        loading,
        set_loading,
        refresh_counter,
        set_form_id,
        set_slug_manually_edited,
        set_slug_status,
        set_form_title,
        set_form_description,
        set_form_org_id,
        set_form_status,
        set_form_reward_type,
        set_form_criteria,
        set_form_rc_name,
        set_form_rc_symbol,
        set_form_rc_description,
        set_form_rc_image_url,
        set_form_rc_metadata_uri,
        set_form_rc_collection_mint,
        set_events_list,
        set_orgs_list,
        set_draft_nudge,
        set_pending_event_to_link,
        ..
    } = s;
    let do_reload = move || s.reload();
    let handle_status_change = move |id: String, status: String| s.change_status(id, status);

    // Load events list once for the Events-tab picker dropdown.
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match api::list_events().await {
                Ok(data) => set_events_list.set(data.events),
                Err(e) => {
                    log::warn!("[campaigns-page] failed to load events list: {e}");
                }
            }
        });
    });
    // Load orgs list once for the create-form org picker dropdown. Read access
    // was widened to any authenticated admin (worker handlers/orgs.rs), so this
    // succeeds for plain organizers, not just super admins.
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match api::list_orgs().await {
                Ok(data) => set_orgs_list.set(data),
                Err(e) => {
                    log::warn!("[campaigns-page] failed to load orgs list: {e}");
                }
            }
        });
    });
    // Load campaign list
    Effect::new(move |_| {
        let _ = refresh_counter.get();
        set_loading.set(true);

        leptos::task::spawn_local(async move {
            match api::list_campaigns(None, None).await {
                Ok(data) => set_campaigns.set(data),
                Err(e) => {
                    log::error!("[campaigns-page] failed to load campaigns: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to load campaigns: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_loading.set(false);
        });
    });
    // Load detail when selected
    Effect::new(move |_| {
        let id = selected_id.get();
        if id.is_none() {
            set_campaign_detail.set(None);
            set_progress.set(Vec::new());
            set_stats.set(None);
            return;
        }
        let id_val = id.unwrap();
        let id_for_detail = id_val.clone();
        let id_for_progress = id_val.clone();
        let id_for_stats = id_val;
        leptos::task::spawn_local(async move {
            match api::get_campaign(&id_for_detail).await {
                Ok(detail) => set_campaign_detail.set(Some(detail)),
                Err(e) => {
                    log::error!("[campaigns-page] failed to load campaign: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to load campaign: {e}"),
                        ToastType::Error,
                    );
                }
            }
        });
        // Load progress
        leptos::task::spawn_local(async move {
            match api::list_campaign_progress(&id_for_progress).await {
                Ok(data) => set_progress.set(data),
                Err(e) => {
                    log::warn!("[campaigns-page] failed to load progress: {e}");
                    set_progress.set(Vec::new());
                }
            }
        });
        // Load stats
        leptos::task::spawn_local(async move {
            match api::get_campaign_stats(&id_for_stats).await {
                Ok(data) => set_stats.set(Some(data)),
                Err(e) => {
                    log::warn!("[campaigns-page] failed to load stats: {e}");
                    set_stats.set(None);
                }
            }
        });
    });

    let reset_form = move || {
        set_form_id.set(String::new());
        set_slug_manually_edited.set(false);
        set_slug_status.set(SlugStatus::Unchecked);
        set_form_title.set(String::new());
        set_form_description.set(String::new());
        set_form_org_id.set(String::new());
        set_form_status.set(api::CampaignStatus::Draft.as_str().to_string());
        set_form_reward_type.set("none".to_string());
        set_form_criteria.set(String::new());
        set_form_rc_name.set(String::new());
        set_form_rc_symbol.set(String::new());
        set_form_rc_description.set(String::new());
        set_form_rc_image_url.set(String::new());
        set_form_rc_metadata_uri.set(String::new());
        set_form_rc_collection_mint.set(String::new());
    };

    let populate_form = move |c: &api::CampaignDetail| {
        set_form_title.set(c.title.clone());
        set_form_description.set(c.description.clone());
        set_form_org_id.set(c.organization_id.clone());
        set_form_reward_type.set(c.reward_type.clone());
        set_form_criteria.set(c.completion_criteria.clone());
        let rc: serde_json::Value =
            serde_json::from_str(&c.reward_config).unwrap_or(serde_json::json!({}));
        set_form_rc_name.set(
            rc.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
        set_form_rc_symbol.set(
            rc.get("symbol")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
        set_form_rc_description.set(
            rc.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
        set_form_rc_image_url.set(
            rc.get("image_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
        set_form_rc_metadata_uri.set(
            rc.get("metadata_uri")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
        set_form_rc_collection_mint.set(
            rc.get("collection_mint")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
    };

    // Consume any pending "promote event → campaign" payload (sent from
    // `EventsPage`). The admin shell sets the signal and switches section to
    // Campaigns, which mounts this component fresh — so this Effect runs once
    // on mount with the payload present.
    Effect::new(move |_| {
        if let Some(p) = pending_promote_event.get() {
            // Bind locals because Rust's inline format capture does not
            // support field access (e.g. `{p.event_id}`).
            let event_id = p.event_id.clone();
            let event_name = p.event_name.clone();
            reset_form();
            set_form_id.set(format!("{event_id}-campaign"));
            set_form_title.set(if event_name.is_empty() {
                String::new()
            } else {
                format!("{event_name} Campaign")
            });
            // Sensible default reward type; organizers can change it.
            set_form_reward_type.set("none".to_string());
            // Remember the source event so we auto-link it after create.
            set_pending_event_to_link.set(Some(event_id));
            set_editing_id.set(None);
            set_current_view.set(CampaignView::Create);
            // Clear the payload so it isn't re-consumed on a future mount.
            set_pending_promote_event.set(None);
        }
    });

    // Create new
    let handle_create_new = move |_: web_sys::MouseEvent| {
        reset_form();
        set_editing_id.set(None);
        // A manual "+ Create Campaign" click should never inherit a leftover
        // "promote from event" auto-link intent.
        set_pending_event_to_link.set(None);
        // Also clear any stale draft nudge from a prior create.
        set_draft_nudge.set(None);
        set_current_view.set(CampaignView::Create);
    };

    let handle_edit = {
        move |c: api::CampaignDetail| {
            populate_form(&c);
            set_editing_id.set(Some(c.id.clone()));
            set_current_view.set(CampaignView::Edit);
        }
    };
    // View detail
    let handle_view = move |id: String| {
        // Selecting a different campaign clears any stale draft nudge left
        // over from a prior just-created campaign.
        set_draft_nudge.set(None);
        set_selected_id.set(Some(id));
        set_detail_tab.set(DetailTab::Events);
        set_current_view.set(CampaignView::Detail);
    };

    let handle_delete = {
        move |id: String, ev: web_sys::MouseEvent| {
            ev.stop_propagation();
            let confirmed = web_sys::window()
                .map(|w| w.confirm_with_message("Delete this campaign?"))
                .unwrap_or(Ok(false));
            if confirmed != Ok(true) {
                return;
            }
            let set_toast = set_toast;
            leptos::task::spawn_local(async move {
                match api::delete_campaign(&id).await {
                    Ok(()) => {
                        components::show_toast(&set_toast, "Campaign deleted", ToastType::Success);
                        do_reload();
                    }
                    Err(e) => {
                        components::show_toast(
                            &set_toast,
                            &format!("Failed to delete: {e}"),
                            ToastType::Error,
                        );
                    }
                }
            });
        }
    };

    view! {
        <div class="admin-campaigns-page">
            // === LIST VIEW ===
            <Show when=move || current_view.get() == CampaignView::List fallback=|| view! { <div></div> }>
                <div class="events-header-row">
                    <h2 class="admin-section-heading">
                        <Icon icon=IconName::Trophy class="icon-md" />
                        " Campaigns & Series"
                    </h2>
                    <div class="events-header-actions">
                        <button class="btn btn-primary btn-sm" on:click=handle_create_new>
                            "+ Create Campaign"
                        </button>
                    </div>
                </div>
                <Show when=move || loading.get() fallback=|| view! { <div></div> }>
                    <div class="admin-empty-state">
                        <span class="status-dot status-dot-loading"></span>
                        " Loading campaigns..."
                    </div>
                </Show>
                <Show when=move || !loading.get() && campaigns.get().is_empty() fallback=|| view! { <div></div> }>
                    <div class="admin-empty-state">
                        <Icon icon=IconName::Trophy class="icon-lg" />
                        <p>"No campaigns yet. Create your first campaign."</p>
                    </div>
                </Show>
                <Show when=move || !loading.get() && !campaigns.get().is_empty() fallback=|| view! { <div></div> }>
                    <div class="campaigns-list">
                        <For
                            each=move || campaigns.get()
                            key=|c| c.id.clone()
                            children=move |c: api::CampaignDetail| {
                                let cid = c.id.clone();
                                let cid_view = cid.clone();
                                let cid_edit = cid.clone();
                                let cid_del = cid.clone();
                                let c_edit = c.clone();
                                let status = c.status.clone();
                                let status2 = status.clone();
                                let status3 = status.clone();
                                let id_for_status = cid.clone();

                                let status_class = status_badge_class(&status);
                                let status_label = status.clone();

                                view! {
                                    <div class="card">
                                        <div class="card-header">
                                            <div class="card-header-left">
                                                <span class=status_class>
                                                    {move || {
                                                        match status_label.as_str() {
                                                            "active" => "Active",
                                                            "completed" => "Completed",
                                                            _ => "Draft",
                                                        }
                                                    }}
                                                </span>
                                                <h3>{c.title.clone()}</h3>
                                            </div>
                                            <div class="card-header-actions">
                                                <button
                                                    class="btn btn-outline btn-sm"
                                                    on:click=move |_: web_sys::MouseEvent| handle_view(cid_view.clone())
                                                >
                                                    "View"
                                                </button>
                                                <button
                                                    class="btn btn-outline btn-sm"
                                                    on:click={
                                                        let c_edit = c_edit.clone();
                                                        move |_: web_sys::MouseEvent| handle_edit(c_edit.clone())
                                                    }
                                                >
                                                    "Edit"
                                                </button>
                                                <Show when=move || status2 != "active" fallback=|| view! { <div></div> }>
                                                    <button
                                                        class="btn btn-sm"
                                                        on:click={
                                                            let id_for_status = id_for_status.clone();
                                                            move |_: web_sys::MouseEvent| {
                                                                handle_status_change(id_for_status.clone(), "active".to_string());
                                                            }
                                                        }
                                                    >
                                                        "Activate"
                                                    </button>
                                                </Show>
                                                <Show when=move || status3 == "active" fallback=|| view! { <div></div> }>
                                                    <button
                                                        class="btn btn-sm"
                                                        on:click={
                                                            let cid_edit = cid_edit.clone();
                                                            move |_: web_sys::MouseEvent| {
                                                                handle_status_change(cid_edit.clone(), "completed".to_string());
                                                            }
                                                        }
                                                    >
                                                        "Complete"
                                                    </button>
                                                </Show>
                                                <button
                                                    class="btn btn-danger btn-sm"
                                                    on:click=move |ev: web_sys::MouseEvent| {
                                                        handle_delete(cid_del.clone(), ev);
                                                    }
                                                >
                                                    "Delete"
                                                </button>
                                            </div>
                                        </div>
                                        <div class="card-body">
                                            <p>{c.description.clone()}</p>
                                            <div class="card-meta">
                                                <span class="meta-item">
                                                    <Icon icon=IconName::Gift class="icon-sm" />
                                                    " " {c.reward_type.clone()}
                                                </span>
                                                <span class="meta-item">
                                                    <Icon icon=IconName::Calendar class="icon-sm" />
                                                    " " {c.created_at.clone()}
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                }
                            }
                        />
                    </div>
                </Show>
            </Show>
            {form_view(s)}
            {detail_view(s)}

        </div>
    }
}
