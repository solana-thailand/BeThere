//! Landing page — public marketing page for BeThere.
//!
//! Showcases the platform with hero, problem/solution, how-it-works steps,
//! organizer and attendee pitches, and footer branding.
//! No backend calls — purely static marketing content with SPA navigation.

pub mod auth;
/// The site header. Public because `/discover` uses it too — it was inline in
/// `page.rs`, which is why that page had no chrome at all (`.issues/100`).
pub mod nav;
mod notifications;
mod page;
mod registrations;
mod upcoming;
mod waitlist;

pub use auth::AuthState;
pub use nav::SiteHeader;
pub use page::Landing;
