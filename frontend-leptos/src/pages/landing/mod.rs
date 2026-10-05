//! Landing page — public marketing page for BeThere.
//!
//! The rebuild in progress is `.plans/043` (build plan 0.5): hero, upcoming
//! events, how it works, then the sections still to port. Numbers come from
//! `GET /api/public/stats` through `stats`.

pub mod auth;
/// The card's deposit rule and order; public for `tests/landing_event_card.rs`.
pub mod event_card;
mod hero;
/// The swimlane; public for `tests/landing_how_routes.rs`.
pub mod how;
/// The site header. Public because `/discover` uses it too — it was inline in
/// `page.rs`, which is why that page had no chrome at all (`.issues/100`).
pub mod nav;
mod notifications;
mod page;
mod registrations;
mod sofar;
/// The shared stats fetch and duration label; public for tests.
pub mod stats;
pub mod theme;
mod upcoming;
mod waitlist;

pub use auth::AuthState;
pub use nav::SiteHeader;
pub use page::Landing;
