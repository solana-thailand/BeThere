//! Create / edit form for a campaign.

use leptos::prelude::*;

use super::reward::{build_reward_config, nft_preview_card};
use super::state::CampaignsState;
use super::types::{CampaignView, DetailTab, SlugStatus, slugify};
use crate::api;
use crate::components::{self, ToastType};

pub(super) fn form_view(s: CampaignsState) -> impl IntoView {
    let CampaignsState {
        set_toast,
        current_view,
        set_current_view,
        set_detail_tab,
        set_selected_id,
        editing_id,
        set_editing_id,
        saving,
        set_saving,
        form_id,
        set_form_id,
        slug_manually_edited,
        set_slug_manually_edited,
        slug_status,
        set_slug_status,
        form_title,
        set_form_title,
        form_description,
        set_form_description,
        form_org_id,
        set_form_org_id,
        form_status,
        set_form_status,
        form_reward_type,
        set_form_reward_type,
        form_criteria,
        set_form_criteria,
        form_rc_name,
        set_form_rc_name,
        form_rc_symbol,
        set_form_rc_symbol,
        form_rc_description,
        set_form_rc_description,
        form_rc_image_url,
        set_form_rc_image_url,
        form_rc_metadata_uri,
        set_form_rc_metadata_uri,
        form_rc_collection_mint,
        set_form_rc_collection_mint,
        orgs_list,
        set_draft_nudge,
        pending_event_to_link,
        set_pending_event_to_link,
        ..
    } = s;
    let do_reload = move || s.reload();
    let handle_back = move |_: web_sys::MouseEvent| s.back();

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
                components::show_toast(&set_toast, "Organization is required", ToastType::Warning);
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
                // Captured before the request moves `req`, so the post-create
                // banner can describe the status that was actually requested.
                let created_status = req.status.clone();
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
                                if let Err(e) = api::set_campaign_events(&id_for_link, events).await
                                {
                                    log::warn!(
                                        "[campaigns-page] auto-link source event failed: {e}"
                                    );
                                    components::show_toast(
                                        &set_toast,
                                        &format!("Campaign created, but failed to link event: {e}"),
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
                                set_draft_nudge.set(Some(created_status.clone()));
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

    view! {
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
                <button class="btn btn-outline btn-sm" on:click=handle_back>
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
                        <button class="btn btn-outline" on:click=handle_back>
                            "Cancel"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}
