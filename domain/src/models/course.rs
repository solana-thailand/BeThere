//! Courses (.plans/045 R4.4, owner 2026-10-11): a course is an active
//! campaign, its episodes the campaign's public events ordered by date, each
//! with the recording from the event's own `video_url`. Series that go on
//! (Road to Mainnet, Solana in Latent Space) grow when an organizer adds the
//! next event to the campaign; nothing here lists events by hand.
//!
//! The worker serves these types (`GET /api/public/courses[/{id}]`); the
//! frontend renders them. The helpers are pure and tested natively.

use serde::{Deserialize, Serialize};

/// A course on `/events` (Learn from past events).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseSummary {
    /// The campaign id, also the course's address: `/events/<id>`.
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Episodes that have happened.
    pub held: u32,
    /// Start times of the held episodes, oldest first (for the cadence line).
    #[serde(default)]
    pub held_starts_ms: Vec<i64>,
}

/// One episode of a course.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseEpisode {
    /// The event slug: the episode's key in progress, and `/e/<slug>`.
    pub slug: String,
    pub name: String,
    pub start_ms: i64,
    pub end_ms: i64,
    /// The YouTube video id of the recording, or empty.
    #[serde(default)]
    pub video: String,
}

impl CourseEpisode {
    /// Not over yet: the next episode, to register for.
    pub fn upcoming(&self, now_ms: i64) -> bool {
        self.end_ms > now_ms
    }
}

/// A course page: the course and its episodes, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseDetail {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub episodes: Vec<CourseEpisode>,
}

/// Whether `id` can be a course address (campaign ids are slugs). Checked
/// before anything reaches D1.
pub fn is_course_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The 11-character YouTube id in `url` (watch, youtu.be, embed, shorts,
/// live, or a bare id), or `None`.
pub fn youtube_id(url: &str) -> Option<String> {
    let url = url.trim();
    let candidate = if let Some((_, rest)) = url.split_once("v=") {
        rest
    } else if let Some(rest) = ["youtu.be/", "/embed/", "/shorts/", "/live/"]
        .iter()
        .find_map(|m| url.split_once(m).map(|(_, r)| r))
    {
        rest
    } else {
        url
    };
    let id: String = candidate
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    (id.len() == 11).then_some(id)
}

const DAY_MS: i64 = 86_400_000;

/// How often a course has run: the count, the median gap between episodes
/// in whole weeks (rounded), and the latest start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cadence {
    pub times: u32,
    pub every_weeks: u32,
    pub last_ms: i64,
}

/// The empty state's line, from the held episodes' start times: `None` under
/// three, where a median gap says nothing. The median of an even count of
/// gaps is the upper middle one.
pub fn cadence(starts_ms: &[i64]) -> Option<Cadence> {
    if starts_ms.len() < 3 {
        return None;
    }
    let mut starts = starts_ms.to_vec();
    starts.sort_unstable();
    let mut gaps: Vec<i64> = starts.windows(2).map(|w| w[1] - w[0]).collect();
    gaps.sort_unstable();
    let median_days = gaps[gaps.len() / 2] as f64 / DAY_MS as f64;
    Some(Cadence {
        times: starts.len() as u32,
        every_weeks: (median_days / 7.0).round() as u32,
        last_ms: *starts.last()?,
    })
}

/// The course whose cadence the empty state quotes: the one held most often.
pub fn busiest(courses: &[CourseSummary]) -> Option<&CourseSummary> {
    courses.iter().max_by_key(|c| c.held)
}
