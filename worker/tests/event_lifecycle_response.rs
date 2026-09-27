//! Archive and restore responses carry the event name. The admin toasts read
//! `data.name`, and without it they said "Event '' archived".

#[test]
fn archive_and_restore_return_the_event_name() {
    let src = include_str!("../src/handlers/events/lifecycle.rs");
    for status in ["archived", "draft"] {
        let at = src
            .find(&format!("\"status\": \"{status}\","))
            .unwrap_or_else(|| panic!("{status} response missing"));
        let response = &src[src[..at].rfind("json!({").expect("response object")..at];
        assert!(
            response.contains("\"name\": config.name"),
            "the {status} response must include the event name"
        );
    }
}
