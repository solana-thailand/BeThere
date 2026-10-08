//! The short booking display code (`.issues/178`): alphabet, generation and
//! the normalisation of what staff type at the door.

use event_checkin_domain::models::attendee::{
    DISPLAY_CODE_ALPHABET, DISPLAY_CODE_LEN, DISPLAY_CODE_RANDOM_BYTES, DisplayCode,
    DisplayCodeError,
};

#[test]
fn the_alphabet_has_no_look_alikes() {
    for banned in *b"0O1IL" {
        assert!(
            !DISPLAY_CODE_ALPHABET.contains(&banned),
            "{} must not be in the alphabet",
            banned as char
        );
    }
    let mut sorted = DISPLAY_CODE_ALPHABET.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), DISPLAY_CODE_ALPHABET.len(), "no duplicates");
    assert_eq!(DISPLAY_CODE_ALPHABET.len(), 31);
    assert_eq!(DISPLAY_CODE_LEN, 6);
}

#[test]
fn random_bytes_map_into_the_alphabet() {
    let bytes: Vec<u8> = (0u8..DISPLAY_CODE_RANDOM_BYTES as u8).collect();
    let code = DisplayCode::from_random_bytes(&bytes).expect("16 low bytes are all usable");
    assert_eq!(code.as_str(), "234567");
    assert!(
        code.as_str()
            .bytes()
            .all(|b| DISPLAY_CODE_ALPHABET.contains(&b))
    );
}

#[test]
fn biased_bytes_are_rejected_not_folded() {
    // 248..=255 would make the first 8 symbols more likely if taken mod 31.
    let mut bytes = vec![255u8, 248, 250];
    bytes.extend([30u8, 30, 30, 30, 30, 30]);
    let code = DisplayCode::from_random_bytes(&bytes).expect("six usable bytes");
    assert_eq!(code.as_str(), "ZZZZZZ");
}

#[test]
fn too_few_usable_bytes_is_none_so_the_caller_draws_again() {
    assert_eq!(DisplayCode::from_random_bytes(&[255u8; 16]), None);
    assert_eq!(DisplayCode::from_random_bytes(&[1, 2, 3]), None);
    assert_eq!(DisplayCode::from_random_bytes(&[]), None);
}

#[test]
fn every_symbol_is_reachable_and_uniform() {
    // Each usable byte value 0..248 lands on symbol `b % 31` exactly 8 times.
    let mut counts = [0u32; 31];
    for b in 0u8..248 {
        let code = DisplayCode::from_random_bytes(&[b; 6]).expect("usable");
        let sym = code.as_str().as_bytes()[0];
        let idx = DISPLAY_CODE_ALPHABET
            .iter()
            .position(|&a| a == sym)
            .unwrap();
        counts[idx] += 1;
    }
    assert!(counts.iter().all(|&c| c == 8), "{counts:?}");
}

#[test]
fn typed_input_is_normalised() {
    let want = DisplayCode::parse("7KQ2XM").unwrap();
    for typed in [
        "7kq2xm",
        " 7KQ-2XM ",
        "7KQ 2XM",
        "Nº 7KQ2XM",
        "nº 7kq-2xm",
        "№7kq-2xm",
        "7-K-Q-2-X-M",
    ] {
        assert_eq!(DisplayCode::parse(typed), Ok(want), "{typed:?}");
    }
    assert_eq!(want.to_string(), "7KQ2XM");
}

#[test]
fn ambiguous_characters_are_rejected_not_guessed() {
    assert_eq!(
        DisplayCode::parse("7KQ2X0"),
        Err(DisplayCodeError::Character('0'))
    );
    assert_eq!(
        DisplayCode::parse("7KQ2Xo"),
        Err(DisplayCodeError::Character('o'))
    );
    assert_eq!(
        DisplayCode::parse("7KQ2X1"),
        Err(DisplayCodeError::Character('1'))
    );
    assert_eq!(
        DisplayCode::parse("7KQ2Xl"),
        Err(DisplayCodeError::Character('l'))
    );
    assert_eq!(
        DisplayCode::parse("7KQ2Xi"),
        Err(DisplayCodeError::Character('i'))
    );
    assert_eq!(
        DisplayCode::parse("7KQ2X%"),
        Err(DisplayCodeError::Character('%'))
    );
}

#[test]
fn wrong_lengths_are_rejected() {
    assert_eq!(DisplayCode::parse(""), Err(DisplayCodeError::Length(0)));
    assert_eq!(
        DisplayCode::parse("7KQ2X"),
        Err(DisplayCodeError::Length(5))
    );
    assert_eq!(
        DisplayCode::parse("7KQ2XMM"),
        Err(DisplayCodeError::Length(7))
    );
    // An attendee id typed into the same box is not mistaken for a code.
    assert!(DisplayCode::parse("gst-abc123").is_err());
    assert!(DisplayCode::parse("0192f0c4-6a3e-7b1d-9e2f-5c8a1b3d4e6f").is_err());
}

#[test]
fn the_error_text_names_no_url_scheme() {
    // The API redactor turns "://" into [redacted-url]; keep messages plain.
    for e in [
        DisplayCodeError::Length(3),
        DisplayCodeError::Character('0'),
    ] {
        assert!(!e.to_string().contains("://"));
    }
}

#[test]
fn serde_round_trips_and_validates() {
    let code = DisplayCode::parse("ABC234").unwrap();
    let json = serde_json::to_string(&code).unwrap();
    assert_eq!(json, "\"ABC234\"");
    assert_eq!(serde_json::from_str::<DisplayCode>(&json).unwrap(), code);
    assert!(serde_json::from_str::<DisplayCode>("\"ABC230\"").is_err());
}
