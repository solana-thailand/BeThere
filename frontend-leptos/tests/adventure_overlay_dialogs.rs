//! Plan 037 §6: every adventure overlay card is announced as a dialog.
//!
//! The overlays (level select, intro, level complete, NPC/sign dialog,
//! puzzle) cover the board, so a screen reader must hear them as modal
//! dialogs with a name. The game keeps its own key handling, so this only
//! pins the ARIA attributes, not focus management.

const PAGE: &str = include_str!("../src/pages/adventure/page.rs");

const CARDS: [&str; 5] = [
    "adventure-level-select-card",
    "adventure-intro-card",
    "adventure-overlay-card-success",
    "adventure-dialog-card",
    "adventure-puzzle-card",
];

fn open_tag_of(card: &str) -> &'static str {
    let at = PAGE
        .find(&format!("{card}\""))
        .unwrap_or_else(|| panic!("{card} is gone from the adventure page"));
    let start = PAGE[..at].rfind('<').expect("open tag start");
    let end = at + PAGE[at..].find('>').expect("open tag end");
    &PAGE[start..=end]
}

#[test]
fn every_overlay_card_is_a_modal_dialog() {
    for card in CARDS {
        let tag = open_tag_of(card);
        assert!(tag.contains("role=\"dialog\""), "{card}: {tag}");
        assert!(tag.contains("aria-modal=\"true\""), "{card}: {tag}");
    }
}

#[test]
fn every_overlay_card_has_a_name() {
    for card in CARDS {
        let tag = open_tag_of(card);
        assert!(tag.contains("aria-label="), "{card}: {tag}");
    }
}
