//! The catalogue of past public events (.plans/045 R4.3): what ran, when,
//! in which series, and where its recording is. Read by `/events` for the
//! empty state ("Road to Mainnet has run 6 times, about every 4 weeks") and
//! "Learn from past events".
//!
//! A hand-kept snapshot, like `facts`: public events only, nothing about
//! people. Taken 2026-10-07 from D1 (read-only, devrel-helper
//! `scripts/site_events.py`); the recordings are the public YouTube videos.
//! Past events are listed by the API only once a recap is published, and
//! events carry no series or recording field yet (series become campaigns
//! with R4.4), so until then this is the one source. Add a row when an event
//! ends.

/// A series of events; each one is a course you can watch episode by episode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Series {
    /// Solana x AI Builders: The Road to Mainnet, the Bangkok meetups.
    RoadToMainnet,
    /// Solana in Latent Space, the online workshops.
    LatentSpace,
    /// A one-off event.
    Single,
}

impl Series {
    /// The series a visitor can follow as a course, in display order.
    pub const COURSES: [Series; 2] = [Series::RoadToMainnet, Series::LatentSpace];

    /// The course's address, `/events/<slug>` (.plans/045 R4.4), and its key
    /// in `course_enrolments` / `course_progress`. A one-off is no course.
    pub const fn course_slug(self) -> Option<&'static str> {
        match self {
            Series::RoadToMainnet => Some("road-to-mainnet"),
            Series::LatentSpace => Some("solana-in-latent-space"),
            Series::Single => None,
        }
    }

    /// The course at `slug`, if there is one.
    pub fn from_course_slug(slug: &str) -> Option<Series> {
        Series::COURSES
            .into_iter()
            .find(|s| s.course_slug() == Some(slug))
    }
}

/// A course's recorded episodes, oldest first: what `/events/<slug>` lists
/// and the only episodes progress can be recorded for.
pub fn course_episodes(series: Series) -> Vec<PastEvent> {
    episodes(&CATALOGUE, series)
        .into_iter()
        .filter(|e| !e.video.is_empty())
        .collect()
}

/// Whether `episode` (an event slug) is an episode of the course at
/// `course` (a course slug): the worker's input check before it writes.
pub fn is_course_episode(course: &str, episode: &str) -> bool {
    Series::from_course_slug(course)
        .is_some_and(|s| course_episodes(s).iter().any(|e| e.slug == episode))
}

/// One past public event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PastEvent {
    pub slug: &'static str,
    pub name: &'static str,
    pub series: Series,
    /// The episode number within its series (1 for a one-off).
    pub ep: u32,
    pub start_ms: i64,
    /// The YouTube video id of the recording; empty when there is none.
    pub video: &'static str,
}

/// When the snapshot was taken.
pub const CATALOGUE_MEASURED_AT: &str = "2026-10-07";

/// Every past public event, oldest first.
pub const CATALOGUE: [PastEvent; 14] = [
    PastEvent {
        slug: "solana-x-ai-builders-the-road-to-mainnet-1-bangkok",
        name: "Solana x AI Builders: The Road to Mainnet #1 (Bangkok)",
        series: Series::RoadToMainnet,
        ep: 1,
        start_ms: 1777170600000,
        video: "vGhI2oxDKjI",
    },
    PastEvent {
        slug: "solana-x-ai-builders-the-road-to-mainnet-2-bangkok",
        name: "Solana x AI Builders: The Road to Mainnet #2 (Bangkok)",
        series: Series::RoadToMainnet,
        ep: 2,
        start_ms: 1779602400000,
        video: "cVoItPytTJo",
    },
    PastEvent {
        slug: "intro-to-vibing-on-solana",
        name: "Intro to Vibing on Solana",
        series: Series::Single,
        ep: 1,
        start_ms: 1781096400000,
        video: "SPBrQd6FfoY",
    },
    PastEvent {
        slug: "solana-in-latent-space-part-1",
        name: "Solana in Latent Space Part 1",
        series: Series::LatentSpace,
        ep: 1,
        start_ms: 1781701200000,
        video: "1oU6IvwyT1w",
    },
    PastEvent {
        slug: "solana-x-ai-builders-the-road-to-mainnet-3-bangkok",
        name: "Solana x AI Builders: The Road to Mainnet #3 (Bangkok)",
        series: Series::RoadToMainnet,
        ep: 3,
        start_ms: 1781935200000,
        video: "WYMjGDehtjs",
    },
    PastEvent {
        slug: "islanddao-v4-demo",
        name: "IslandDAO V4 Demo",
        series: Series::Single,
        ep: 1,
        start_ms: 1782180000000,
        video: "",
    },
    PastEvent {
        slug: "solana-in-latent-space-part-2",
        name: "Solana in Latent Space Part 2",
        series: Series::LatentSpace,
        ep: 2,
        start_ms: 1782306000000,
        video: "3R5XG_UxWfs",
    },
    PastEvent {
        slug: "solana-in-latent-space-part-3",
        name: "Solana in Latent Space Part 3",
        series: Series::LatentSpace,
        ep: 3,
        start_ms: 1782910800000,
        video: "BLErmADEhLU",
    },
    PastEvent {
        slug: "solana-in-latent-space-part-4",
        name: "Solana in Latent Space Part 4",
        series: Series::LatentSpace,
        ep: 4,
        start_ms: 1783515600000,
        video: "-IKXs3x4wBk",
    },
    PastEvent {
        slug: "solana-in-latent-space-part-5",
        name: "Solana in Latent Space Part 5",
        series: Series::LatentSpace,
        ep: 5,
        start_ms: 1784120400000,
        video: "xHQZi4qE9Hk",
    },
    PastEvent {
        slug: "solana-x-ai-builders-the-road-to-mainnet-4-bangkok",
        name: "Solana x AI Builders: The Road to Mainnet #4 (Bangkok)",
        series: Series::RoadToMainnet,
        ep: 4,
        start_ms: 1784440800000,
        video: "UbijAMArpkw",
    },
    PastEvent {
        slug: "solana-in-latent-space-part-6",
        name: "Solana in Latent Space Part 6",
        series: Series::LatentSpace,
        ep: 6,
        start_ms: 1784725200000,
        video: "uD4mDGiAeJQ",
    },
    PastEvent {
        slug: "solana-x-ai-builders-the-road-to-mainnet-5-bangkok",
        name: "Solana x AI Builders: The Road to Mainnet #5 (Bangkok)",
        series: Series::RoadToMainnet,
        ep: 5,
        start_ms: 1787464800000,
        video: "aHghneIdEYQ",
    },
    PastEvent {
        slug: "solana-x-ai-builders-the-road-to-mainnet-6-bangkok",
        name: "Solana x AI Builders: The Road to Mainnet #6 (Bangkok)",
        series: Series::RoadToMainnet,
        ep: 6,
        start_ms: 1791093600000,
        video: "gzFU1NvC3aw",
    },
];

const DAY_MS: i64 = 86_400_000;

/// How often a series has run: the count, the median gap between episodes
/// in whole weeks (rounded), and the latest episode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cadence {
    pub times: u32,
    pub every_weeks: u32,
    pub last: PastEvent,
}

/// `series`' episodes in `events`, oldest first.
pub fn episodes(events: &[PastEvent], series: Series) -> Vec<PastEvent> {
    let mut eps: Vec<PastEvent> = events
        .iter()
        .copied()
        .filter(|e| e.series == series)
        .collect();
    eps.sort_by_key(|e| e.start_ms);
    eps
}

/// The empty state's line for `series`: `None` under three episodes, where a
/// median gap says nothing. The median of an even count is the upper middle
/// gap, as in the prototype.
pub fn cadence(events: &[PastEvent], series: Series) -> Option<Cadence> {
    let eps = episodes(events, series);
    if eps.len() < 3 {
        return None;
    }
    let mut gaps: Vec<i64> = eps
        .windows(2)
        .map(|w| w[1].start_ms - w[0].start_ms)
        .collect();
    gaps.sort_unstable();
    let median_days = gaps[gaps.len() / 2] as f64 / DAY_MS as f64;
    Some(Cadence {
        times: eps.len() as u32,
        every_weeks: (median_days / 7.0).round() as u32,
        last: *eps.last()?,
    })
}
