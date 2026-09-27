//! Plan 028 F6: the roster list rebuilds only when its rows change.
//!
//! The list closure used to read `selected_ids`, so every checkbox tick
//! rebuilt every visible row, and the search box wrote the filter on every
//! keystroke. Rows now read their own selection, and the filter trails the
//! box by a debounce.

const PAGE: &str = include_str!("../src/pages/admin/page.rs");
const ROW: &str = include_str!("../src/pages/admin/attendee_row.rs");

fn roster_list_closure() -> &'static str {
    let start = PAGE
        .find("// Rows read the selection themselves")
        .expect("roster list closure moved; update this guard");
    let body = &PAGE[start..];
    &body[..body.find("}.into_any()").expect("list closure end")]
}

#[test]
fn the_list_closure_does_not_track_the_selection() {
    assert!(!roster_list_closure().contains("selected_ids"));
}

#[test]
fn each_row_tracks_its_own_selection() {
    assert!(ROW.contains("selected_ids.with(|ids| ids.contains(&id))"));
    assert!(!ROW.contains("is_selected: bool"));
}

#[test]
fn search_filter_is_debounced() {
    assert!(PAGE.contains("prop:value=move || search_input.get()"));
    assert!(PAGE.contains("move || set_search_query.set(val)"));
    assert!(PAGE.contains("pending.clear()"));
}
