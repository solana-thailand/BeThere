//! Creating an event shows only what it cannot exist without (.plans/049,
//! owner pick A): every Optional / Recommended section of the event form
//! starts collapsed on create (`open=!is_create`, or `open=false`), and the
//! Required ones start open. Deposit Details is the one exception and stays
//! open: a new event has the deposit on, and then the amount is required (the
//! form refuses "Create" with none), so collapsing it would hide the field the
//! error is about. Measured before: 38 fields over 9.3 phone screens, save
//! button at the end (`.benchmarks/009`); after: 25 fields, 6.3 screens, save
//! row stuck to the bottom of the screen.

use std::fs;
use std::path::Path;

struct Section {
    file: String,
    title: String,
    badge: String,
    open: Option<String>,
}

fn sections() -> Vec<Section> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/pages/event_form");
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).expect("event_form dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let src = fs::read_to_string(&path).expect("read");
        for chunk in src.split("<FormSection").skip(1) {
            let tag = chunk.split('>').next().unwrap_or_default();
            let attr = |name: &str| {
                let key = format!("{name}=");
                tag.split(&key)
                    .nth(1)
                    .map(|rest| match rest.strip_prefix('"') {
                        // A quoted value is read whole: titles contain spaces.
                        Some(quoted) => quoted.split('"').next().unwrap_or_default().to_string(),
                        None => rest
                            .split_whitespace()
                            .next()
                            .unwrap_or_default()
                            .to_string(),
                    })
            };
            out.push(Section {
                file: path.file_name().unwrap().to_string_lossy().into_owned(),
                title: attr("title").unwrap_or_default(),
                badge: attr("badge").unwrap_or_default(),
                open: attr("open"),
            });
        }
    }
    out
}

#[test]
fn the_form_has_the_sections_this_guard_expects() {
    let all = sections();
    assert!(all.len() >= 10, "found {} sections", all.len());
    assert!(
        all.iter().any(|s| s.badge.ends_with("Required")),
        "no Required section found: the parser is blind"
    );
}

#[test]
fn optional_sections_start_collapsed_on_create() {
    for s in sections() {
        if s.badge.ends_with("Required") || s.title == "Deposit Details" {
            continue;
        }
        let ok = matches!(s.open.as_deref(), Some("!is_create") | Some("false"));
        assert!(
            ok,
            "{} / {:?} ({}) must start collapsed on create: open=!is_create",
            s.file, s.title, s.badge
        );
    }
}

#[test]
fn required_sections_and_deposit_details_start_open() {
    for s in sections() {
        if s.badge.ends_with("Required") || s.title == "Deposit Details" {
            assert!(
                s.open.is_none() || s.open.as_deref() == Some("true"),
                "{} / {:?} must start open",
                s.file,
                s.title
            );
        }
    }
}

#[test]
fn the_save_row_is_sticky() {
    let css =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("styles/style-09-core.css"))
            .expect("css");
    let at = css
        .find(".card > .form-actions-row")
        .expect("sticky save row rule");
    assert!(
        css[at..]
            .split('}')
            .next()
            .unwrap()
            .contains("position: sticky")
    );
}

/// `overflow-x: clip` (which lets the sticky save row stick) drops the 0
/// automatic minimum width `hidden` gave the grid item, so `<main>` grew to its
/// widest child (722 px on a 390 px phone, e2e notifications.spec). The rule
/// must keep `min-width: 0` for as long as it says `clip`.
#[test]
fn clipping_the_admin_content_keeps_it_shrinkable() {
    let css = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("styles/style-05-admin.staff.css"),
    )
    .expect("css");
    let at = css.find(".admin-content {").expect(".admin-content rule");
    let rule = css[at..].split('}').next().unwrap();
    if rule.contains("overflow-x: clip") {
        assert!(
            rule.contains("min-width: 0"),
            ".admin-content clips without min-width: 0, so wide children widen the page"
        );
    }
}
