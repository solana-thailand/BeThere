//! Plan 028 F4: the scanner fetches the selected event's detail once per
//! selection. The mount task used to fetch it as well, and then setting
//! `active_event_id` made the selection effect fetch it again.

const PAGE: &str = include_str!("../src/pages/scanner/page.rs");

#[test]
fn event_detail_is_fetched_from_one_place() {
    assert_eq!(PAGE.matches("api::get_event_detail(").count(), 1);
}

#[test]
fn a_stale_detail_response_is_dropped() {
    let at = PAGE
        .find("api::get_event_detail(")
        .expect("detail fetch moved; update this guard");
    let after = &PAGE[at..];
    let check = after
        .find("active_event_id.try_get_untracked()")
        .expect("the selection must be re-read after the fetch");
    let apply = after.find("set_ee.set(").expect("escrow flag apply moved");
    assert!(
        check < apply,
        "re-check the selection before applying the detail"
    );
}
