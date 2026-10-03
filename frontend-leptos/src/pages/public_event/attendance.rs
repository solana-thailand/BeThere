//! The "Attendance" row of the event page: who has registered, per track, and
//! for a capped track how full it is.
//!
//! The Worker counts every attendee row of the event, the same tally that gates
//! the cap, so `count` here is exactly the seats already taken.

use super::types::PublicEventData;
use crate::i18n::{Locale, td_string, use_i18n};
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

/// An uncapped track shows its head-count only from this many people: "3
/// registered" reads as an empty room, while a cap gives the number a scale.
pub const MIN_PUBLIC_UNCAPPED_COUNT: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    InPerson,
    Online,
}

/// One track's head-count and, when it is capped, its room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackAttendance {
    pub track: Track,
    pub count: u32,
    pub capacity: Option<u32>,
}

impl TrackAttendance {
    /// The track as the page shows it, or `None` when it has nothing to say:
    /// an uncapped track below [`MIN_PUBLIC_UNCAPPED_COUNT`].
    pub fn new(track: Track, count: u32, capacity: Option<u32>) -> Option<Self> {
        match (capacity, count) {
            (None, n) if n < MIN_PUBLIC_UNCAPPED_COUNT => None,
            _ => Some(Self {
                track,
                count,
                capacity,
            }),
        }
    }

    /// Seats still open; `None` for an uncapped track.
    pub fn left(&self) -> Option<u32> {
        self.capacity.map(|cap| cap.saturating_sub(self.count))
    }

    pub fn is_full(&self) -> bool {
        self.left() == Some(0)
    }

    /// Fill of the capped track, 0..=100. A zero cap is full.
    pub fn percent(&self) -> Option<u32> {
        self.capacity.map(|cap| match cap {
            0 => 100,
            _ => u32::try_from(u64::from(self.count).saturating_mul(100) / u64::from(cap))
                .unwrap_or(100)
                .min(100),
        })
    }
}

/// The tracks worth showing, in-person first.
pub fn attendance_tracks(data: &PublicEventData) -> Vec<TrackAttendance> {
    let in_person = data.in_person_count.unwrap_or(0);
    let online = data.online_count.unwrap_or(0);
    [
        TrackAttendance::new(Track::InPerson, in_person, data.in_person_capacity),
        TrackAttendance::new(Track::Online, online, data.online_capacity),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// A total line is only worth its row when two tracks have people.
pub fn total_when_both(tracks: &[TrackAttendance]) -> Option<u32> {
    let populated = tracks.iter().filter(|t| t.count > 0).count();
    (populated == 2).then(|| tracks.iter().map(|t| t.count).fold(0, u32::saturating_add))
}

fn track_label(locale: Locale, track: Track) -> &'static str {
    match track {
        Track::InPerson => td_string!(locale, event.attendance_in_person),
        Track::Online => td_string!(locale, event.attendance_online),
    }
}

fn headline(locale: Locale, t: TrackAttendance) -> String {
    let label = track_label(locale, t.track);
    match t.capacity {
        Some(cap) => format!("{} / {cap} · {label}", t.count),
        None => format!("{} · {label}", t.count),
    }
}

/// The row, or `None` when no track has anything to show.
pub fn attendance_row(
    data: &PublicEventData,
    event_completed: ReadSignal<bool>,
) -> Option<AnyView> {
    let tracks = attendance_tracks(data);
    if tracks.is_empty() {
        return None;
    }
    let total = total_when_both(&tracks);
    let i18n = use_i18n();
    Some(
        view! {
            <div class="pe-meta-row">
                <Icon icon=IconName::Ticket class="icon-sm icon-muted" />
                <div class="pe-meta-body">
                    <span class="pe-meta-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.meta_attendance))}</span>
                    {tracks.into_iter().map(|t| view! {
                        <div class="pe-attendance-track">
                            <span class="pe-detail-text" class:pe-meta-full=move || t.is_full() && !event_completed.get()>
                                {move || headline(i18n.get_locale(), t)}
                            </span>
                            // Room left matters only while the event is still ahead.
                            <Show when=move || t.capacity.is_some() && !event_completed.get() fallback=|| ()>
                                <div
                                    class="pe-cap-bar"
                                    class:pe-cap-bar-full=t.is_full()
                                    role="progressbar"
                                    aria-valuemin="0"
                                    aria-valuemax=t.capacity.unwrap_or(0)
                                    aria-valuenow=t.count.min(t.capacity.unwrap_or(t.count))
                                    aria-label=move || track_label(i18n.get_locale(), t.track)
                                >
                                    <span class="pe-cap-bar-fill" style=format!("width:{}%", t.percent().unwrap_or(0))></span>
                                </div>
                                <span class="pe-detail-secondary">
                                    {move || match t.left() {
                                        Some(0) => td_string!(i18n.get_locale(), event.attendance_full).to_string(),
                                        left => format!(
                                            "{} {}",
                                            left.unwrap_or(0),
                                            td_string!(i18n.get_locale(), event.attendance_spots_left)
                                        ),
                                    }}
                                </span>
                            </Show>
                        </div>
                    }).collect::<Vec<_>>()}
                    {total.map(|n| view! {
                        <span class="pe-detail-secondary">
                            {move || format!("{n} · {}", td_string!(i18n.get_locale(), event.attendance_total))}
                        </span>
                    })}
                </div>
            </div>
        }
        .into_any(),
    )
}
