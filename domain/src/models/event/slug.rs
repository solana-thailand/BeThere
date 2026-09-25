//! Slug ownership — one public locator per event.
//!
//! An event is reached by its id or by its slug, and `resolve_event` tries the
//! id first. A slug is therefore taken when another event holds it as a slug
//! **or** as an id: in the second case the renamed event would be unreachable
//! on every id-first path.

/// Whether `slug` already locates an event other than `own_id`.
///
/// `events` yields `(id, slug)` pairs. Pass an `own_id` that matches no event
/// (for example `""`) when the event does not exist yet.
pub fn slug_taken_by_other<'a>(
    slug: &str,
    own_id: &str,
    events: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> bool {
    events
        .into_iter()
        .any(|(id, other_slug)| id != own_id && (id == slug || other_slug == slug))
}
