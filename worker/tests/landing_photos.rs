//! The landing photo list (.plans/043 L9): one photo per line, the owner's
//! caption rule, hashes for every stored object, and a serving allowlist.
//! Deliberately no count: taking a photo down must stay a one-line change.

use event_checkin_worker::landing_photos::{is_listed, parse, photos};
use event_checkin_worker::storage::Visibility;

const LIST: &str = include_str!("../landing-photos.jsonl");

#[test]
fn one_photo_per_line_and_every_line_parses() {
    let lines: Vec<_> = LIST.lines().collect();
    assert!(lines.iter().all(|l| !l.trim().is_empty()), "no blank lines");
    assert_eq!(parse(LIST).unwrap().len(), lines.len());
    assert_eq!(photos().len(), lines.len());
    assert!(
        parse("{\"file\":\"x.jpg\"}").is_err(),
        "a short line is an error"
    );
}

#[test]
fn captions_are_event_labels_and_objects_are_hashed() {
    let mut names = std::collections::HashSet::new();
    for p in photos() {
        let label = p.event.strip_prefix("RTM #").unwrap_or_default();
        assert!(
            !label.is_empty() && label.chars().all(|c| c.is_ascii_digit()),
            "{}",
            p.event
        );
        assert!(
            p.alt_en.contains(&p.event) && p.alt_th.contains(&p.event),
            "{}",
            p.file
        );
        for h in [&p.sha256, &p.thumb_sha256] {
            assert!(
                h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()),
                "{}",
                p.file
            );
        }
        for name in [&p.file, &p.thumb] {
            assert!(name.ends_with(".jpg") && !name.contains('/'), "{name}");
            assert!(names.insert(name.clone()), "{name} listed twice");
        }
        assert!(p.w > 0 && p.h > 0 && p.w.max(p.h) <= 1600, "{}", p.file);
    }
}

#[test]
fn only_listed_names_are_served_and_briefly_cached() {
    let list = photos();
    let first = list.first().expect("at least one photo");
    assert!(is_listed(&list, &first.file) && is_listed(&list, &first.thumb));
    for name in ["", "x.jpg", "../slips/e/a.jpg", "landing-photos/x.jpg"] {
        assert!(!is_listed(&list, name), "{name}");
    }
    assert_eq!(
        Visibility::Removable.cache_control(),
        "public, max-age=3600"
    );
}
