//! Sponsor placements (.plans/043 L6): LIVE only for what ships, every
//! category has somewhere to go, and the copy names no prices.

use event_checkin_frontend::pages::landing::sponsors::{Category, Placement};

#[test]
fn live_only_for_what_ships() {
    let live: Vec<_> = Placement::ALL.into_iter().filter(|p| p.is_live()).collect();
    assert_eq!(
        live,
        [
            Placement::EventPage,
            Placement::Poster,
            Placement::GroupPhoto
        ]
    );
}

#[test]
fn every_category_has_a_placement() {
    for cat in [
        Category::Venue,
        Category::Food,
        Category::Stream,
        Category::Ecosystem,
    ] {
        assert!(Placement::ALL.into_iter().any(|p| p.suits(cat)), "{cat:?}");
    }
    assert!(Placement::ALL.into_iter().all(|p| p.suits(Category::All)));
}

#[test]
fn sponsor_copy_names_no_price() {
    let root = env!("CARGO_MANIFEST_DIR");
    for lang in ["en", "th"] {
        let text = std::fs::read_to_string(format!("{root}/locales/{lang}/landing.json")).unwrap();
        let catalog: serde_json::Value = serde_json::from_str(&text).unwrap();
        let all = catalog["sponsors"].to_string();
        for money in ["$", "฿", "USD", "THB", "บาท"] {
            assert!(!all.contains(money), "{lang} sponsors copy has {money}");
        }
    }
}
