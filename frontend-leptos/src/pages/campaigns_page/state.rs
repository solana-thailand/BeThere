//! Every signal the campaigns page owns, bundled so the form and detail
//! views can live in their own files.

use leptos::prelude::*;

use super::types::{CampaignView, DetailTab, SlugStatus};
use crate::api;
use crate::components::{self, ToastType};

/// All fields are signals, so the bundle is `Copy` and moves into any
/// number of closures for free.
#[derive(Clone, Copy)]
pub(super) struct CampaignsState {
    pub(super) set_toast: WriteSignal<Option<components::ToastMessage>>,
    pub(super) current_view: ReadSignal<CampaignView>,
    pub(super) set_current_view: WriteSignal<CampaignView>,
    pub(super) detail_tab: ReadSignal<DetailTab>,
    pub(super) set_detail_tab: WriteSignal<DetailTab>,
    pub(super) selected_id: ReadSignal<Option<String>>,
    pub(super) set_selected_id: WriteSignal<Option<String>>,
    pub(super) editing_id: ReadSignal<Option<String>>,
    pub(super) set_editing_id: WriteSignal<Option<String>>,
    pub(super) campaigns: ReadSignal<Vec<api::CampaignDetail>>,
    pub(super) set_campaigns: WriteSignal<Vec<api::CampaignDetail>>,
    pub(super) campaign_detail: ReadSignal<Option<api::CampaignDetailResponse>>,
    pub(super) set_campaign_detail: WriteSignal<Option<api::CampaignDetailResponse>>,
    pub(super) progress: ReadSignal<Vec<api::DeveloperProgressItem>>,
    pub(super) set_progress: WriteSignal<Vec<api::DeveloperProgressItem>>,
    pub(super) stats: ReadSignal<Option<api::CampaignStatsResponse>>,
    pub(super) set_stats: WriteSignal<Option<api::CampaignStatsResponse>>,
    pub(super) loading: ReadSignal<bool>,
    pub(super) set_loading: WriteSignal<bool>,
    pub(super) saving: ReadSignal<bool>,
    pub(super) set_saving: WriteSignal<bool>,
    pub(super) refresh_counter: ReadSignal<u32>,
    pub(super) set_refresh_counter: WriteSignal<u32>,
    pub(super) form_id: ReadSignal<String>,
    pub(super) set_form_id: WriteSignal<String>,
    pub(super) slug_manually_edited: ReadSignal<bool>,
    pub(super) set_slug_manually_edited: WriteSignal<bool>,
    pub(super) slug_status: ReadSignal<SlugStatus>,
    pub(super) set_slug_status: WriteSignal<SlugStatus>,
    pub(super) form_title: ReadSignal<String>,
    pub(super) set_form_title: WriteSignal<String>,
    pub(super) form_description: ReadSignal<String>,
    pub(super) set_form_description: WriteSignal<String>,
    pub(super) form_org_id: ReadSignal<String>,
    pub(super) set_form_org_id: WriteSignal<String>,
    pub(super) form_status: ReadSignal<String>,
    pub(super) set_form_status: WriteSignal<String>,
    pub(super) form_reward_type: ReadSignal<String>,
    pub(super) set_form_reward_type: WriteSignal<String>,
    pub(super) form_criteria: ReadSignal<String>,
    pub(super) set_form_criteria: WriteSignal<String>,
    pub(super) form_rc_name: ReadSignal<String>,
    pub(super) set_form_rc_name: WriteSignal<String>,
    pub(super) form_rc_symbol: ReadSignal<String>,
    pub(super) set_form_rc_symbol: WriteSignal<String>,
    pub(super) form_rc_description: ReadSignal<String>,
    pub(super) set_form_rc_description: WriteSignal<String>,
    pub(super) form_rc_image_url: ReadSignal<String>,
    pub(super) set_form_rc_image_url: WriteSignal<String>,
    pub(super) form_rc_metadata_uri: ReadSignal<String>,
    pub(super) set_form_rc_metadata_uri: WriteSignal<String>,
    pub(super) form_rc_collection_mint: ReadSignal<String>,
    pub(super) set_form_rc_collection_mint: WriteSignal<String>,
    pub(super) add_event_id: ReadSignal<String>,
    pub(super) set_add_event_id: WriteSignal<String>,
    pub(super) add_seq_order: ReadSignal<i64>,
    pub(super) set_add_seq_order: WriteSignal<i64>,
    pub(super) add_is_required: ReadSignal<bool>,
    pub(super) set_add_is_required: WriteSignal<bool>,
    pub(super) events_list: ReadSignal<Vec<api::EventMeta>>,
    pub(super) set_events_list: WriteSignal<Vec<api::EventMeta>>,
    pub(super) orgs_list: ReadSignal<Vec<api::OrgOption>>,
    pub(super) set_orgs_list: WriteSignal<Vec<api::OrgOption>>,
    pub(super) draft_nudge: ReadSignal<Option<String>>,
    pub(super) set_draft_nudge: WriteSignal<Option<String>>,
    pub(super) pending_event_to_link: ReadSignal<Option<String>>,
    pub(super) set_pending_event_to_link: WriteSignal<Option<String>>,
}

impl CampaignsState {
    pub(super) fn new(set_toast: WriteSignal<Option<components::ToastMessage>>) -> Self {
        let (current_view, set_current_view) = signal(CampaignView::List);
        let (detail_tab, set_detail_tab) = signal(DetailTab::Events);
        let (selected_id, set_selected_id) = signal(None::<String>);
        let (editing_id, set_editing_id) = signal(None::<String>);
        let (campaigns, set_campaigns) = signal(Vec::<api::CampaignDetail>::new());
        let (campaign_detail, set_campaign_detail) = signal(None::<api::CampaignDetailResponse>);
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
        let (form_status, set_form_status) =
            signal(api::CampaignStatus::Draft.as_str().to_string());
        // Must match the first `<option value="none">` below. An empty default
        // made the select *display* "None" while the signal stayed `""`, which the
        // worker rejects (`invalid reward_type:`) — so creating a campaign without
        // touching this dropdown failed with a 400 on the default happy path.
        // Caught in staging click-through, 2026-08-20.
        let (form_reward_type, set_form_reward_type) = signal("none".to_string());
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
        // Holds the status the campaign was created with, so the banner describes
        // what actually happened. Was a plain bool until P2.3 made the status a
        // choice — at which point creating an Active campaign still announced
        // "created as draft" (caught in staging click-through, 2026-08-20).
        let (draft_nudge, set_draft_nudge) = signal(None::<String>);
        // Event id awaiting auto-link after a successful create (set when the
        // create form was pre-filled via "promote from event").
        let (pending_event_to_link, set_pending_event_to_link) = signal(None::<String>);
        Self {
            set_toast,
            current_view,
            set_current_view,
            detail_tab,
            set_detail_tab,
            selected_id,
            set_selected_id,
            editing_id,
            set_editing_id,
            campaigns,
            set_campaigns,
            campaign_detail,
            set_campaign_detail,
            progress,
            set_progress,
            stats,
            set_stats,
            loading,
            set_loading,
            saving,
            set_saving,
            refresh_counter,
            set_refresh_counter,
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
            add_event_id,
            set_add_event_id,
            add_seq_order,
            set_add_seq_order,
            add_is_required,
            set_add_is_required,
            events_list,
            set_events_list,
            orgs_list,
            set_orgs_list,
            draft_nudge,
            set_draft_nudge,
            pending_event_to_link,
            set_pending_event_to_link,
        }
    }

    /// Bump the refresh counter so the list Effect refetches.
    pub(super) fn reload(self) {
        self.set_refresh_counter.update(|n| *n += 1);
    }

    /// Return to the list from the form or the detail view.
    pub(super) fn back(self) {
        self.set_current_view.set(CampaignView::List);
        self.set_selected_id.set(None);
        self.set_editing_id.set(None);
        // Forget any "promote from event" auto-link intent if the organizer
        // cancels out of the form, so a later manual create isn't auto-linked.
        self.set_pending_event_to_link.set(None);
        // Leaving the detail view dismisses any draft nudge.
        self.set_draft_nudge.set(None);
        self.reload();
    }

    pub(super) fn change_status(self, id: String, status: String) {
        let set_toast = self.set_toast;
        leptos::task::spawn_local(async move {
            match api::update_campaign_status(&id, &status).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Status changed to {status}"),
                        ToastType::Success,
                    );
                    self.reload();
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
}
