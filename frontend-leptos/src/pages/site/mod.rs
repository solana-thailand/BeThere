//! The Release 4 site (.plans/045): the pages beside the landing and the
//! doors that join them. The shared frame is `pages::landing::frame`.

/// Pages, doors and the try band; public for `tests/site_pages.rs`.
pub mod doors;
mod events;
/// The page head in the still room; public for `tests/site_heads.rs`.
pub mod head;
mod organizers;
mod sponsors;

pub use events::{DiscoverRedirect, EventsPage};
pub use organizers::Organizers;
pub use sponsors::SponsorsPage;
