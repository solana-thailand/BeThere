//! The home page after the "C" redesign, made lean (.plans/049, owner picks):
//! the chairs are the picture, one sentence names them, the one call to action
//! is the waitlist (organizer access is by invitation), and nothing else
//! competes: the numbers live on the pages that ask for them.

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
fn the_home_page_is_hero_and_photos_only() {
    let page = read("src/pages/landing/page.rs");
    for kept in ["<Hero ", "<MyRegistrations", "<PhotoReel"] {
        assert!(page.contains(kept), "home lost {kept}");
    }
    for gone in ["<Doors", "<Proof", "<SoFar"] {
        assert!(!page.contains(gone), "home still has {gone}");
    }
}

#[test]
fn the_running_totals_moved_to_the_sponsors_page() {
    // "A headcount you can check" is the sponsors page's promise; the strip,
    // with its meals line, is drawn there and no longer on the home page.
    let sponsors = read("src/pages/site/sponsors.rs");
    assert!(sponsors.contains("<SoFar"));
    // The medians are already on the organizers and events pages.
    assert!(read("src/pages/landing/how.rs").contains("duration_label"));
    assert!(read("src/pages/site/deposit_walk.rs").contains("duration_label"));
}

#[test]
fn the_new_copy_exists_in_both_languages_and_promises_nothing() {
    for lang in ["en", "th"] {
        let c = catalog(lang);
        for (section, key) in [("hero", "c_lead"), ("hero", "b_events_link")] {
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
fn the_hero_sentence_takes_its_numbers_from_the_hall() {
    // Owner pick C: the chairs are the picture and one sentence names them. The
    // numbers are the hall's own (`ladder_total`), never typed in.
    let hero = read("src/pages/landing/hero.rs");
    assert!(hero.contains("ladder_total()"));
    assert!(hero.contains(r#"("came", &came.to_string())"#));
    assert!(hero.contains(r#"("gone", &paid.saturating_sub(came).to_string())"#));
    for lang in ["en", "th"] {
        let lead = catalog(lang)["hero"]["c_lead"]
            .as_str()
            .expect("hero.c_lead")
            .to_string();
        for hole in ["{came}", "{paid}", "{gone}"] {
            assert!(lead.contains(hole), "{lang}: c_lead lost {hole}");
        }
        assert!(
            !lead.chars().any(|c| c.is_ascii_digit()),
            "{lang}: c_lead must not carry a typed number"
        );
    }
}
