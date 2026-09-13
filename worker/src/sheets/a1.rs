//! A1-notation helpers for the Google Sheets API.
//!
//! Every Sheets read and write in this module tree addresses cells with an A1
//! range — `Attendees!A2:R`. The part before the `!` is a *sheet name*, and
//! Google's grammar only accepts it bare when it is a plain identifier. Anything
//! else — a space, a hyphen, a `'`, a leading digit, or a name that could itself
//! be read as a cell reference — has to be single-quoted, with any interior
//! quote doubled.
//!
//! This matters because the name is not ours. `sheet_name` and
//! `staff_sheet_name` are free-text fields on the event form (defaults
//! `"Attendees"` / `"staff"`, only trimmed), so an organiser who names their tab
//! `Attendee List` or `Day 1` produced `Attendee List!A2:R` — which the API
//! rejects as `INVALID_ARGUMENT`. Almost every Sheets call in the worker is
//! detached best-effort work whose errors are logged and dropped, so the failure
//! was silent: the tab simply never filled in.
//!
//! [`sheet_ref`] is the single place that decision is made. Use it for the part
//! of a range before the `!`, and only there — a cache key, a gid lookup, or a
//! comparison against a title returned by the API all want the raw name.

/// Render a sheet name for use in an A1 range, quoting it when the grammar
/// requires it.
///
/// Quoting an already-safe name is *not* a no-op for the API (`'Attendees'!A1`
/// is accepted, but the extra quotes show up in returned range strings), so the
/// bare form is preserved whenever it is legal.
///
/// ```ignore
/// assert_eq!(sheet_ref("Attendees"), "Attendees");
/// assert_eq!(sheet_ref("Attendee List"), "'Attendee List'");
/// assert_eq!(sheet_ref("Bob's tab"), "'Bob''s tab'");
/// ```
pub fn sheet_ref(name: &str) -> String {
    match needs_quoting(name) {
        false => name.to_string(),
        true => format!("'{}'", name.replace('\'', "''")),
    }
}

/// Whether `name` must be single-quoted to appear before the `!` of a range.
///
/// Bare names are limited to `[A-Za-z0-9_]` that do not start with a digit, and
/// must not be readable as a cell reference — Google resolves `A1!B2` against a
/// *cell* `A1`, not a sheet called `A1`.
fn needs_quoting(name: &str) -> bool {
    let Some(first) = name.chars().next() else {
        // An empty name cannot be addressed at all; `''` at least fails loudly
        // at the API rather than silently producing the bare range `!A2:R`,
        // which Sheets reads as "the first tab".
        return true;
    };
    first.is_ascii_digit()
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        || looks_like_cell_reference(name)
}

/// Largest column label Sheets addresses (`ZZZ`, column 18278) — so a bare name
/// of four or more letters cannot be a column, and `Sheet1` (Google's own
/// default tab name) stays unquoted.
const MAX_COLUMN_LETTERS: usize = 3;

/// Largest row number Sheets addresses is seven digits, for the same reason.
const MAX_ROW_DIGITS: usize = 7;

/// Whether `name` reads as an A1 cell reference (`A1`, `AB12`) or an R1C1 one
/// (`R1C1`), either of which Sheets resolves as a cell rather than a tab.
///
/// Deliberately bounded by [`MAX_COLUMN_LETTERS`] / [`MAX_ROW_DIGITS`]: an
/// unbounded "letters then digits" rule would quote `Sheet1` and `Attendees2`,
/// which are not cell references and are common tab names.
fn looks_like_cell_reference(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    is_r1c1(&upper) || is_a1(&upper)
}

/// `R`, digits, `C`, digits — e.g. `R1C1`.
fn is_r1c1(upper: &str) -> bool {
    let Some(rest) = upper.strip_prefix('R') else {
        return false;
    };
    let Some((row, col)) = rest.split_once('C') else {
        return false;
    };
    is_bounded_digits(row, MAX_ROW_DIGITS) && is_bounded_digits(col, MAX_ROW_DIGITS)
}

/// One to [`MAX_COLUMN_LETTERS`] letters followed by one to [`MAX_ROW_DIGITS`]
/// digits, and nothing else — e.g. `A1`, `AB12`.
fn is_a1(upper: &str) -> bool {
    let letters = upper.bytes().take_while(u8::is_ascii_alphabetic).count();
    (1..=MAX_COLUMN_LETTERS).contains(&letters)
        && is_bounded_digits(&upper[letters..], MAX_ROW_DIGITS)
}

/// A non-empty run of at most `max` ASCII digits.
fn is_bounded_digits(s: &str, max: usize) -> bool {
    (1..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_identifiers_are_left_bare() {
        for name in [
            "Attendees",
            "staff",
            "waitlist",
            "Sheet1",
            "Sheet1_2",
            "_hidden",
            "A",
        ] {
            assert_eq!(sheet_ref(name), name, "{name} needs no quoting");
        }
    }

    #[test]
    fn the_shipped_defaults_are_left_bare() {
        // The event form's defaults and the hardcoded tabs. If quoting ever
        // starts firing on these, every existing event's ranges change shape.
        for name in ["Attendees", "staff", "waitlist"] {
            assert_eq!(sheet_ref(name), name);
        }
    }

    #[test]
    fn names_the_grammar_rejects_are_quoted() {
        // The realistic organiser input that motivated this helper.
        assert_eq!(sheet_ref("Attendee List"), "'Attendee List'");
        assert_eq!(sheet_ref("Day 1"), "'Day 1'");
        assert_eq!(sheet_ref("walk-ins"), "'walk-ins'");
        assert_eq!(sheet_ref("2026"), "'2026'");
        assert_eq!(sheet_ref("1st day"), "'1st day'");
        assert_eq!(sheet_ref("ผู้เข้าร่วม"), "'ผู้เข้าร่วม'");
    }

    #[test]
    fn interior_quotes_are_doubled() {
        assert_eq!(sheet_ref("Bob's tab"), "'Bob''s tab'");
        assert_eq!(sheet_ref("'"), "''''");
    }

    #[test]
    fn cell_shaped_names_are_quoted() {
        // `A1!B2` addresses cell A1, not a tab named A1.
        for name in ["A1", "AB12", "R1C1", "r1c1", "Z999"] {
            assert_eq!(
                sheet_ref(name),
                format!("'{name}'"),
                "{name} is cell-shaped"
            );
        }
    }

    #[test]
    fn identifiers_that_merely_contain_digits_stay_bare() {
        // Letters-then-digits is cell-shaped; anything else is not.
        // `Sheet1` is Google's default tab name and `ZZZZ1` has too many letters
        // to be a column; neither is a cell reference.
        for name in ["Sheet1", "Attendees2", "ZZZZ1", "A1B", "day1_2", "R1C1x"] {
            assert_eq!(sheet_ref(name), name, "{name} is not cell-shaped");
        }
    }

    #[test]
    fn an_empty_name_is_quoted_rather_than_dropped() {
        // Bare would yield `!A2:R`, which Sheets silently reads as the first
        // tab — a wrong-tab write. `''!A2:R` is rejected instead.
        assert_eq!(sheet_ref(""), "''");
    }
}
