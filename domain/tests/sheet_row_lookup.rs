//! Sheet writes find the attendee's row by `api_id` in column A, at write time.
//!
//! D1's `sheet_row_index` was empty for every attendee D1 created, so writers
//! that trusted it addressed row 0 and every status write failed silently; and
//! organizers insert rows by hand, so any remembered number goes stale. These
//! pin the lookup and the append-position parser the worker relies on.

use event_checkin_domain::models::attendee::{
    RowMatch, column_index, column_letter, find_row, range_start,
};

fn column(cells: &[&str]) -> Vec<Vec<String>> {
    cells
        .iter()
        .map(|c| match c.is_empty() {
            true => vec![],
            false => vec![c.to_string()],
        })
        .collect()
}

#[test]
fn finds_the_row_after_rows_were_inserted_above_it() {
    let before = column(&["api_id", "a1", "a2", "a3"]);
    assert_eq!(find_row(&before, "a3"), RowMatch::Found(4));
    // Two rows inserted by hand above a3 (one blank, one without an api_id).
    let after = column(&["api_id", "a1", "", "", "a2", "a3"]);
    assert_eq!(find_row(&after, "a3"), RowMatch::Found(6));
}

#[test]
fn the_header_never_matches() {
    assert_eq!(
        find_row(&column(&["api_id", "a1"]), "api_id"),
        RowMatch::NotFound
    );
}

#[test]
fn a_missing_or_blank_id_is_not_found() {
    let sheet = column(&["api_id", "a1", ""]);
    assert_eq!(find_row(&sheet, "zz"), RowMatch::NotFound);
    assert_eq!(find_row(&sheet, ""), RowMatch::NotFound);
    assert_eq!(find_row(&sheet, "  "), RowMatch::NotFound);
}

#[test]
fn a_duplicated_id_is_refused_rather_than_guessed() {
    let sheet = column(&["api_id", "a1", "a2", "a1"]);
    assert_eq!(find_row(&sheet, "a1"), RowMatch::Duplicate);
    assert_eq!(find_row(&sheet, "a2"), RowMatch::Found(3));
}

#[test]
fn cells_are_compared_trimmed() {
    assert_eq!(
        find_row(&column(&["api_id", " a1 "]), "a1"),
        RowMatch::Found(2)
    );
}

#[test]
fn column_letters_round_trip() {
    for (idx, letters) in [
        (0, "A"),
        (1, "B"),
        (25, "Z"),
        (26, "AA"),
        (34, "AI"),
        (701, "ZZ"),
        (702, "AAA"),
    ] {
        assert_eq!(column_letter(idx), letters);
        assert_eq!(column_index(letters), Some(idx));
    }
    assert_eq!(column_index("ab"), Some(27));
    assert_eq!(column_index(""), None);
    assert_eq!(column_index("A1"), None);
}

#[test]
fn range_start_reads_where_an_append_landed() {
    assert_eq!(range_start("Attendees!A57:AH57"), Some((0, 57)));
    // The 2026-09-25 RTM#6 row: shifted one column right.
    assert_eq!(range_start("Attendees!B57:AI57"), Some((1, 57)));
    assert_eq!(range_start("'VIP list'!AA3:AB3"), Some((26, 3)));
    assert_eq!(range_start("B2"), Some((1, 2)));
    assert_eq!(range_start("Attendees!A:A"), None);
    assert_eq!(range_start(""), None);
}
