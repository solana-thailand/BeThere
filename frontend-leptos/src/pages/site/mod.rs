//! The Release 4 site (.plans/045): the pages beside the landing and the
//! doors that join them. The shared frame is `pages::landing::frame`.

/// Pages, doors and the try band; public for `tests/site_pages.rs`.
pub mod doors;
mod events;
mod organizers;
mod sponsors;

pub use events::DiscoverRedirect;
pub use organizers::Organizers;
pub use sponsors::SponsorsPage;
