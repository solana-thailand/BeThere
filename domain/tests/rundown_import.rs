//! Ontime rundown import tests (plan 019 Phase 1).
//!
//! Fixtures are the two real Ontime exports committed to this repo, so the
//! parser is pinned against files an organizer actually produced rather than
//! against a shape invented for the test.

use event_checkin_domain::models::rundown::{
    RundownError, parse_ontime_csv, to_agenda_text,
};

const APR: &str = include_str!("../../ontime/solana-dev-thailand-26apr.csv");
const BKK: &str = include_str!("../../ontime/road-to-mainnet-3-bangkok.csv");

// --- 3.1 real exports parse -------------------------------------------------

#[test]
fn parses_the_committed_april_export() {
    let sessions = parse_ontime_csv(APR).expect("real export should parse");
    assert_eq!(sessions.len(), 8, "8 non-skipped rows in the April export");
    assert_eq!(sessions[0].title, "Registration");
    assert_eq!(sessions[0].start_local, "09:30");
    assert_eq!(sessions[0].duration, "00:30");
    assert_eq!(sessions[2].title, "Rust AI and Gaming Ep. 2");
    assert_eq!(sessions[2].presenter, "Katopz");
}

#[test]
fn parses_the_committed_bangkok_export() {
    let sessions = parse_ontime_csv(BKK).expect("real export should parse");
    assert!(!sessions.is_empty());
    assert_eq!(sessions[0].title, "Setup");
}

/// Operator-only columns must never reach a public field.
#[test]
fn drops_run_of_show_columns() {
    let sessions = parse_ontime_csv(APR).unwrap();
    let agenda = to_agenda_text(&sessions);
    for leaked in ["Registration desk opens", "Deep dive session", "grey", "load-next", "count-down"] {
        assert!(!agenda.contains(leaked), "operator field leaked: {leaked}");
    }
}

// --- 3.2 Skip ---------------------------------------------------------------

#[test]
fn excludes_skipped_rows() {
    let csv = "Time start,Duration,Cue,Title,Skip,Presenter\n\
               09:00,00:30,1,Kept,false,\n\
               09:30,00:30,2,Dropped,true,\n\
               10:00,00:30,3,AlsoKept,TRUE_NOT,\n";
    let s = parse_ontime_csv(csv).unwrap();
    let titles: Vec<&str> = s.iter().map(|x| x.title.as_str()).collect();
    assert_eq!(titles, vec!["Kept", "AlsoKept"]);
}

#[test]
fn skip_matching_is_case_insensitive() {
    let csv = "Cue,Title,Skip\n1,Gone,TRUE\n2,Here,False\n";
    let s = parse_ontime_csv(csv).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].title, "Here");
}

// --- 3.3 empty presenter ----------------------------------------------------

#[test]
fn empty_presenter_leaves_no_trailing_dash() {
    let sessions = parse_ontime_csv(APR).unwrap();
    let agenda = to_agenda_text(&sessions);
    assert!(agenda.contains("09:30  Registration"));
    assert!(
        !agenda.contains("Registration —"),
        "empty presenter must not emit a dash:\n{agenda}"
    );
    for line in agenda.lines() {
        assert!(!line.trim_end().ends_with('—'), "dangling dash: {line}");
    }
}

#[test]
fn presenter_is_appended_when_present() {
    let sessions = parse_ontime_csv(APR).unwrap();
    let agenda = to_agenda_text(&sessions);
    assert!(agenda.contains("Rust AI and Gaming Ep. 2 — Katopz"));
}

// --- 3.4 quoted fields ------------------------------------------------------

#[test]
fn quoted_field_containing_a_comma_stays_one_field() {
    let csv = "Cue,Title,Presenter\n1,\"Rust, Anchor and You\",Katopz\n";
    let s = parse_ontime_csv(csv).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].title, "Rust, Anchor and You");
    assert_eq!(s[0].presenter, "Katopz");
}

#[test]
fn escaped_double_quotes_are_unescaped() {
    let csv = "Cue,Title\n1,\"The \"\"Hello World\"\" talk\"\n";
    let s = parse_ontime_csv(csv).unwrap();
    assert_eq!(s[0].title, "The \"Hello World\" talk");
}

/// CSV permits newlines inside quotes; `lines()`-based parsers corrupt them.
#[test]
fn newline_inside_quotes_does_not_split_the_record() {
    let csv = "Cue,Title,Presenter\n1,\"Two\nLines\",Katopz\n";
    let s = parse_ontime_csv(csv).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].title, "Two\nLines");
    assert_eq!(s[0].presenter, "Katopz");
}

// --- 3.5 numeric ordering ---------------------------------------------------

#[test]
fn orders_by_cue_numerically_not_lexically() {
    let csv = "Cue,Title\n10,Ten\n9,Nine\n1,One\n";
    let s = parse_ontime_csv(csv).unwrap();
    let titles: Vec<&str> = s.iter().map(|x| x.title.as_str()).collect();
    assert_eq!(titles, vec!["One", "Nine", "Ten"]);
}

#[test]
fn missing_cue_falls_back_to_row_order() {
    let csv = "Title\nFirst\nSecond\nThird\n";
    let s = parse_ontime_csv(csv).unwrap();
    let titles: Vec<&str> = s.iter().map(|x| x.title.as_str()).collect();
    assert_eq!(titles, vec!["First", "Second", "Third"]);
}

// --- 3.6 malformed input ----------------------------------------------------

#[test]
fn empty_input_is_an_error() {
    assert_eq!(parse_ontime_csv(""), Err(RundownError::Empty));
    assert_eq!(parse_ontime_csv("\n\n  \n"), Err(RundownError::Empty));
}

#[test]
fn a_file_without_a_title_column_is_rejected() {
    let csv = "Foo,Bar\n1,2\n";
    assert_eq!(parse_ontime_csv(csv), Err(RundownError::MissingTitleColumn));
}

#[test]
fn header_only_file_reports_no_sessions() {
    let csv = "Time start,Duration,Cue,Title,Skip,Presenter\n";
    assert_eq!(parse_ontime_csv(csv), Err(RundownError::NoSessions));
}

#[test]
fn all_rows_skipped_reports_no_sessions() {
    let csv = "Cue,Title,Skip\n1,A,true\n2,B,true\n";
    assert_eq!(parse_ontime_csv(csv), Err(RundownError::NoSessions));
}

#[test]
fn errors_render_a_usable_message() {
    assert!(RundownError::MissingTitleColumn.to_string().contains("Title"));
    assert!(!RundownError::Empty.to_string().is_empty());
}

// --- header robustness ------------------------------------------------------

#[test]
fn header_matching_is_case_and_order_insensitive() {
    let csv = "presenter,TITLE,cue,Time Start\nKatopz,Talk,1,09:00\n";
    let s = parse_ontime_csv(csv).unwrap();
    assert_eq!(s[0].title, "Talk");
    assert_eq!(s[0].presenter, "Katopz");
    assert_eq!(s[0].start_local, "09:00");
}

#[test]
fn crlf_line_endings_parse() {
    let csv = "Cue,Title\r\n1,Windows Export\r\n";
    let s = parse_ontime_csv(csv).unwrap();
    assert_eq!(s[0].title, "Windows Export");
}

/// A row shorter than the header must not panic.
#[test]
fn short_rows_are_tolerated() {
    let csv = "Time start,Duration,Cue,Title,Skip,Presenter\n09:00,00:30,1,Talk\n";
    let s = parse_ontime_csv(csv).unwrap();
    assert_eq!(s[0].title, "Talk");
    assert_eq!(s[0].presenter, "");
}

// --- agenda rendering -------------------------------------------------------

#[test]
fn agenda_of_the_real_export_is_stable() {
    let sessions = parse_ontime_csv(APR).unwrap();
    let agenda = to_agenda_text(&sessions);
    let lines: Vec<&str> = agenda.lines().collect();
    assert_eq!(lines.len(), 8);
    assert_eq!(lines[0], "09:30  Registration");
    assert_eq!(lines[2], "10:10  Rust AI and Gaming Ep. 2 — Katopz");
    assert_eq!(lines[7], "12:10  Networking Session");
}

#[test]
fn title_only_session_renders_without_leading_spaces() {
    let csv = "Cue,Title\n1,No Time\n";
    let s = parse_ontime_csv(csv).unwrap();
    assert_eq!(to_agenda_text(&s), "No Time");
}

#[test]
fn empty_session_list_renders_empty() {
    assert_eq!(to_agenda_text(&[]), "");
}

// --- presenter already named in the title -----------------------------------

/// Real rundowns repeat the speaker in the title; appending it again reads as a
/// bug. Observed in the committed April export, row 2.
#[test]
fn presenter_already_in_the_title_is_not_repeated() {
    let sessions = parse_ontime_csv(APR).unwrap();
    let agenda = to_agenda_text(&sessions);
    let line = agenda
        .lines()
        .find(|l| l.contains("Opening by"))
        .expect("the opening session should be present");
    assert_eq!(
        line, "10:00  Opening by Solana Developer Thailand & Solana Thailand DAO",
        "presenter duplicated: {line}"
    );
}

#[test]
fn redundancy_check_is_case_insensitive() {
    let csv = "Cue,Title,Presenter\n1,Keynote by KATOPZ,Katopz\n";
    let s = parse_ontime_csv(csv).unwrap();
    assert_eq!(to_agenda_text(&s), "Keynote by KATOPZ");
}

/// A distinct presenter must still be appended.
#[test]
fn distinct_presenter_is_still_appended() {
    let csv = "Cue,Title,Presenter\n1,Deep Dive,Golf\n";
    let s = parse_ontime_csv(csv).unwrap();
    assert_eq!(to_agenda_text(&s), "Deep Dive — Golf");
}
