//! Public-facing pages for completed-event surfaces (Plan 008 — Phase 2).
//!
//! These pages are the public side of the post-event lifecycle:
//!
//! - [`past_events::PastEvents`] — `/past-events` feed of completed events
//!   that have a published recap. Each card links to the dedicated recap page.
//! - [`event_recap::EventRecap`] — `/events/:slug/recap` view of one
//!   published recap: hero image, event meta, rendered markdown body, and the
//!   headline attendance funnel (registered / deposited / checked in).
//!
//! Both pages are unauthenticated and consume the public API endpoints added
//! in Plan 008 Phase 2 (`GET /api/public/events/past` and
//! `GET /api/public/event/{slug}/recap`). Sensitive fields (refunded totals,
//! no-show counts, financials) are intentionally excluded by the backend —
//! the public recap celebrates attendance, not accounting.

pub mod event_recap;
pub mod past_events;

pub use event_recap::EventRecap;
pub use past_events::PastEvents;
