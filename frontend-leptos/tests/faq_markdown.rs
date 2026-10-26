//! The FAQ Markdown served for AI readers (`/faq.md`, `/faq.th.md`,
//! .plans/049 item 4) is generated from the same catalog as the `/faq` page.
//! Regenerate after a copy change:
//! `BETHERE_UPDATE_DOCS=1 cargo test --test faq_markdown`.

use event_checkin_frontend::i18n::Locale;
use event_checkin_frontend::pages::faq::faq_markdown;

const FILES: [(Locale, &str); 2] = [
    (Locale::en, "share/docs/faq.md"),
    (Locale::th, "share/docs/faq.th.md"),
];

fn path(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

#[test]
fn the_committed_faq_markdown_matches_the_catalog() {
    let update = std::env::var("BETHERE_UPDATE_DOCS").is_ok_and(|v| v == "1");
    for (locale, rel) in FILES {
        let want = faq_markdown(locale);
        if update {
            std::fs::write(path(rel), &want).expect("write faq markdown");
            continue;
        }
        let have = std::fs::read_to_string(path(rel)).unwrap_or_default();
        assert_eq!(
            have, want,
            "{rel} drifted from the FAQ catalog; run BETHERE_UPDATE_DOCS=1 cargo test --test faq_markdown"
        );
    }
}

#[test]
fn the_markdown_carries_every_question_and_no_template_holes() {
    for (locale, _) in FILES {
        let md = faq_markdown(locale);
        assert_eq!(
            md.matches("\n### ").count(),
            13,
            "7 + deposit + privacy, then 3 + settle"
        );
        assert!(
            !md.contains("{{"),
            "an unfilled placeholder reached the Markdown"
        );
        assert!(md.contains("](/privacy)"));
    }
}

#[test]
fn the_attendee_shell_ships_both_files() {
    let index = std::fs::read_to_string(path("index.html")).expect("index.html");
    for (_, rel) in FILES {
        assert!(
            index.contains(&format!(
                "<link data-trunk rel=\"copy-file\" href=\"{rel}\" />"
            )),
            "index.html must copy {rel} into dist"
        );
    }
}

#[test]
fn the_markdown_is_served_as_utf8() {
    let headers = std::fs::read_to_string(path("_headers")).expect("_headers");
    let rule = headers
        .split("\n/faq*.md\n")
        .nth(1)
        .expect("_headers has a /faq*.md rule");
    let block = rule.split("\n\n").next().unwrap_or_default();
    assert!(block.contains("Content-Type: text/markdown; charset=utf-8"));
}
