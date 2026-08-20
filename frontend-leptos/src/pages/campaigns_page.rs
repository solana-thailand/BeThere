//! Campaigns & Series admin page — list, create, edit, detail with events/progress/stats.
//!
//! Issue #049 Phase 3: Campaigns admin management.

use leptos::prelude::*;

use crate::api;
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};

// ===== Promote-from-event payload =====

/// Payload produced by `EventsPage` when an organizer chooses to promote an
/// existing event into a new campaign. Consumed by `CampaignsPage` on mount
/// to pre-fill the create form and auto-link the source event.
#[derive(Debug, Clone, Default)]
pub struct PromoteEventPayload {
    pub event_id: String,
    pub event_name: String,
}

// ===== View State =====

#[derive(Debug, Clone, Copy, PartialEq)]
enum CampaignView {
    List,
    Create,
    Edit,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DetailTab {
    Events,
    Progress,
    Stats,
}

/// Availability of the campaign id (slug) currently in the create form.
///
/// Answers are advisory except for [`SlugStatus::Taken`], which is a certain
/// save failure and so blocks submission.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SlugStatus {
    /// Never checked, or the slug changed since the last answer arrived.
    Unchecked,
    Checking,
    Available,
    Taken,
    /// Rejected by the server as malformed (not `[A-Za-z0-9_-]`, or too long).
    Malformed,
    /// The probe itself failed (offline, 5xx). Never blocks saving.
    CheckFailed,
}

/// Build the `reward_config` JSON object from the individual form fields.
///
/// Shared by the save path and the preview card so the card previews exactly
/// what will be persisted. Blank fields are written through as `""` rather than
/// omitted — that is the historical shape of this column, and
/// `domain::models::campaign` treats blank as unset when resolving defaults.
///
/// The three minted fields are keyed off the domain constants rather than
/// string literals: renaming one there is then a compile-time change here, so
/// the form and the mint resolver cannot silently drift apart. The other three
/// are stored for the organizer's reference and never reach the mint request.
#[allow(clippy::too_many_arguments)]
fn build_reward_config(
    name: &str,
    symbol: &str,
    description: &str,
    image_url: &str,
    metadata_uri: &str,
    collection_mint: &str,
) -> serde_json::Value {
    use event_checkin_domain::models::campaign as reward;

    serde_json::json!({
        reward::KEY_NAME: name,
        reward::KEY_DESCRIPTION: description,
        reward::KEY_IMAGE_URL: image_url,
        "symbol": symbol,
        "metadata_uri": metadata_uri,
        "collection_mint": collection_mint,
    })
}

/// Live "what gets minted" card for the NFT reward section (plan 016 P2.2).
///
/// Resolves through `domain::models::campaign::resolve_reward` — the very
/// function the worker's mint path calls — rather than re-deriving the defaults
/// here, so the card cannot drift from what actually mints.
///
/// Only the three fields that reach the mint request are shown. Symbol,
/// Metadata URI and Collection Mint are stored on the campaign but are not part
/// of the minted metadata, so previewing them would imply otherwise.
fn nft_preview_card(title: &str, rc: &serde_json::Value) -> AnyView {
    use event_checkin_domain::models::campaign as reward;

    let resolved = reward::resolve_reward(title, rc);
    // Both textual defaults interpolate the title. With no title typed yet they
    // read as " - Campaign Complete", which looks like a bug rather than a
    // default — prompt for the title instead.
    let needs_title = title.trim().is_empty();
    let name_is_default = reward::reward_config_field(rc, "name").is_none();
    let desc_is_default = reward::reward_config_field(rc, "description").is_none();

    let line = |value: String, is_default: bool, prompt: &'static str| match is_default
        && needs_title
    {
        true => (prompt.to_string(), true, false),
        false => (value, false, is_default),
    };
    let (name_text, name_pending, name_tagged) = line(
        resolved.name,
        name_is_default,
        "Set a title to preview the minted name",
    );
    let (desc_text, desc_pending, desc_tagged) = line(
        resolved.description,
        desc_is_default,
        "Set a title to preview the minted description",
    );

    let image_url = resolved.image_url;
    let has_image = !image_url.is_empty();
    let default_tag = |shown: bool| {
        shown.then(|| view! { <span class="nft-preview-tag">"default"</span> })
    };
    let pending_class = |pending: bool| match pending {
        true => "nft-preview-pending",
        false => "",
    };

    view! {
        <div class="nft-preview-card">
            <div class="nft-preview-media">
                // Placeholder sits underneath; a failed <img> uncovers it.
                <Icon icon=IconName::Trophy class="icon-lg" />
                {has_image
                    .then(|| {
                        view! {
                            <img
                                class="nft-preview-img"
                                src=image_url.clone()
                                alt=""
                                on:error=move |ev| {
                                    // A dead artwork URL should fall back to the
                                    // placeholder, not a broken-image glyph.
                                    use wasm_bindgen::JsCast;
                                    if let Some(el) = ev
                                        .target()
                                        .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                                    {
                                        let _ = el.style().set_property("display", "none");
                                    }
                                }
                            />
                        }
                    })}
            </div>
            <div class="nft-preview-meta">
                <p class=format!("nft-preview-name {}", pending_class(name_pending))>
                    {name_text}
                    {default_tag(name_tagged)}
                </p>
                <p class=format!("nft-preview-desc {}", pending_class(desc_pending))>
                    {desc_text}
                    {default_tag(desc_tagged)}
                </p>
            </div>
        </div>
    }
    .into_any()
}

// ===== Helper =====

fn status_badge_class(status: &str) -> &'static str {
    match status {
        "active" => "badge badge-success",
        "completed" => "badge badge-info",
        _ => "badge badge-warning",
    }
}

/// Convert a free-form title into a URL-safe kebab-case slug.
/// Lowercases; replaces runs of non-[a-z0-9] with '-'; trims leading/trailing
/// dashes; caps length at 60 chars. Returns empty string for empty/whitespace input.
fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > 60 {
        out.truncate(60);
        while out.ends_with('-') {
            out.pop();
        }
    }
    out
}

// ===== Page Component =====

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
    let (current_view, set_current_view) = signal(CampaignView::List);
    let (detail_tab, set_detail_tab) = signal(DetailTab::Events);
    let (selected_id, set_selected_id) = signal(None::<String>);
    let (editing_id, set_editing_id) = signal(None::<String>);
    let (campaigns, set_campaigns) = signal(Vec::<api::CampaignDetail>::new());
    let (campaign_detail, set_campaign_detail) =
        signal(None::<api::CampaignDetailResponse>);
    let (progress, set_progress) = signal(Vec::<api::DeveloperProgressItem>::new());
    let (stats, set_stats) = signal(None::<api::CampaignStatsResponse>);
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);
    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let (form_id, set_form_id) = signal(String::new());
    // True once the user manually edits the slug; suppresses auto-fill from Title.
    let (slug_manually_edited, set_slug_manually_edited) = signal(false);
    // Availability of the slug currently in the create form (plan 016 P2.1).
    let (slug_status, set_slug_status) = signal(SlugStatus::Unchecked);
    let (form_title, set_form_title) = signal(String::new());
    let (form_description, set_form_description) = signal(String::new());
    let (form_org_id, set_form_org_id) = signal(String::new());
    // Initial status chosen on create (plan 016 P2.3). Draft is the default,
    // matching the behaviour from before the selector existed.
    let (form_status, set_form_status) = signal(
        api::CampaignStatus::Draft.as_str().to_string(),
    );
    let (form_reward_type, set_form_reward_type) = signal(String::new());
    let (form_criteria, set_form_criteria) = signal(String::new());
    let (form_rc_name, set_form_rc_name) = signal(String::new());
    let (form_rc_symbol, set_form_rc_symbol) = signal(String::new());
    let (form_rc_description, set_form_rc_description) = signal(String::new());
    let (form_rc_image_url, set_form_rc_image_url) = signal(String::new());
    let (form_rc_metadata_uri, set_form_rc_metadata_uri) = signal(String::new());
    let (form_rc_collection_mint, set_form_rc_collection_mint) = signal(String::new());
    let (add_event_id, set_add_event_id) = signal(String::new());
    let (add_seq_order, set_add_seq_order) = signal(0i64);
    let (add_is_required, set_add_is_required) = signal(true);
    // Events available to link (loaded once on mount for the Events-tab picker).
    let (events_list, set_events_list) = signal(Vec::<api::EventMeta>::new());
    // Organizations available to pick in the create-form org dropdown.
    let (orgs_list, set_orgs_list) = signal(Vec::<api::OrgOption>::new());
    // One-shot nudge flag: set true right after a fresh (non-promote) create so
    // the Detail → Events tab can show a "add events to activate" banner.
    // Cleared on any navigation away from the just-created detail view.
    let (draft_nudge, set_draft_nudge) = signal(false);
    // Event id awaiting auto-link after a successful create (set when the
    // create form was pre-filled via "promote from event").
    let (pending_event_to_link, set_pending_event_to_link) = signal(None::<String>);
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

    let do_reload = move || {
        set_refresh_counter.update(|n| *n += 1);
    };

    let reset_form = move || {
        set_form_id.set(String::new());
        set_slug_manually_edited.set(false);
        set_slug_status.set(SlugStatus::Unchecked);
        set_form_title.set(String::new());
        set_form_description.set(String::new());
        set_form_org_id.set(String::new());
        set_form_status.set(api::CampaignStatus::Draft.as_str().to_string());
        set_form_reward_type.set(String::new());
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

    // Probe whether the typed slug is already taken.
    //
    // Fired on blur of the slug *and* title fields — the title auto-fills the
    // slug, so checking only the slug field would miss the common path where an
    // organizer types a title and submits without ever focusing the slug. Blur
    // is already low-frequency, so no debounce is needed.
    let check_slug = move || {
        // Only meaningful on create; the slug is immutable once a campaign exists.
        if editing_id.get_untracked().is_some() {
            return;
        }
        let slug = form_id.get_untracked().trim().to_string();
        if slug.is_empty() {
            set_slug_status.set(SlugStatus::Unchecked);
            return;
        }
        set_slug_status.set(SlugStatus::Checking);
        leptos::task::spawn_local(async move {
            let result = api::campaign_exists(&slug).await;
            // Discard a stale answer: the organizer may have kept typing while
            // this request was in flight.
            if form_id.get_untracked().trim() != slug {
                return;
            }
            let status = match result {
                Ok(true) => SlugStatus::Taken,
                Ok(false) => SlugStatus::Available,
                Err(e) if e.status == 400 => SlugStatus::Malformed,
                Err(e) => {
                    log::warn!("[campaigns-page] slug availability check failed: {e}");
                    SlugStatus::CheckFailed
                }
            };
            set_slug_status.set(status);
        });
    };

    // Create new
    let handle_create_new = move |_: web_sys::MouseEvent| {
        reset_form();
        set_editing_id.set(None);
        // A manual "+ Create Campaign" click should never inherit a leftover
        // "promote from event" auto-link intent.
        set_pending_event_to_link.set(None);
        // Also clear any stale draft nudge from a prior create.
        set_draft_nudge.set(false);
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
        set_draft_nudge.set(false);
        set_selected_id.set(Some(id));
        set_detail_tab.set(DetailTab::Events);
        set_current_view.set(CampaignView::Detail);
    };

    let handle_back = move |_: web_sys::MouseEvent| {
        set_current_view.set(CampaignView::List);
        set_selected_id.set(None);
        set_editing_id.set(None);
        // Forget any "promote from event" auto-link intent if the organizer
        // cancels out of the form, so a later manual create isn't auto-linked.
        set_pending_event_to_link.set(None);
        // Leaving the detail view dismisses any draft nudge.
        set_draft_nudge.set(false);
        do_reload();
    };
    // Save (create or update)
    let handle_save = move |_: web_sys::MouseEvent| {
        if saving.get() {
            return;
        }
        let edit_id = editing_id.get();
        let title = form_title.get();
        if title.trim().is_empty() {
            components::show_toast(&set_toast, "Title is required", ToastType::Warning);
            return;
        }

        if edit_id.is_none() {
            let id = form_id.get();
            if id.trim().is_empty() {
                components::show_toast(
                    &set_toast,
                    "Campaign ID (slug) is required",
                    ToastType::Warning,
                );
                return;
            }
            // Organization is chosen from a dropdown and is immutable after
            // create — require it here so an empty draft with no org cannot
            // be saved.
            let org_id = form_org_id.get();
            if org_id.trim().is_empty() {
                components::show_toast(
                    &set_toast,
                    "Organization is required",
                    ToastType::Warning,
                );
                return;
            }
            // Availability is advisory — the probe can fail offline, and an
            // unchecked slug still saves. A *known* collision, though, is a
            // certain failure, so stop before the round-trip.
            match slug_status.get() {
                SlugStatus::Taken => {
                    components::show_toast(
                        &set_toast,
                        "That Campaign ID is already taken — pick a different one",
                        ToastType::Warning,
                    );
                    return;
                }
                SlugStatus::Malformed => {
                    components::show_toast(
                        &set_toast,
                        "Campaign ID may only contain letters, numbers, '-' and '_'",
                        ToastType::Warning,
                    );
                    return;
                }
                _ => {}
            }
        }

        // Build reward_config JSON from individual fields
        let rc = build_reward_config(
            &form_rc_name.get(),
            &form_rc_symbol.get(),
            &form_rc_description.get(),
            &form_rc_image_url.get(),
            &form_rc_metadata_uri.get(),
            &form_rc_collection_mint.get(),
        );
        let reward_config = serde_json::to_string(&rc).unwrap_or_default();

        set_saving.set(true);

        match edit_id {
            Some(eid) => {
                let req = api::UpdateCampaignRequest {
                    title,
                    description: form_description.get(),
                    completion_criteria: form_criteria.get(),
                    reward_type: form_reward_type.get(),
                    reward_config,
                };
                leptos::task::spawn_local(async move {
                    match api::update_campaign(&eid, &req).await {
                        Ok(_) => {
                            components::show_toast(
                                &set_toast,
                                "Campaign updated",
                                ToastType::Success,
                            );
                            set_current_view.set(CampaignView::List);
                            set_editing_id.set(None);
                            do_reload();
                        }
                        Err(e) => {
                            components::show_toast(
                                &set_toast,
                                &format!("Failed to update: {e}"),
                                ToastType::Error,
                            );
                        }
                    }
                    set_saving.set(false);
                });
            }
            None => {
                let req = api::CreateCampaignRequest {
                    id: form_id.get(),
                    title,
                    description: form_description.get(),
                    organization_id: form_org_id.get(),
                    status: form_status.get(),
                    completion_criteria: form_criteria.get(),
                    reward_type: form_reward_type.get(),
                    reward_config,
                };
                // If this create was triggered via "promote from event",
                // capture the source event id so we can auto-link it.
                let link_event_id = pending_event_to_link.get();
                let id_for_link = req.id.clone();
                leptos::task::spawn_local(async move {
                    match api::create_campaign(&req).await {
                        Ok(_) => {
                            // Auto-link the source event as the first campaign event.
                            if let Some(eid) = link_event_id.as_ref() {
                                let events = vec![api::CampaignEventInput {
                                    event_id: eid.clone(),
                                    sequence_order: 0,
                                    is_required: true,
                                }];
                                if let Err(e) =
                                    api::set_campaign_events(&id_for_link, events).await
                                {
                                    log::warn!(
                                        "[campaigns-page] auto-link source event failed: {e}"
                                    );
                                    components::show_toast(
                                        &set_toast,
                                        &format!(
                                            "Campaign created, but failed to link event: {e}"
                                        ),
                                        ToastType::Warning,
                                    );
                                }
                            }
                            set_pending_event_to_link.set(None);
                            components::show_toast(
                                &set_toast,
                                "Campaign created",
                                ToastType::Success,
                            );
                            // If promoted from an event, open the new campaign's
                            // detail so the organizer sees the linked event.
                            if link_event_id.is_some() {
                                set_selected_id.set(Some(id_for_link));
                                set_detail_tab.set(DetailTab::Events);
                                set_current_view.set(CampaignView::Detail);
                            } else {
                                // Pure create (not promoted from an event):
                                // drop the organizer into the new campaign's
                                // Events tab and show a one-shot "add events to
                                // activate" nudge instead of returning to List.
                                set_draft_nudge.set(true);
                                set_selected_id.set(Some(id_for_link));
                                set_detail_tab.set(DetailTab::Events);
                                set_current_view.set(CampaignView::Detail);
                            }
                            do_reload();
                        }
                        Err(e) => {
                            components::show_toast(
                                &set_toast,
                                &format!("Failed to create: {e}"),
                                ToastType::Error,
                            );
                        }
                    }
                    set_saving.set(false);
                });
            }
        }
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
                        components::show_toast(
                            &set_toast,
                            "Campaign deleted",
                            ToastType::Success,
                        );
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

    let handle_status_change = {
        move |id: String, status: String| {
            let set_toast = set_toast;
            leptos::task::spawn_local(async move {
                match api::update_campaign_status(&id, &status).await {
                    Ok(_) => {
                        components::show_toast(
                            &set_toast,
                            &format!("Status changed to {status}"),
                            ToastType::Success,
                        );
                        do_reload();
                    }
                    Err(e) => {
                        components::show_toast(
                            &set_toast,
                            &format!("Failed to change status: {e}"),
                            ToastType::Error,
                        );
                    }
                }
            });
        }
    };

    let handle_add_event = {
        move |_: web_sys::MouseEvent| {
            let id = selected_id.get().unwrap_or_default();
            let new_event_id = add_event_id.get();
            if new_event_id.trim().is_empty() {
                components::show_toast(
                    &set_toast,
                    "Select an event to add",
                    ToastType::Warning,
                );
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
                        components::show_toast(
                            &set_toast,
                            "Event added",
                            ToastType::Success,
                        );
                        set_add_event_id.set(String::new());
                        set_add_seq_order.set(0);
                        set_add_is_required.set(true);
                        // Reload detail
                        if let Ok(d) = api::get_campaign(&id).await { set_campaign_detail.set(Some(d)) }
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
                        components::show_toast(
                            &set_toast,
                            "Event removed",
                            ToastType::Success,
                        );
                        if let Ok(d) = api::get_campaign(&id).await { set_campaign_detail.set(Some(d)) }
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
        <div class="admin-campaigns-page">
            // === LIST VIEW ===
            <Show when=move || current_view.get() == CampaignView::List fallback=|| view! { <div></div> }>
                <div class="events-header-row">
                    <h2 class="admin-section-heading">
                        <Icon icon=IconName::Trophy class="icon-heading" />
                        " Campaigns & Series"
                    </h2>
                    <div class="events-header-actions">
                        <button class="btn btn-primary btn-sm" on:click=handle_create_new>
                            "+ Create Campaign"
                        </button>
                    </div>
                </div>
                <Show when=move || loading.get() fallback=|| view! { <div></div> }>
                    <div class="empty-state">
                        <span class="status-dot status-dot-loading"></span>
                        " Loading campaigns..."
                    </div>
                </Show>
                <Show when=move || !loading.get() && campaigns.get().is_empty() fallback=|| view! { <div></div> }>
                    <div class="empty-state">
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
                                                    class="btn btn-secondary btn-sm"
                                                    on:click=move |_: web_sys::MouseEvent| handle_view(cid_view.clone())
                                                >
                                                    "View"
                                                </button>
                                                <button
                                                    class="btn btn-secondary btn-sm"
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
            // === CREATE / EDIT VIEW ===
            <Show
                when=move || current_view.get() == CampaignView::Create
                    || current_view.get() == CampaignView::Edit
                fallback=|| view! { <div></div> }
            >
                <div class="events-header-row">
                    <h2 class="admin-section-heading">
                        {move || {
                            if editing_id.get().is_some() {
                                "Edit Campaign"
                            } else {
                                "Create Campaign"
                            }
                        }}
                    </h2>
                    <button class="btn btn-secondary btn-sm" on:click=handle_back>
                        "← Back"
                    </button>
                </div>

                <div class="card">
                    <div class="card-body">
                        <div class="form-group">
                            <label class="form-label">"Campaign ID (slug)" <span class="required-marker">"*"</span></label>
                            <input
                                class="form-input"
                                type="text"
                                placeholder="e.g. solana-hacker-series-2025"
                                disabled=move || editing_id.get().is_some()
                                prop:value=move || form_id.get()
                                on:input=move |ev| {
                                    set_form_id.set(event_target_value(&ev));
                                    set_slug_manually_edited.set(true);
                                    // The previous verdict described a different
                                    // slug — drop it rather than show it stale.
                                    set_slug_status.set(SlugStatus::Unchecked);
                                }
                                on:blur=move |_| check_slug()
                            />
                            {move || {
                                let (text, class) = match slug_status.get() {
                                    SlugStatus::Unchecked => return ().into_any(),
                                    SlugStatus::Checking => {
                                        ("Checking availability…", "hint-note-sm")
                                    }
                                    SlugStatus::Available => {
                                        ("Available", "hint-note-sm slug-ok")
                                    }
                                    SlugStatus::Taken => (
                                        "Already taken — pick a different Campaign ID.",
                                        "hint-note-sm slug-bad",
                                    ),
                                    SlugStatus::Malformed => (
                                        "Use letters, numbers, '-' or '_' only (max 64 characters).",
                                        "hint-note-sm slug-bad",
                                    ),
                                    SlugStatus::CheckFailed => (
                                        "Could not check availability — you can still save.",
                                        "hint-note-sm",
                                    ),
                                };
                                view! { <p class=class>{text}</p> }.into_any()
                            }}
                        </div>
                        <div class="form-group">
                            <label class="form-label">"Title" <span class="required-marker">"*"</span></label>
                            <input
                                class="form-input"
                                type="text"
                                placeholder="Campaign title"
                                prop:value=move || form_title.get()
                                on:input=move |ev| {
                                    let v = event_target_value(&ev);
                                    set_form_title.set(v.clone());
                                    // Auto-fill slug from title on create, unless the user
                                    // has manually edited the slug field.
                                    if editing_id.get().is_none() && !slug_manually_edited.get() {
                                        set_form_id.set(slugify(&v));
                                        set_slug_status.set(SlugStatus::Unchecked);
                                    }
                                }
                                on:blur=move |_| check_slug()
                            />
                        </div>
                        <div class="form-group">
                            <label class="form-label">"Description"</label>
                            <textarea
                                class="form-input"
                                rows="3"
                                placeholder="Campaign description"
                                prop:value=move || form_description.get()
                                on:input=move |ev| set_form_description.set(event_target_value(&ev))
                            />
                        </div>
                        <Show when=move || editing_id.get().is_none() fallback=|| view! { <div></div> }>
                            <div class="form-group">
                                <label class="form-label">"Organization" <span class="required-marker">"*"</span></label>
                                <select
                                    class="form-select"
                                    prop:value=move || form_org_id.get()
                                    on:change=move |ev| set_form_org_id.set(event_target_value(&ev))
                                >
                                    <option value="">"— Select organization —"</option>
                                    {move || {
                                        // Mirror the Events-tab picker: sort by name,
                                        // fall back to id when the name is blank.
                                        let mut orgs = orgs_list.get();
                                        orgs.sort_by(|a, b| a.name.cmp(&b.name));
                                        orgs.into_iter().map(|o| {
                                            let id = o.id.clone();
                                            let label = if o.name.trim().is_empty() {
                                                o.id.clone()
                                            } else {
                                                o.name.clone()
                                            };
                                            view! {
                                                <option value=id>{label}</option>
                                            }
                                        }).collect::<Vec<_>>()
                                    }}
                                </select>
                                <p class="hint-note-sm">
                                    "Organization is set on create and cannot be changed after."
                                </p>
                            </div>
                            <div class="form-group">
                                <label class="form-label">"Initial status"</label>
                                <select
                                    class="form-select"
                                    prop:value=move || form_status.get()
                                    on:change=move |ev| set_form_status.set(event_target_value(&ev))
                                >
                                    <option value="draft">"Draft"</option>
                                    <option value="active">"Active"</option>
                                </select>
                                <p class="hint-note-sm">
                                    "Draft is a planning marker — check-in progress is tracked either way. You can switch status later from the campaign list."
                                </p>
                            </div>
                        </Show>
                        <div class="form-group">
                            <label class="form-label">"Reward Type"</label>
                            <select
                                class="form-select"
                                prop:value=move || form_reward_type.get()
                                on:change=move |ev| set_form_reward_type.set(event_target_value(&ev))
                            >
                                <option value="none">"None"</option>
                                <option value="nft_certificate">"NFT Certificate"</option>
                            </select>
                        </div>
                        <div class="form-group">
                            <label class="form-label">"Completion Criteria (descriptive only)"</label>
                            <p class="hint-note-sm">
                                "Descriptive only — the enforced rule is: attend all required events. Use this field for notes only."
                            </p>
                            <textarea
                                class="form-input"
                                rows="3"
                                placeholder="e.g. Complete all 3 events in the series"
                                prop:value=move || form_criteria.get()
                                on:input=move |ev| set_form_criteria.set(event_target_value(&ev))
                            />
                        </div>
                        <Show when=move || form_reward_type.get() == "nft_certificate" fallback=|| view! { <div></div> }>
                            <div class="form-section">
                                <h4 class="form-section-title">"NFT Reward Configuration"</h4>
                                <p class="hint-note-sm">"All fields below are optional — sensible defaults are applied on mint."</p>
                                <div class="nft-preview">
                                    <p class="nft-preview-label">"Preview — what gets minted"</p>
                                    {move || {
                                        nft_preview_card(
                                            &form_title.get(),
                                            &build_reward_config(
                                                &form_rc_name.get(),
                                                &form_rc_symbol.get(),
                                                &form_rc_description.get(),
                                                &form_rc_image_url.get(),
                                                &form_rc_metadata_uri.get(),
                                                &form_rc_collection_mint.get(),
                                            ),
                                        )
                                    }}
                                </div>
                                <div class="form-group">
                                    <label class="form-label">"NFT Name"</label>
                                    <input class="form-input" type="text"
                                        placeholder="e.g. Series Completion Badge"
                                        prop:value=move || form_rc_name.get()
                                        on:input=move |ev| set_form_rc_name.set(event_target_value(&ev))
                                    />
                                    <p class="hint-note-sm">"Leave blank to use '{Title} - Campaign Complete' on mint."</p>
                                </div>
                                <div class="form-group">
                                    <label class="form-label">"Symbol"</label>
                                    <input class="form-input" type="text"
                                        placeholder="e.g. BUILDER"
                                        prop:value=move || form_rc_symbol.get()
                                        on:input=move |ev| set_form_rc_symbol.set(event_target_value(&ev))
                                    />
                                    <p class="hint-note-sm">
                                        "Stored on the campaign for your own reference. Not part of the minted metadata, so it does not appear in the preview above."
                                    </p>
                                </div>
                                <div class="form-group">
                                    <label class="form-label">"Description"</label>
                                    <textarea class="form-input" rows="2"
                                        placeholder="NFT description (defaults to 'Completed the {title} campaign')"
                                        prop:value=move || form_rc_description.get()
                                        on:input=move |ev| set_form_rc_description.set(event_target_value(&ev))
                                    />
                                    <p class="hint-note-sm">"Leave blank to use 'Completed the {Title} campaign' on mint."</p>
                                </div>
                                <details class="form-advanced">
                                    <summary class="form-advanced-summary">"Advanced (optional)"</summary>
                                    <p class="hint-note-sm">
                                        "Optional fields for custom artwork, off-chain metadata, or on-chain collection grouping. Leave blank to use defaults."
                                    </p>
                                    <div class="form-group">
                                        <label class="form-label">"Image URL"</label>
                                        <input class="form-input" type="url"
                                            placeholder="https://arweave.net/... or IPFS URL"
                                            prop:value=move || form_rc_image_url.get()
                                            on:input=move |ev| set_form_rc_image_url.set(event_target_value(&ev))
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">"Metadata URI"</label>
                                        <input class="form-input" type="url"
                                            placeholder="https://arweave.net/... (off-chain metadata JSON)"
                                            prop:value=move || form_rc_metadata_uri.get()
                                            on:input=move |ev| set_form_rc_metadata_uri.set(event_target_value(&ev))
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">"Collection Mint"</label>
                                        <input class="form-input" type="text"
                                            placeholder="Solana collection mint address (optional)"
                                            prop:value=move || form_rc_collection_mint.get()
                                            on:input=move |ev| set_form_rc_collection_mint.set(event_target_value(&ev))
                                        />
                                        <p class="hint-note-sm">
                                            "Optional. Groups minted NFTs into an on-chain Solana collection and is used to tell campaign rewards apart from event NFTs. Leave blank if unsure."
                                        </p>
                                    </div>
                                </details>
                            </div>
                        </Show>
                        <div class="form-actions">
                            <button
                                class="btn btn-primary"
                                disabled=move || saving.get()
                                on:click=handle_save
                            >
                                {move || if saving.get() { "Saving..." } else { "Save Campaign" }}
                            </button>
                            <button class="btn btn-secondary" on:click=handle_back>
                                "Cancel"
                            </button>
                        </div>
                    </div>
                </div>
            </Show>
            // === DETAIL VIEW ===
            <Show when=move || current_view.get() == CampaignView::Detail fallback=|| view! { <div></div> }>
                <div class="events-header-row">
                    <h2 class="admin-section-heading">
                        {move || campaign_detail.get().map(|d| d.campaign.title.clone()).unwrap_or_default()}
                    </h2>
                    <button class="btn btn-secondary btn-sm" on:click=handle_back>
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
                <div class="tab-bar">
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
                    <Show when=move || draft_nudge.get() fallback=|| view! { <div></div> }>
                        <div class="campaign-nudge">
                            <strong>"Campaign created as draft."</strong>
                            " Add events to activate it."
                            <button
                                class="btn btn-sm btn-secondary"
                                style="margin-left: 0.75rem; padding: 0.15rem 0.5rem; font-size: 0.75rem;"
                                on:click=move |_: web_sys::MouseEvent| set_draft_nudge.set(false)
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
                                <div class="empty-state">
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
                                <div class="empty-state">
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
                                        <div class="stats-overview">
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
                                        <div class="progress-bar-wrapper">
                                            <div
                                                class="progress-bar-fill"
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

        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use event_checkin_domain::models::campaign as reward;

    // --- slugify (plan 016 P0.1) -------------------------------------------

    #[test]
    fn slugify_produces_kebab_case() {
        assert_eq!(slugify("Solana Hacker Series 2025"), "solana-hacker-series-2025");
    }

    #[test]
    fn slugify_collapses_runs_and_trims() {
        assert_eq!(slugify("  Hello --- World!!  "), "hello-world");
    }

    #[test]
    fn slugify_returns_empty_for_blank_input() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("   "), "");
        assert_eq!(slugify("!!!"), "");
    }

    #[test]
    fn slugify_caps_length_without_trailing_dash() {
        let long = "a".repeat(80);
        assert_eq!(slugify(&long).len(), 60);
        // A cut landing on a separator must not leave a dangling dash.
        let awkward = format!("{} x", "a".repeat(59));
        let out = slugify(&awkward);
        assert!(!out.ends_with('-'), "slug ended with a dash: {out}");
    }

    // --- reward_config contract (plan 016 P2.2) ----------------------------

    fn config() -> serde_json::Value {
        build_reward_config("", "BUILDER", "", "", "https://arweave.net/x", "mint111")
    }

    /// The form must write the exact keys the mint resolver reads, or both the
    /// preview and the mint would silently fall back to defaults forever.
    #[test]
    fn build_reward_config_writes_the_resolver_keys() {
        let rc = config();
        assert!(rc.get(reward::KEY_NAME).is_some());
        assert!(rc.get(reward::KEY_DESCRIPTION).is_some());
        assert!(rc.get(reward::KEY_IMAGE_URL).is_some());
    }

    /// Fields stored for the organizer's reference are persisted but must not
    /// reach the minted metadata — the preview card omits them for that reason.
    #[test]
    fn build_reward_config_keeps_unminted_fields_out_of_resolution() {
        let rc = config();
        assert_eq!(rc.get("symbol").and_then(|v| v.as_str()), Some("BUILDER"));
        assert_eq!(
            rc.get("metadata_uri").and_then(|v| v.as_str()),
            Some("https://arweave.net/x")
        );

        let resolved = reward::resolve_reward("My Campaign", &rc);
        assert_eq!(resolved.name, reward::default_reward_name("My Campaign"));
        assert_eq!(
            resolved.description,
            reward::default_reward_description("My Campaign")
        );
        assert_eq!(resolved.image_url, "");
    }

    /// Blank inputs are written as `""`, not omitted — the preview relies on the
    /// resolver treating that as unset.
    #[test]
    fn blank_form_fields_round_trip_to_defaults() {
        let rc = build_reward_config("", "", "", "", "", "");
        assert_eq!(rc.get(reward::KEY_NAME).and_then(|v| v.as_str()), Some(""));

        let resolved = reward::resolve_reward("Devcon", &rc);
        assert_eq!(resolved.name, "Devcon - Campaign Complete");
        assert_eq!(resolved.description, "Completed the Devcon campaign");
    }

    #[test]
    fn filled_form_fields_survive_resolution() {
        let rc = build_reward_config(
            "Builder Badge",
            "BUILDER",
            "You shipped.",
            "https://example.com/i.png",
            "",
            "",
        );
        let resolved = reward::resolve_reward("Devcon", &rc);
        assert_eq!(resolved.name, "Builder Badge");
        assert_eq!(resolved.description, "You shipped.");
        assert_eq!(resolved.image_url, "https://example.com/i.png");
    }
}
