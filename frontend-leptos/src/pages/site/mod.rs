//! The Release 4 site (.plans/045): the pages beside the landing and the
//! doors that join them. The shared frame is `pages::landing::frame`.

/// The course page; public for `tests/site_course.rs`.
pub mod course;
mod deposit_walk;
/// Pages, doors and the try band; public for `tests/site_pages.rs`.
pub mod doors;
mod events;
/// The page head in the still room; public for `tests/site_heads.rs`.
pub mod head;
mod learn;
/// The open events and the empty state.
pub mod open_events;
mod organizers;
/// The planning tile; public for `tests/site_plan.rs`.
pub mod plan;
mod sponsors;
/// The "email me when it opens" form.
pub mod subscribe;
mod unsubscribe;

pub use course::CoursePage;
pub use events::{DiscoverRedirect, EventsPage};
pub use organizers::Organizers;
pub use sponsors::SponsorsPage;
pub use unsubscribe::UnsubscribePage;
