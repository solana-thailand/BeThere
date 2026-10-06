//! The landing header and side index (`bethere-ux/landing.html`), and the
//! share/SEO copy that must not promise what the system does not do.

use event_checkin_frontend::locale::shows_lang_bar;
use event_checkin_frontend::pages::landing::header::{HEADER_SECTIONS, SIDE_SECTIONS};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{ROOT}/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Every jump target is an element id the landing renders, so no link or
/// dot lands nowhere.
#[test]
fn every_section_link_has_a_target() {
    let sources: String = std::fs::read_dir(format!("{ROOT}/src/pages/landing"))
        .expect("landing dir")
        .filter_map(|e| std::fs::read_to_string(e.ok()?.path()).ok())
        .collect();
    for s in HEADER_SECTIONS.iter().chain(SIDE_SECTIONS.iter()) {
        assert!(
            sources.contains(&format!("id=\"{}\"", s.id)),
            "no element with id=\"{}\" on the landing",
            s.id
        );
    }
    let ids: Vec<_> = HEADER_SECTIONS.iter().map(|s| s.id).collect();
    assert_eq!(ids, ["how", "goal", "sponsors", "join"]);
}

/// The landing's header carries the switch; the bar above the page stays
/// on the other attendee pages and off staff pages.
#[test]
fn lang_bar_is_off_on_the_landing_only() {
    assert!(!shows_lang_bar("/"));
    assert!(shows_lang_bar("/discover"));
    assert!(shows_lang_bar("/e/rtm-6"));
    assert!(!shows_lang_bar("/admin"));
}

/// Refunds are paid by hand and deposits are THB by PromptPay: the meta
/// descriptions use the landing's own line, the same in every tag.
#[test]
fn share_descriptions_claim_nothing_automated() {
    let html = read("index.html");
    let line = "Free events. Hold your seat with a deposit, show up, get it all back.";
    for tag in [
        "name=\"description\"",
        "property=\"og:description\"",
        "name=\"twitter:description\"",
    ] {
        let at = html.find(tag).unwrap_or_else(|| panic!("{tag} missing"));
        let content = &html[at..at + 200];
        assert!(
            content.contains(line),
            "{tag} does not use the landing line"
        );
    }
    for claim in ["automated", "Attendees commit money"] {
        assert!(!html.contains(claim), "index.html still says {claim:?}");
    }
}

/// Nobody has measured how fast a USDC claim is (and there is no claim tool
/// yet), so neither the hero nor the swimlane says.
#[test]
fn hero_promises_no_speed() {
    for (lang, word) in [("en", "instantly"), ("th", "ทันที")] {
        let catalog: serde_json::Value =
            serde_json::from_str(&read(&format!("locales/{lang}/landing.json"))).expect("json");
        for (section, key) in [("hero", "now"), ("how", "after_end")] {
            let text = catalog[section][key].as_str().unwrap_or_default();
            assert!(!text.is_empty(), "{lang} {section}.{key} missing");
            assert!(!text.contains(word), "{lang} {section}.{key} says {word:?}");
        }
        let right_after = ["right after", "ทันทีหลัง"];
        let after_end = catalog["how"]["after_end"].as_str().unwrap_or_default();
        assert!(
            !right_after.iter().any(|w| after_end.contains(w)),
            "{lang} how.after_end promises speed: {after_end:?}"
        );
    }
}

/// The film button points at files the build ships.
#[test]
fn why_film_files_ship() {
    for name in [
        "why-en.mp4",
        "why-th.mp4",
        "why-en.jpg",
        "why-th.jpg",
        "why-en.vtt",
        "why-th.vtt",
    ] {
        assert!(
            std::path::Path::new(&format!("{ROOT}/media/{name}")).is_file(),
            "media/{name} missing"
        );
    }
    assert!(read("index.html").contains("rel=\"copy-dir\" href=\"media\""));
}
