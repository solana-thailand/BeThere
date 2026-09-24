//! Refund-proof links are staff-typed and attendee-clicked (`.issues/145`).

use event_checkin_domain::validation::safe_document_link;

#[test]
fn https_and_storage_paths_pass_through() {
    for url in [
        "https://bank.example/receipt/123",
        "https://drive.google.com/file/d/abc/view?usp=sharing",
        "/api/storage/refunds/evt_1/att_1",
        "/api/storage/slips/evt_1/att_1",
    ] {
        assert_eq!(safe_document_link(url), Some(url), "{url}");
    }
}

#[test]
fn script_and_other_schemes_are_rejected() {
    for url in [
        "javascript:alert(document.domain)",
        "JavaScript:alert(1)",
        " javascript:alert(1)",
        "java\tscript:alert(1)",
        "data:text/html;base64,PHNjcmlwdD4=",
        "data:image/png;base64,iVBORw0KGgo=",
        "http://bank.example/receipt",
        "vbscript:msgbox(1)",
        "//evil.example/x",
        "https:///evil.example",
        "https://",
        "/\\evil.example",
        "/ticket/other-page",
        "",
        "https://bank.example/a b",
    ] {
        assert_eq!(safe_document_link(url), None, "{url:?}");
    }
}
