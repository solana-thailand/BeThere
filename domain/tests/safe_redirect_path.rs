//! Post-login redirect targets must stay on this origin (open redirect fix,
//! ISO 27001 gap assessment 2026-09-23).

use event_checkin_domain::validation::safe_redirect_path;

#[test]
fn same_origin_paths_pass_through() {
    for ok in [
        "/",
        "/e/rtm-6",
        "/ticket/abc?event_id=rtm-6",
        "/feedback#top",
    ] {
        assert_eq!(safe_redirect_path(ok), Some(ok), "{ok}");
    }
}

#[test]
fn off_origin_and_script_targets_are_rejected() {
    for bad in [
        "",
        "https://evil.example",
        "http://evil.example/e/rtm-6",
        "javascript:alert(1)",
        "JavaScript:alert(1)",
        "data:text/html,x",
        "//evil.example",
        "/\\evil.example",
        "\\\\evil.example",
        "/e/\\..\\evil",
        "/\t/evil.example",
        "/\n/evil.example",
        "/ok\r\nSet-Cookie: x=1",
        "evil.example",
        " /e/rtm-6",
    ] {
        assert_eq!(safe_redirect_path(bad), None, "{bad:?}");
    }
}
