//! The organizer's deposit lists name each slip. Names came from the Google
//! Sheet only, so an unreadable sheet (or shifted rows, `.issues/151`) showed
//! raw attendee ids. `resolve_attendee_names` now reads D1 first and asks the
//! sheet only for ids D1 lacks.

const SRC: &str = include_str!("../src/handlers/deposit/thb/handlers/mod.rs");

#[test]
fn names_come_from_d1_before_the_sheet() {
    let start = SRC
        .find("pub(crate) async fn resolve_attendee_names(")
        .expect("resolve_attendee_names");
    let body = &SRC[start..];
    let body = &body[..body.find("\n}\n").expect("end of fn")];
    let d1 = body
        .find("crate::db::attendees::get_attendees_by_event(")
        .expect("D1 read");
    let sheet = body
        .find("sheets::get_attendees_map(")
        .expect("sheet fallback");
    assert!(d1 < sheet, "D1 first, the sheet only as a fallback");
    assert!(
        body[..sheet].contains("return names;"),
        "skip the sheet when D1 named every deposit"
    );
}
