//! D1 campaign query helpers (Issue 049 Phase 3).
//!
//! Campaigns group events into series with completion tracking and rewards.
//! Three tables: campaigns, campaign_events, developer_campaign_progress.

mod checkin;
mod crud;
mod events;
mod progress;
mod series;
mod stats;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use types::CampaignRow;

pub(crate) use checkin::on_event_checkin;
pub(crate) use crud::{
    campaign_collection_mints, campaign_exists, create_campaign, delete_campaign, get_campaign,
    list_campaigns, update_campaign, update_campaign_status,
};
pub(crate) use events::{
    list_campaign_events, set_campaign_events,
};
pub(crate) use progress::{
    get_developer_progress, list_campaign_attendance, list_campaign_progress,
    list_developer_campaigns, mark_reward_claimed_with_mint,
};
pub(crate) use series::{get_campaign_for_event, list_campaign_event_summaries};
pub(crate) use stats::campaign_completion_stats;

pub use series::{EventSeriesEntry, compute_series_neighbors};
