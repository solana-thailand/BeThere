//! What is on now at an event run in Ontime (plan 050).
//!
//! The event page listens to the Ontime room's attendee feed (see
//! `models::event::ontime_feed`). The room sends `public-data` packets only
//! when something changes: the agenda when the show is edited, where the show
//! is when the operator acts. Between packets the page counts by itself, from
//! the timer as it was when the packet arrived.
//!
//! Nothing here is stored. Ontime stays the one place the agenda is edited;
//! the page shows whatever the room sent last.

use std::collections::BTreeMap;

use serde::Deserialize;

/// A minute, in ms: Ontime's smallest step worth telling attendees.
const MINUTE_MS: i64 = 60_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Playback {
    Play,
    Pause,
    Armed,
    Roll,
    #[default]
    #[serde(other)]
    Stop,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Event,
    Milestone,
    Group,
    #[default]
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgendaEntry {
    #[serde(rename = "type", default, deserialize_with = "null_as_default")]
    pub kind: EntryKind,
    #[serde(default, deserialize_with = "null_as_default")]
    pub id: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub title: String,
    /// Fields the room chose to show, by label, e.g. `Presenter`.
    #[serde(default, deserialize_with = "fields_without_nulls")]
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Agenda {
    #[serde(default, deserialize_with = "null_as_default")]
    pub title: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub entries: Vec<AgendaEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timer {
    /// Time left when the packet was sent, in ms; negative once over.
    pub current: Option<i64>,
    pub duration: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Offset {
    #[serde(default, deserialize_with = "null_as_default")]
    pub mode: String,
    /// How far behind the plan, in ms. Positive is late.
    pub absolute: Option<i64>,
    pub relative: Option<i64>,
    pub expected_end: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Live {
    #[serde(default, deserialize_with = "null_as_default")]
    pub playback: Playback,
    pub now: Option<String>,
    pub next: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub timer: Timer,
    #[serde(default, deserialize_with = "null_as_default")]
    pub offset: Offset,
    pub planned_end: Option<i64>,
}

/// Ontime sends `null` where a value is absent; read it as the default.
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// A field the room left empty arrives as `null`; it is not shown, and it must
/// not throw away the rest of the packet.
fn fields_without_nulls<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<BTreeMap<String, Option<String>>>::deserialize(deserializer)?;
    Ok(raw
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(label, value)| value.map(|value| (label, value)))
        .collect())
}

#[derive(Deserialize)]
struct Packet {
    tag: String,
    #[serde(default)]
    payload: Payload,
}

#[derive(Default, Deserialize)]
struct Payload {
    agenda: Option<Agenda>,
    live: Option<Live>,
}

/// Running late, ahead, or on time, in whole minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    OnTime,
    Late { minutes: u32 },
    Ahead { minutes: u32 },
}

impl Pace {
    /// Ontime's offset (ms, positive = late) as attendees are told it: under a
    /// minute either way is on time.
    pub fn from_offset(offset_ms: i64) -> Self {
        let minutes =
            u32::try_from((offset_ms.abs() + MINUTE_MS / 2) / MINUTE_MS).unwrap_or(u32::MAX);
        match offset_ms {
            ms if ms >= MINUTE_MS => Pace::Late { minutes },
            ms if ms <= -MINUTE_MS => Pace::Ahead { minutes },
            _ => Pace::OnTime,
        }
    }
}

/// What the card shows while the show runs.
#[derive(Debug, Clone, PartialEq)]
pub struct NowCard {
    pub paused: bool,
    pub title: String,
    /// The room's chosen fields for this session, as `(label, value)`.
    pub fields: Vec<(String, String)>,
    /// Time left now, in ms; negative once over. `None` if Ontime sent none.
    pub left_ms: Option<i64>,
    /// How much of the session has passed, 0–1.
    pub progress: f64,
    pub next_title: Option<String>,
    pub pace: Pace,
    /// When the whole show ends, if Ontime knows.
    pub finish: Option<Finish>,
}

/// When the show ends, in ms since midnight (the room's clock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// From where the show is now, lateness included.
    Expected(i64),
    /// As planned, when Ontime has no estimate yet.
    Planned(i64),
}

/// The room's feed, as last heard.
#[derive(Debug, Clone, Default)]
pub struct LiveAgenda {
    agenda: Option<Agenda>,
    live: Option<Live>,
    heard_at_ms: i64,
}

impl LiveAgenda {
    /// Takes one socket message (or the `GET /public` body, which has the same
    /// payload without the envelope). Returns whether anything was taken;
    /// anything unreadable or of another kind is ignored.
    pub fn apply(&mut self, text: &str, now_ms: i64) -> bool {
        let payload = match serde_json::from_str::<Packet>(text) {
            Ok(packet) if packet.tag == "public-data" => packet.payload,
            Ok(_) => return false,
            Err(_) => match serde_json::from_str::<Payload>(text) {
                Ok(payload) => payload,
                Err(_) => return false,
            },
        };
        let mut taken = false;
        if let Some(agenda) = payload.agenda {
            self.agenda = Some(agenda);
            taken = true;
        }
        if let Some(live) = payload.live {
            self.live = Some(live);
            self.heard_at_ms = now_ms;
            taken = true;
        }
        taken
    }

    /// Forgets the show: the room closed its feed.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn title(&self) -> Option<&str> {
        self.agenda
            .as_ref()
            .map(|a| a.title.as_str())
            .filter(|t| !t.is_empty())
    }

    /// The card at `now_ms`, or `None` when nothing is running.
    pub fn card(&self, now_ms: i64) -> Option<NowCard> {
        let (agenda, live) = (self.agenda.as_ref()?, self.live.as_ref()?);
        let paused = match live.playback {
            Playback::Play => false,
            Playback::Pause => true,
            _ => return None,
        };
        let find = |id: &Option<String>| {
            id.as_deref().and_then(|id| {
                agenda
                    .entries
                    .iter()
                    .find(|e| e.kind == EntryKind::Event && e.id == id)
            })
        };
        let event = find(&live.now)?;
        let left_ms = live.timer.current.map(|current| match paused {
            true => current,
            false => current - (now_ms - self.heard_at_ms).max(0),
        });
        let progress = match (live.timer.duration, left_ms) {
            (Some(total), Some(left)) if total > 0 => {
                (1.0 - left as f64 / total as f64).clamp(0.0, 1.0)
            }
            _ => 0.0,
        };
        let offset = match live.offset.mode.as_str() {
            "relative" => live.offset.relative,
            _ => live.offset.absolute,
        };
        Some(NowCard {
            paused,
            title: event.title.clone(),
            fields: event
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            left_ms,
            progress,
            next_title: find(&live.next)
                .map(|e| e.title.clone())
                .filter(|t| !t.is_empty()),
            pace: Pace::from_offset(offset.unwrap_or(0)),
            finish: live
                .offset
                .expected_end
                .map(Finish::Expected)
                .or(live.planned_end.map(Finish::Planned)),
        })
    }
}

/// `29:59`, `1:02:03`, or `-0:12` once over.
pub fn clock_text(ms: i64) -> String {
    let sign = if ms < 0 { "-" } else { "" };
    // count down in whole seconds, the way a timer shows it: 29:59.4 is 30:00
    let total = match ms < 0 {
        true => (-ms) / 1000,
        false => (ms + 999) / 1000,
    };
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    match h {
        0 => format!("{sign}{m}:{s:02}"),
        _ => format!("{sign}{h}:{m:02}:{s:02}"),
    }
}

/// `HH:MM` for a time of day in ms since midnight (as Ontime sends it).
pub fn time_of_day(ms: i64) -> String {
    let minutes = ms.rem_euclid(24 * 3_600_000) / MINUTE_MS;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}
