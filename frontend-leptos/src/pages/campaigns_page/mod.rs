//! Campaigns & Series admin page — list, create, edit, detail with events/progress/stats.
//!
//! Issue #049 Phase 3: Campaigns admin management.

mod detail_view;
mod form_view;
mod page;
mod reward;
mod state;
mod types;

pub use page::CampaignsPage;
pub use types::PromoteEventPayload;
