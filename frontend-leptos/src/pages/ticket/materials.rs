//! The recording and the organizer's slides, code and downloads, as one block
//! (.plans/047), shared by both ticket views.
//!
//! Order on a ticket: what the person can still do (claim the badge), then
//! this block, then the history. On the in-person ticket the claim card leads
//! the main card, so the block follows it directly. On the online ticket the
//! "What's next?" timeline is history once the event is over, so it moves
//! below the block ([`TimelineSlot`]).

use leptos::prelude::*;

use super::video_section::VideoSection;
use crate::api::CommunityLink;

/// The recording (when there is one) and the learning resources.
pub fn materials(has_video: bool, video_url: String, links: Vec<CommunityLink>) -> AnyView {
    view! {
        {has_video.then(|| view! {
            <VideoSection video_url=video_url variant="card".to_string() />
        })}
        {super::resources::resources_section(links)}
    }
    .into_any()
}

/// Where the online ticket's timeline goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineSlot {
    /// Above the claim and the materials, while the event is ahead.
    Steps,
    /// Below the materials, once the event is over.
    History,
}

impl TimelineSlot {
    /// The slot the timeline belongs in for this event.
    pub fn for_event(event_over: bool) -> Self {
        match event_over {
            true => Self::History,
            false => Self::Steps,
        }
    }
}
