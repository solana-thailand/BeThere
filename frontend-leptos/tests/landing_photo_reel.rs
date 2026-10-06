//! The photo reel (.plans/043 L9): served only through the worker's list
//! route, the take-down line in both languages, and an anchor it lands on.

use event_checkin_frontend::pages::landing::photos::photo_url;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{ROOT}/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn photos_come_from_the_listed_route() {
    assert_eq!(
        photo_url("m3-s.jpg"),
        "/api/storage/landing-photos/m3-s.jpg"
    );
    // No image files in the frontend: they live in R2, never in git.
    let reel = read("src/pages/landing/photos.rs");
    assert!(!reel.contains(".jpg\""), "no photo is named in the code");
    assert!(reel.contains("loading=\"lazy\""));
}

#[test]
fn take_down_line_in_both_languages_lands_on_the_contact() {
    for (lang, words) in [("en", "removed"), ("th", "เอาออก")] {
        let catalog: serde_json::Value =
            serde_json::from_str(&read(&format!("locales/{lang}/landing.json"))).unwrap();
        let line = catalog["sofar"]["photo_remove"]
            .as_str()
            .unwrap_or_default();
        assert!(line.contains(words), "{lang}: {line}");
        assert!(
            !catalog["sofar"]["photo_remove_link"]
                .as_str()
                .unwrap_or_default()
                .is_empty()
        );
    }
    assert!(read("src/pages/landing/photos.rs").contains("href=\"#contact\""));
    assert!(read("src/pages/landing/sponsors.rs").contains("id=\"contact\""));
}
