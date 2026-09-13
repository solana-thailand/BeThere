//! Landing page — public marketing page for BeThere.
//!
//! Showcases the platform with hero, problem/solution, how-it-works steps,
//! organizer and attendee pitches, and footer branding.
//! No backend calls — purely static marketing content with SPA navigation.

mod auth;
mod notifications;
mod page;
mod registrations;
mod upcoming;
mod waitlist;

pub use page::Landing;
