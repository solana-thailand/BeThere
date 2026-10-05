//! Sponsor list validation (migration 0057, plan 039 F7-b). Both URLs land in
//! a public `src`/`href`, so everything but `https` is refused at the write.

use event_checkin_domain::models::event::{
    EventConfig, MAX_SPONSOR_NAME_CHARS, MAX_SPONSORS, Sponsor, UpdateEventRequest,
    normalize_sponsors,
};

fn sponsor(name: &str, logo_url: &str, link: &str) -> Sponsor {
    Sponsor {
        name: name.into(),
        logo_url: logo_url.into(),
        link: link.into(),
    }
}

#[test]
fn trims_and_drops_blank_rows() {
    let raw = [
        sponsor("  Solana Foundation ", " https://example.com/logo.png ", ""),
        sponsor("  ", "", "   "),
    ];
    assert_eq!(
        normalize_sponsors(&raw),
        Ok(vec![sponsor(
            "Solana Foundation",
            "https://example.com/logo.png",
            ""
        )])
    );
}

#[test]
fn empty_list_is_valid() {
    assert_eq!(normalize_sponsors(&[]), Ok(Vec::new()));
}

#[test]
fn a_row_with_a_url_but_no_name_is_refused() {
    let err = normalize_sponsors(&[sponsor("", "https://example.com/l.png", "")]).unwrap_err();
    assert!(err.contains("sponsor 1 needs a name"), "{err}");
}

#[test]
fn non_https_urls_are_refused() {
    for bad in [
        "http://example.com/logo.png",
        "javascript:alert(1)",
        "data:image/svg+xml,<svg/>",
        "/api/poster/x.png",
        "https://",
        "https:///path",
        "https://exa mple.com",
    ] {
        assert!(
            normalize_sponsors(&[sponsor("A", bad, "")]).is_err(),
            "logo {bad}"
        );
        assert!(
            normalize_sponsors(&[sponsor("A", "", bad)]).is_err(),
            "link {bad}"
        );
    }
}

#[test]
fn error_text_has_no_scheme_separator() {
    // The API error redactor rewrites "://" to [redacted-url].
    let err = normalize_sponsors(&[sponsor("A", "http://x.com", "")]).unwrap_err();
    assert!(!err.contains("://"), "{err}");
}

#[test]
fn count_and_name_are_bounded() {
    let many: Vec<Sponsor> = (0..=MAX_SPONSORS)
        .map(|i| sponsor(&format!("S{i}"), "", ""))
        .collect();
    assert!(normalize_sponsors(&many).is_err());
    assert!(normalize_sponsors(&many[..MAX_SPONSORS]).is_ok());

    // Characters, not bytes: a Thai name at the limit is accepted.
    let at_limit = "ก".repeat(MAX_SPONSOR_NAME_CHARS);
    assert!(normalize_sponsors(&[sponsor(&at_limit, "", "")]).is_ok());
    let over = "ก".repeat(MAX_SPONSOR_NAME_CHARS + 1);
    assert!(normalize_sponsors(&[sponsor(&over, "", "")]).is_err());
}

#[test]
fn config_without_sponsors_deserializes_and_omits_the_field() {
    let config = EventConfig::from_global_config(
        "E",
        "",
        "",
        1_790_000_000_000,
        1_790_003_600_000,
        "sheet",
        "Attendees",
        "staff",
        "",
        "",
        "",
        "",
        vec![],
        vec![],
        "",
        "",
    );
    let mut json = serde_json::to_value(&config).unwrap();
    assert!(json.get("sponsors").is_none(), "{json}");
    json.as_object_mut().unwrap().remove("sponsors");
    let back: EventConfig = serde_json::from_value(json).unwrap();
    assert!(back.sponsors.is_empty());
}

#[test]
fn update_request_distinguishes_absent_from_cleared() {
    let absent: UpdateEventRequest = serde_json::from_str("{}").unwrap();
    assert_eq!(absent.sponsors, None);
    let cleared: UpdateEventRequest = serde_json::from_str(r#"{"sponsors":[]}"#).unwrap();
    assert_eq!(cleared.sponsors, Some(Vec::new()));
}
