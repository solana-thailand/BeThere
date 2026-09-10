//! Row and dashboard-stat types decoded from D1 result sets.

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct CampaignRow {
    pub id: String,
    pub title: String,
    pub description: String,
    pub organization_id: String,
    pub status: String,
    pub completion_criteria: String, // JSON
    pub reward_type: String,
    pub reward_config: String, // JSON
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct CampaignEventRow {
    pub campaign_id: String,
    pub event_id: String,
    pub sequence_order: i64,
    pub is_required: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct DeveloperCampaignProgressRow {
    pub campaign_id: String,
    pub developer_email: String,
    pub events_completed: i64,
    pub total_required: i64,
    pub is_complete: i64,
    pub completed_at: Option<String>,
    pub reward_claimed_at: Option<String>,
}

/// One row per (developer, campaign-event) returned by the campaign
/// attendance breakdown query. `attended` is 1 when the developer has a
/// checked-in attendee record for that event, 0 otherwise.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct DeveloperEventAttendanceRow {
    pub developer_email: String,
    pub event_id: String,
    pub event_name: Option<String>,
    pub sequence_order: i64,
    pub is_required: i64,
    pub attended: i64,
}

// ---------------------------------------------------------------------------
// Dashboard stats types
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct CampaignCompletionStats {
    pub total_enrolled: i64,
    pub total_completed: i64,
    pub completion_rate: f64,
    pub events: Vec<EventDropOff>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct EventDropOff {
    pub event_id: String,
    pub sequence_order: i64,
    pub attended: i64,
    pub total_in_campaign: i64,
}
