//! The home page after the "B" redesign (.plans/049, owner pick): the
//! organizer is the reader, the call to action is the waitlist (organizer
//! access is by invitation), and two measured medians sit under the hero.

use std::fs;

fn read(rel: &str) -> String {
    fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).expect(rel)
}

fn catalog(lang: &str) -> serde_json::Value {
    serde_json::from_str(&read(&format!("locales/{lang}/landing.json"))).expect("catalog")
}

#[test]
fn a_visitor_is_offered_the_waitlist_not_hosting() {
    let hero = read("src/pages/landing/hero.rs");
    assert!(hero.contains(r#"href="/organizers#join""#));
    assert!(hero.contains("landing.join.waitlist_cta"));
    // "See open events" led to a page with no open events to register for.
    assert!(!hero.contains("landing.cta.find_events"));
    // Signed-in roles keep their own entry.
    assert!(hero.contains("landing.cta.dashboard") && hero.contains("landing.cta.scanner"));
}

#[test]
fn the_waitlist_target_exists_on_the_organizers_page() {
    let organizers = read("src/pages/site/organizers.rs");
    assert!(organizers.contains(r#"id="join""#));
}

#[test]
fn the_home_page_is_hero_proof_and_so_far_only() {
    let page = read("src/pages/landing/page.rs");
    for kept in ["<Hero ", "<MyRegistrations", "<Proof", "<SoFar"] {
        assert!(page.contains(kept), "home lost {kept}");
    }
    assert!(
        !page.contains("<Doors"),
        "the three doors left the home page"
    );
}

#[test]
fn the_medians_need_both_timings_and_say_when_and_how_many() {
    let proof = read("src/pages/landing/proof.rs");
    assert!(proof.contains("s.slip_check?") && proof.contains("s.refund_after_end?"));
    for key in [
        "landing.proof.fine",
        "landing.proof.slip",
        "landing.proof.refund",
    ] {
        assert!(proof.contains(key), "{key}");
    }
    for lang in ["en", "th"] {
        let fine = catalog(lang)["proof"]["fine"]
            .as_str()
            .expect("proof.fine")
            .to_string();
        for hole in ["{at}", "{slips}", "{refunds}"] {
            assert!(fine.contains(hole), "{lang}: proof.fine lost {hole}");
        }
    }
}

#[test]
fn the_new_copy_exists_in_both_languages_and_promises_nothing() {
    for lang in ["en", "th"] {
        let c = catalog(lang);
        for (section, key) in [
            ("hero", "b_headline_1"),
            ("hero", "b_headline_2"),
            ("hero", "b_lead"),
            ("hero", "b_events_link"),
            ("proof", "slip"),
            ("proof", "refund"),
            ("proof", "fine"),
        ] {
            let text = c[section][key].as_str().unwrap_or_default();
            assert!(!text.is_empty(), "{lang}: landing.{section}.{key}");
            let lower = text.to_lowercase();
            for banned in ["forfeit", "guarantee", "within 24", "ฟรีตลอด"] {
                assert!(!lower.contains(banned), "{lang}: {key} says {banned:?}");
            }
        }
    }
}

#[test]
fn the_headline_is_one_balanced_sentence() {
    let hero = read("src/pages/landing/hero.rs");
    assert!(
        !hero.contains("<br />"),
        "a forced break defeats text-wrap: balance"
    );
    assert!(read("styles/style-23-landing.css").contains("text-wrap: balance"));
}
