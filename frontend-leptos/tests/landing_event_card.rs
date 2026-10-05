//! The landing event card (.plans/043 L3): the amount is the event's own,
//! no credit exit, and the nearest event leads.

use event_checkin_frontend::pages::landing::event_card::{DepositRule, nearest_first};

#[test]
fn deposit_amount_comes_from_the_event() {
    assert_eq!(
        DepositRule::for_event(true, 300, "in_person"),
        Some(DepositRule::BackWhenYouShowUp(300))
    );
    assert_eq!(
        DepositRule::for_event(true, 500, "hybrid"),
        Some(DepositRule::BackWhenYouShowUp(500))
    );
}

#[test]
fn online_events_are_free_whatever_the_flag() {
    assert_eq!(
        DepositRule::for_event(true, 500, "online"),
        Some(DepositRule::OnlineFree)
    );
    assert_eq!(
        DepositRule::for_event(false, 0, "online"),
        Some(DepositRule::OnlineFree)
    );
}

#[test]
fn no_deposit_is_free_and_an_unset_amount_says_nothing() {
    assert_eq!(
        DepositRule::for_event(false, 500, "in_person"),
        Some(DepositRule::Free)
    );
    assert_eq!(DepositRule::for_event(true, 0, "in_person"), None);
}

#[test]
fn nearest_first_puts_tba_last() {
    let mut v = vec![(300, 0), (0, 1), (100, 2), (200, 3), (0, 4)];
    nearest_first(&mut v);
    assert_eq!(
        v.iter().map(|&(_, i)| i).collect::<Vec<_>>(),
        vec![2, 3, 0, 1, 4]
    );
}

#[test]
fn card_copy_never_offers_credit() {
    let root = env!("CARGO_MANIFEST_DIR");
    for lang in ["en", "th"] {
        let text = std::fs::read_to_string(format!("{root}/locales/{lang}/landing.json")).unwrap();
        let catalog: serde_json::Value = serde_json::from_str(&text).unwrap();
        let upcoming = catalog["upcoming"].as_object().expect("upcoming block");
        for (key, value) in upcoming {
            let s = value.as_str().unwrap_or_default().to_lowercase();
            assert!(
                !s.contains("credit") && !s.contains("เครดิต"),
                "{lang} upcoming.{key}: {s}"
            );
            assert!(
                !s.contains("forfeit") && !s.contains("ริบ"),
                "{lang} upcoming.{key}: {s}"
            );
        }
    }
}
