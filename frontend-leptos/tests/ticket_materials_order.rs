//! Ticket order (.plans/047): what the person can still do (the badge claim),
//! then the recording and slides, then the history. Once an event is over the
//! online ticket's timeline is history, so it moves below the materials.

use event_checkin_frontend::pages::ticket::materials::TimelineSlot;

const ONLINE: &str = include_str!("../src/pages/ticket/online_view.rs");
const IN_PERSON: &str = include_str!("../src/pages/ticket/in_person_view.rs");

#[test]
fn an_ended_event_turns_the_timeline_into_history() {
    assert_eq!(TimelineSlot::for_event(true), TimelineSlot::History);
    assert_eq!(TimelineSlot::for_event(false), TimelineSlot::Steps);
}

fn at(src: &str, needle: &str) -> usize {
    src.find(needle)
        .unwrap_or_else(|| panic!("missing {needle}"))
}

#[test]
fn online_order_is_steps_claim_materials_history() {
    let steps = at(ONLINE, "TimelineSlot::Steps).then_some(timeline)");
    let claim = at(ONLINE, "// 4. NFT section");
    let materials = at(ONLINE, "super::materials::materials(");
    let history = at(ONLINE, "TimelineSlot::History).then_some(timeline_history)");
    assert!(steps < claim && claim < materials && materials < history);
}

#[test]
fn in_person_keeps_the_claim_card_above_the_materials() {
    let card = at(IN_PERSON, "// 2. Main card");
    let materials = at(IN_PERSON, "super::materials::materials(");
    assert!(card < materials, "the claim card leads the main card");
}

#[test]
fn the_recording_and_resources_render_only_through_the_block() {
    for (name, src) in [("online", ONLINE), ("in_person", IN_PERSON)] {
        assert!(
            !src.contains("<VideoSection") && !src.contains("resources_section("),
            "{name}: render the recording and resources through materials()"
        );
        assert_eq!(
            src.matches("super::materials::materials(").count(),
            1,
            "{name}"
        );
    }
}
