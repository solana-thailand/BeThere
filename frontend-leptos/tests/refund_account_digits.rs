//! The refund queue copies the account number as digits only.

use event_checkin_frontend::pages::admin_deposit_bank_info::account_digits;

#[test]
fn dashes_and_spaces_are_dropped() {
    assert_eq!(account_digits("123-4-56789-0"), "1234567890");
    assert_eq!(account_digits(" 123 456 7890 "), "1234567890");
}

#[test]
fn no_digits_copies_nothing() {
    assert_eq!(account_digits("n/a"), "");
}

#[test]
fn the_note_names_the_event() {
    use event_checkin_frontend::pages::admin_deposit_bank_info::refund_note;
    assert_eq!(
        refund_note("Solana x AI Builder #6"),
        "คืนค่างาน Solana x AI Builder #6"
    );
    assert_eq!(refund_note("  "), "คืนค่ามัดจำ");
}

#[test]
fn the_note_fits_the_bank_apps_40_char_limit() {
    use event_checkin_frontend::pages::admin_deposit_bank_info::{
        MAX_REFUND_NOTE_CHARS, refund_note,
    };
    let note = refund_note("Solana Thailand x AI Builder Night Bangkok Edition #6");
    assert!(note.chars().count() <= MAX_REFUND_NOTE_CHARS, "{note}");
    assert!(note.starts_with("คืนค่างาน Solana"));
    assert!(
        note.ends_with(" #6"),
        "the edition survives the cut: {note}"
    );
    assert!(!note.ends_with(' '));
}

#[test]
fn a_cut_never_strands_a_thai_mark() {
    use event_checkin_frontend::pages::admin_deposit_bank_info::fit_chars;
    // "คืน" is ค + ื + น: a cut after 1 char would split ค from its vowel.
    assert_eq!(fit_chars("คืน", 1), "");
    assert_eq!(fit_chars("คืน", 2), "คื");
    assert_eq!(fit_chars("abc", 2), "ab");
    assert_eq!(fit_chars("short", 40), "short");
}

#[test]
fn series_names_keep_their_edition() {
    use event_checkin_frontend::pages::admin_deposit_bank_info::{refund_note, short_event_name};
    assert_eq!(
        short_event_name("Solana x AI Builders: The Road to Mainnet #6 (Bangkok)"),
        "Solana x AI Builders #6"
    );
    assert_eq!(
        refund_note("Solana x AI Builders: The Road to Mainnet #5 (Bangkok)"),
        "คืนค่างาน Solana x AI Builders #5"
    );
    assert_eq!(
        short_event_name("Solana in Latent Space Part 7"),
        "Solana in Latent Space Part 7"
    );
    assert_eq!(short_event_name("IslandDAO V4 Demo"), "IslandDAO V4 Demo");
    assert_eq!(short_event_name("Meetup: Night ครั้งที่ 3"), "Meetup ครั้งที่ 3");
}
