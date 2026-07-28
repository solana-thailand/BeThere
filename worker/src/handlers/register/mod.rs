//! Self-registration handler for public event sign-up.
//!
//! POST /api/public/register — allows attendees to register from the public event page.
//! GET /api/my-registration/:slug — returns attendee info for the authenticated user.
//!
//! Validates input, checks for duplicates, appends to Google Sheet, returns next step.

mod capacity;
mod contact;
mod my_registration;
mod post_event;
mod signup;
mod types;

// Only the route handlers need to be reachable as `register::NAME` (handlers/mod.rs
// routes them). capacity/contact/types expose only `pub(super)` helpers and request
// structs consumed via `super::<submod>::`, so they are plain modules — no re-export.
pub use my_registration::*;
pub use post_event::*;
pub use signup::*;

#[cfg(test)]
mod tests;
