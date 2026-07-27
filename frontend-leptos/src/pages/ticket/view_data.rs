//! Shared data struct for ticket page view components.

use crate::api;
use crate::utils;

/// Pre-computed fields extracted from `AttendeeData`, shared across online/in-person views.
/// Avoids duplicating the 30+ `let` bindings in each view branch.
#[derive(Clone)]
pub struct TicketViewData {
    // QR
    pub qr_image: Option<String>,
    pub has_qr: bool,

    // Attendee
    pub name: String,
    pub ticket_name: String,
    pub participation: String,
    pub masked_email: String,
    pub api_id: String,
    pub claim_token: Option<String>,

    // Status
    pub is_checked_in: bool,
    pub is_approved: bool,
    pub claimed: bool,
    pub claimed_asset_id: Option<String>,
    pub cluster: Option<String>,

    // Check-in detail
    pub checked_in_at: Option<String>,
    pub checked_in_by: Option<String>,
    pub status_detail: String,

    // Claim
    pub claim_href: String,
    pub has_claim: bool,

    // Deposit
    pub deposit_enabled: bool,
    pub deposit_deadline_hours: Option<u32>,
    pub deposit_amount_thb: u64,
    pub deposit_amount_usdc: u64,
    pub deadline_expired: bool,
    pub in_person_available: Option<bool>,
    pub refund_link: Option<String>,
    pub deposit_info: Option<api::DepositInfo>,
    pub escrow_status: String,
    pub escrow_closed: bool,

    // Event
    pub is_online: bool,
    pub event_start_ms: i64,
    pub event_end_ms: i64,
    pub event_name: String,
    pub video_url: String,
    pub has_video: bool,
    pub event_link: String,
    pub event_location: String,
    pub event_tagline: String,
    pub nft_image_url: String,
    pub is_in_person: bool,

    // Deposit href
    pub deposit_href: String,

    // Event ID (source event for rollover)
    pub event_id: String,

    // Rollover
    pub rollover_target_event: Option<crate::api::RolloverTargetEvent>,

    // Orb link
    pub orb_link: Option<String>,

    // Quest
    pub quiz_enabled: bool,

    // Community
    pub community_links: Vec<crate::api::CommunityLink>,

    // Calendar
    pub calendar_subscribe_url: String,
}

impl TicketViewData {
    /// Build from API response data.
    pub fn from_data(data: &api::AttendeeData) -> Self {
        let status_detail = if data.is_checked_in {
            let ts = data
                .attendee
                .checked_in_at
                .as_deref()
                .map(utils::format_timestamp)
                .unwrap_or_default();
            let by = data
                .attendee
                .checked_in_by
                .as_ref()
                .map(|by| {
                    if by.is_empty() {
                        String::new()
                    } else {
                        format!(" by {}", utils::escape_html(by))
                    }
                })
                .unwrap_or_default();
            format!("{ts}{by}")
        } else {
            String::new()
        };

        let claim_href = data
            .attendee
            .claim_token
            .as_ref()
            .filter(|t| !t.is_empty())
            .map(|t| format!("/claim/{t}"))
            .unwrap_or_default();
        let has_claim = !claim_href.is_empty();

        let escrow_status = data.escrow_status.clone();
        let escrow_closed = escrow_status == "closed"
            || escrow_status == "cancelled"
            || escrow_status == "deactivated";

        let orb_link = data.claimed_asset_id.as_ref().and_then(|id| {
            let c = data.cluster.as_deref().unwrap_or("devnet");
            if id.is_empty() {
                None
            } else {
                Some(utils::orb_nft_url(id, c))
            }
        });

        let is_online = !data.is_in_person;

        Self {
            qr_image: data.qr_image.clone(),
            has_qr: data.qr_image.is_some(),
            name: data.attendee.name.clone(),
            ticket_name: data.attendee.ticket_name.clone(),
            participation: data.participation_type.clone(),
            masked_email: data.attendee.email.clone(),
            api_id: data.attendee.api_id.clone(),
            claim_token: data.attendee.claim_token.clone(),
            is_checked_in: data.is_checked_in,
            is_approved: data.is_approved,
            claimed: data.claimed,
            claimed_asset_id: data.claimed_asset_id.clone(),
            cluster: data.cluster.clone(),
            checked_in_at: data.attendee.checked_in_at.clone(),
            checked_in_by: data.attendee.checked_in_by.clone(),
            status_detail,
            claim_href,
            has_claim,
            deposit_enabled: data.deposit_enabled,
            deposit_deadline_hours: data.deposit_deadline_hours,
            deposit_amount_thb: data.deposit_amount_thb,
            deposit_amount_usdc: data.deposit_amount_usdc,
            deadline_expired: data.deadline_expired,
            in_person_available: data.in_person_available,
            refund_link: data.refund_link.clone(),
            deposit_info: data.deposit_info.clone(),
            escrow_status,
            escrow_closed,
            is_online,
            event_start_ms: data.event_start_ms,
            event_end_ms: data.event_end_ms,
            event_name: data.event_name.clone(),
            video_url: data.video_url.clone(),
            has_video: !data.video_url.is_empty(),
            event_link: data.event_link.clone(),
            event_location: data.event_location.clone(),
            event_tagline: data.event_tagline.clone(),
            nft_image_url: data.nft_image_url.clone(),
            is_in_person: data.is_in_person,
            deposit_href: String::new(), // set after construction
            event_id: data.event_id.clone(),
            rollover_target_event: data.rollover_target_event.clone(),
            orb_link,
            quiz_enabled: data.quiz_enabled,
            community_links: data.community_links.clone(),
            calendar_subscribe_url: data.calendar_subscribe_url.clone(),
        }
    }
}
