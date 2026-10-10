//! Landing page — public marketing page for BeThere.
//!
//! The rebuild in progress is `.plans/043` (build plan 0.5): hero, upcoming
//! events, how it works, then the sections still to port. Numbers come from
//! `GET /api/public/stats` through `stats`.

pub mod auth;
/// The card's deposit rule and order; public for `tests/landing_event_card.rs`.
pub mod event_card;
/// The chrome every site page shares; public for `pages::site`.
pub mod frame;
/// The goal bar and the globe loader; public for `tests/landing_goal.rs`.
pub mod goal;
pub mod hall;
/// The landing's header and side index; public for `tests/landing_header.rs`.
pub mod header;
/// The hero; `Markup` is public for the site's page heads.
pub mod hero;
/// The swimlane; public for `tests/landing_how_routes.rs`.
pub mod how;
/// The organizer card; public for `pages::site::organizers`.
pub mod join;
/// The site header. Public because `/discover` uses it too — it was inline in
/// `page.rs`, which is why that page had no chrome at all (`.issues/100`).
pub mod nav;
mod notifications;
mod page;
/// The photo reel; public for `tests/landing_photo_reel.rs`.
pub mod photos;
mod registrations;
/// The lit room's mount bridge; public for `tests/landing_room.rs`.
pub mod room;
mod sofar;
/// Sponsor placements; public for `tests/landing_sponsors.rs`.
pub mod sponsors;
/// The shared stats fetch and duration label; public for tests.
pub mod stats;
/// The commitment ladder; public for `tests/landing_story.rs`.
pub mod story;
pub mod theme;
mod waitlist;

pub use auth::AuthState;
pub use nav::SiteHeader;
pub use page::Landing;
