//! The live-event registration form prefills from the server profile
//! (.plans/047 Phase 2), so a returning attendee on a new browser is not
//! asked for their name again. Source guards: the behaviour needs a browser.

const FORM: &str = include_str!("../src/pages/public_event/registration_form.rs");

#[test]
fn the_form_reads_the_server_profile() {
    assert!(FORM.contains("get_my_profile()"));
    assert!(FORM.contains("registration_name()"));
}

#[test]
fn wallet_only_sessions_skip_the_lookup() {
    assert!(FORM.contains("if !wallet_only {\n        leptos::task::spawn_local"));
}

#[test]
fn typed_or_local_values_are_never_overwritten() {
    assert!(FORM.contains("reg_name.get_untracked().is_empty()"));
    assert!(FORM.contains("reg_contact_channel.get_untracked().is_empty()"));
    assert!(FORM.contains("reg_contact_handle.get_untracked().is_empty()"));
}
