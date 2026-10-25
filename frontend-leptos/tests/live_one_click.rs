//! One-click registration on the live form (.plans/047 Phase 2): the owner
//! rule, and the form keeps one bot-check slot and one submit path.

use event_checkin_frontend::pages::public_event::types::{OneClickInputs, one_click_eligible};

fn free_event<'a>() -> OneClickInputs<'a> {
    OneClickInputs {
        has_deposit: false,
        is_hybrid: false,
        wallet_only: false,
        require_photo_consent: false,
        require_contact: false,
        has_required_custom_field: false,
        name: "Nok",
        contact_channel: "",
        contact_handle: "",
    }
}

#[test]
fn a_free_single_track_event_with_a_name_on_file_is_one_click() {
    assert!(one_click_eligible(&free_event()));
}

#[test]
fn anything_the_profile_cannot_answer_means_the_form() {
    let cases: [fn(&mut OneClickInputs); 5] = [
        |i| i.has_deposit = true,
        |i| i.is_hybrid = true,
        |i| i.wallet_only = true,
        |i| i.require_photo_consent = true,
        |i| i.has_required_custom_field = true,
    ];
    for set in cases {
        let mut i = free_event();
        set(&mut i);
        assert!(!one_click_eligible(&i));
    }
}

#[test]
fn no_name_on_file_means_the_form() {
    let mut i = free_event();
    i.name = "  ";
    assert!(!one_click_eligible(&i));
}

#[test]
fn a_required_contact_must_be_on_file() {
    let mut i = free_event();
    i.require_contact = true;
    assert!(!one_click_eligible(&i));
    i.contact_channel = "Telegram";
    assert!(!one_click_eligible(&i));
    i.contact_handle = "@nok";
    assert!(one_click_eligible(&i));
}

const FORM: &str = include_str!("../src/pages/public_event/registration_form.rs");

#[test]
fn one_bot_check_slot_and_one_submit_path() {
    // The widget renders once into its slot; a second slot would stay empty.
    assert_eq!(FORM.matches("<crate::bot_check::BotCheckSlot").count(), 1);
    // Both buttons call the same closure, so validation and the request body
    // cannot drift apart.
    assert_eq!(FORM.matches("RegisterBody {").count(), 1);
    assert!(FORM.matches("submit()").count() >= 2);
}
