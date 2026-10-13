//! When an event is over, and whether it has a recording people can watch.
//!
//! Shared by the worker (who may register to watch) and the frontend (what
//! the event page shows), so the page and the API cannot disagree. Before
//! this, "over" was the organizer-set status alone, and RTM #6 still read
//! "Happening now!" with an open seat form nine days after it ended
//! (.issues/165).

const DAY_MS: i64 = 86_400_000;

/// The end time has passed. With the time still TBA the date is real but the
/// hour is not, so it ends a day later; no end time never ends. This is the
/// in-person seat cutoff too (`EventConfig::in_person_registration_closed`).
pub fn ended(end_ms: i64, time_tba: bool, now_ms: i64) -> bool {
    match end_ms {
        0 => false,
        end if time_tba => now_ms >= end.saturating_add(DAY_MS),
        end => now_ms >= end,
    }
}

/// Over once the organizer marks it completed, or once it has [`ended`].
pub fn is_over(completed: bool, end_ms: i64, time_tba: bool, now_ms: i64) -> bool {
    completed || ended(end_ms, time_tba, now_ms)
}

/// `url` when it is safe to put in a public link: `https://` and something
/// after it. HTTP, relative and active-content URLs give `None`.
pub fn https_public_url(url: &str) -> Option<String> {
    url.trim()
        .strip_prefix("https://")
        .filter(|rest| !rest.trim().is_empty())
        .map(|rest| format!("https://{rest}"))
}
