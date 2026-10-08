//! The commitment ladder (.plans/043 L8): the owner's table under one
//! definition, the total summed from it, and the room's stages.

use event_checkin_frontend::pages::landing::story::{
    LADDER, LadderSource, ROOM, Stage, ladder_total, percent,
};

#[test]
fn table_is_the_owners_and_never_shows_came_over_paid() {
    let rows: Vec<_> = LADDER.iter().map(|r| (r.event, r.paid, r.came)).collect();
    assert_eq!(
        rows,
        [
            ("RTM #1", 16, 16),
            ("RTM #2", 15, 15),
            ("RTM #3", 12, 12),
            ("RTM #4", 16, 14),
            ("RTM #5", 14, 13),
            ("RTM #6", 21, 20),
        ]
    );
    for r in LADDER {
        assert!(r.came <= r.paid, "{}", r.event);
    }
    assert_eq!(ladder_total(), (94, 90));
    assert_eq!(percent(90, 94), 96);
    for r in LADDER {
        let d: Vec<&str> = r.measured_at.split('-').collect();
        assert!(
            d.len() == 3 && d[0] == "2026",
            "{}: {}",
            r.event,
            r.measured_at
        );
    }
}

#[test]
fn every_row_names_where_it_comes_from() {
    let sources: Vec<_> = LADDER.iter().map(|r| r.source).collect();
    assert_eq!(
        sources,
        [
            LadderSource::HandRecord,
            LadderSource::ArchiveAndOwnerRecord,
            LadderSource::Statement,
            LadderSource::System,
            LadderSource::System,
            LadderSource::System,
        ]
    );
}

#[test]
fn the_room_narrows_45_30_16_and_the_share_who_came_climbs() {
    assert_eq!(ROOM.iter().map(|g| g.n).sum::<u32>(), 45);
    let shown: Vec<_> = Stage::ALL
        .iter()
        .map(|s| {
            let (n, came) = s.counts();
            (n, came, percent(came, n))
        })
        .collect();
    assert_eq!(shown, [(45, 25, 56), (30, 25, 83), (16, 16, 100)]);
    assert_eq!(percent(0, 0), 0);
}

/// Content rules: no "never forfeited", "automated", "instantly", no credit
/// exit, and the caveat stays in both languages.
#[test]
fn copy_keeps_the_caveat_and_no_banned_words() {
    let root = env!("CARGO_MANIFEST_DIR");
    for (lang, caveat) in [("en", "anyway"), ("th", "อาจตั้งใจมาอยู่แล้ว")]
    {
        let text = std::fs::read_to_string(format!("{root}/locales/{lang}/landing.json")).unwrap();
        let catalog: serde_json::Value = serde_json::from_str(&text).unwrap();
        let story = catalog["story"].to_string();
        assert!(story.contains(caveat), "{lang}");
        let lower = story.to_lowercase();
        for banned in ["forfeit", "automat", "instant", "can't come", "cannot come"] {
            assert!(!lower.contains(banned), "{lang}: {banned}");
        }
    }
}
