//! Ontime rundown import — upstream CSV → public agenda text.
//!
//! Organizers author a per-session rundown in Ontime for every event and export
//! it as CSV. This turns that export into the agenda shown under "About this
//! Event". See `.plans/019_ontime_rundown_import.md` for the format survey and
//! the reasoning behind the choices below.
//!
//! Two decisions worth restating here, because both are easy to get wrong:
//!
//! - **`Time start` is venue wall-clock, not an instant.** It is carried through
//!   as the literal string. Parsing it into an epoch would make BeThere localize
//!   it to the *viewer's* timezone, showing a Bangkok 09:30 slot as 02:30 in
//!   Europe. Only `event_start_ms` is an instant; an agenda row is not.
//! - **Only public columns are read.** `Note`, `Colour`, `End action`, `Timer
//!   type` and the warning thresholds are run-of-show operator fields and are
//!   deliberately dropped rather than published.

use core::fmt;

/// One publishable session from an Ontime rundown.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RundownSession {
    /// Ordering key from the `Cue` column; falls back to row order when absent.
    pub cue: i64,
    /// `Time start` verbatim (e.g. `"09:30"`) — venue wall-clock, never parsed.
    pub start_local: String,
    /// `Duration` verbatim (e.g. `"00:30"`).
    pub duration: String,
    /// Session name.
    pub title: String,
    /// Speaker; frequently empty (Registration, Networking, Group Photo).
    pub presenter: String,
}

/// Why an import could not produce an agenda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RundownError {
    /// File was empty or contained only blank lines.
    Empty,
    /// No `Title` column in the header row — almost certainly not an Ontime export.
    MissingTitleColumn,
    /// Header parsed, but every row was blank or skipped.
    NoSessions,
}

impl fmt::Display for RundownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "the file is empty"),
            Self::MissingTitleColumn => write!(
                f,
                "no 'Title' column found — export the rundown from Ontime as CSV"
            ),
            Self::NoSessions => write!(f, "no sessions to import (all rows blank or skipped)"),
        }
    }
}

/// Split CSV text into records, honouring quoted fields.
///
/// Handles `""` as an escaped quote and permits newlines *inside* quotes, so a
/// title containing a comma or line break survives. A naive `split(',')` over
/// `lines()` silently corrupts both — the fragility called out in plan 019 §5.4.
fn split_records(content: &str) -> Vec<Vec<String>> {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = content.chars().peekable();

    let push_field = |record: &mut Vec<String>, field: &mut String| {
        let taken = core::mem::take(field);
        record.push(taken.trim().to_string());
    };

    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => push_field(&mut record, &mut field),
            '\r' if !in_quotes => {}
            '\n' if !in_quotes => {
                push_field(&mut record, &mut field);
                records.push(core::mem::take(&mut record));
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        push_field(&mut record, &mut field);
        records.push(record);
    }
    records
}

/// Is this record entirely empty (blank line, or a row of empty cells)?
fn is_blank(record: &[String]) -> bool {
    record.iter().all(|f| f.is_empty())
}

/// Locate a column by header name, case-insensitively.
fn column(header: &[String], name: &str) -> Option<usize> {
    header
        .iter()
        .position(|h| h.trim().eq_ignore_ascii_case(name))
}

/// Read a field by optional column index, returning an empty string when the
/// column is absent or the row is short.
fn field(record: &[String], idx: Option<usize>) -> String {
    idx.and_then(|i| record.get(i))
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Parse an upstream Ontime CSV export into publishable sessions.
///
/// Rows flagged `Skip = true` are excluded — Ontime uses that for slots that
/// exist in the run-of-show but will not run, and publishing them would show
/// attendees sessions that never happen.
///
/// Output is sorted by `Cue` **numerically**, so cue 10 follows cue 9 rather
/// than cue 1.
pub fn parse_ontime_csv(content: &str) -> Result<Vec<RundownSession>, RundownError> {
    let records: Vec<Vec<String>> = split_records(content)
        .into_iter()
        .filter(|r| !is_blank(r))
        .collect();

    let (header, rows) = records.split_first().ok_or(RundownError::Empty)?;

    let title_col = column(header, "Title").ok_or(RundownError::MissingTitleColumn)?;
    let cue_col = column(header, "Cue");
    let start_col = column(header, "Time start");
    let duration_col = column(header, "Duration");
    let presenter_col = column(header, "Presenter");
    let skip_col = column(header, "Skip");

    let mut sessions = Vec::new();
    for (row_index, record) in rows.iter().enumerate() {
        if field(record, skip_col).eq_ignore_ascii_case("true") {
            continue;
        }
        let title = field(record, Some(title_col));
        if title.is_empty() {
            continue;
        }
        // Fall back to row order when `Cue` is absent or non-numeric, so a
        // hand-edited export still imports in a sensible order.
        let cue = field(record, cue_col)
            .parse::<i64>()
            .unwrap_or(row_index as i64);

        sessions.push(RundownSession {
            cue,
            start_local: field(record, start_col),
            duration: field(record, duration_col),
            title,
            presenter: field(record, presenter_col),
        });
    }

    match sessions.is_empty() {
        true => Err(RundownError::NoSessions),
        false => {
            sessions.sort_by_key(|s| s.cue);
            Ok(sessions)
        }
    }
}

/// Render sessions as the agenda text that goes into `EventConfig.description`.
///
/// The public page styles that field `white-space: pre-line`, so plain lines
/// with real newlines render as written — no markup needed.
pub fn to_agenda_text(sessions: &[RundownSession]) -> String {
    sessions
        .iter()
        .map(|s| {
            let head = match s.start_local.is_empty() {
                true => s.title.clone(),
                false => format!("{}  {}", s.start_local, s.title),
            };
            // Ontime rundowns often repeat the speaker inside the title
            // ("Opening by Solana Developer Thailand" / presenter "Solana
            // Developer Thailand"). Appending it again reads as a bug, so the
            // presenter is only added when it is not already in the title.
            let redundant = !s.presenter.is_empty()
                && s.title.to_lowercase().contains(&s.presenter.to_lowercase());
            match s.presenter.is_empty() || redundant {
                true => head,
                false => format!("{head} — {}", s.presenter),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
