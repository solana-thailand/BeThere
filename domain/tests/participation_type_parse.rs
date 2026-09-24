//! `ParticipationType::parse` is allocation-free (no `to_lowercase`). This
//! pins it to the previous `to_lowercase()` implementation, kept here as the
//! reference, over the production spellings and the Unicode case-mapping edges.

use event_checkin_domain::models::attendee::ParticipationType;

/// The implementation before 2026-09-24, verbatim apart from the name.
fn reference(s: &str) -> ParticipationType {
    let lower = s.trim().to_lowercase();
    if lower.is_empty() {
        return ParticipationType::InPerson;
    }
    if lower.contains("in-person")
        || lower.contains("in person")
        || lower.contains("in_person")
        || lower.contains("physical")
    {
        return ParticipationType::InPerson;
    }
    if lower.contains("online") || lower.contains("virtual") {
        return ParticipationType::Online;
    }
    if lower == "retrospective" {
        return ParticipationType::Retrospective;
    }
    ParticipationType::Other
}

const CORPUS: &[&str] = &[
    "",
    "   ",
    "\u{3000}",
    "In-Person",
    "IN-PERSON",
    "in person",
    "In_Person",
    "In-Person (Physical Attendance)",
    "Physical",
    "PHYSICAL only",
    "Online",
    "ONLINE (Zoom)",
    "Virtual",
    "vIrTuAl booth",
    "Online or In-Person",
    "Retrospective",
    " RETROSPECTIVE ",
    "retrospective learner",
    "Hybrid",
    "เข้าร่วมออนไลน์",
    "เข้าร่วมที่งาน (in-person)",
    // U+0130 lowercases to "i\u{307}", which breaks "in-person" either way.
    "\u{130}n-person",
    // Kelvin sign lowercases to ASCII 'k'; no needle contains 'k'.
    "\u{212A}online",
    "onl\u{131}ne",
    "ＯＮＬＩＮＥ",
    "on-line",
    "inperson",
    "in-perso",
    "physica",
    "virtua",
    "retrospectiv",
];

#[test]
fn matches_the_to_lowercase_reference_on_the_corpus() {
    for &raw in CORPUS {
        assert_eq!(
            ParticipationType::parse(raw),
            reference(raw),
            "input {raw:?}"
        );
    }
}

#[test]
fn matches_the_reference_on_every_case_variant_of_each_needle() {
    for needle in [
        "in-person",
        "in person",
        "in_person",
        "physical",
        "online",
        "virtual",
        "retrospective",
    ] {
        let bytes = needle.as_bytes();
        for mask in 0u32..(1 << bytes.len().min(13)) {
            let variant: String = bytes
                .iter()
                .enumerate()
                .map(|(i, &b)| match (mask >> i) & 1 {
                    1 => b.to_ascii_uppercase() as char,
                    _ => b as char,
                })
                .collect();
            for wrapped in [
                variant.clone(),
                format!(" x {variant} y "),
                format!("{variant}!"),
            ] {
                assert_eq!(
                    ParticipationType::parse(&wrapped),
                    reference(&wrapped),
                    "input {wrapped:?}"
                );
            }
        }
    }
}

#[test]
fn production_spellings_keep_their_meaning() {
    assert_eq!(
        ParticipationType::parse("In-Person (Physical Attendance)"),
        ParticipationType::InPerson
    );
    assert_eq!(
        ParticipationType::parse("Online"),
        ParticipationType::Online
    );
    assert_eq!(ParticipationType::parse(""), ParticipationType::InPerson);
    assert_eq!(
        ParticipationType::parse(" Retrospective "),
        ParticipationType::Retrospective
    );
    assert_eq!(ParticipationType::parse("Hybrid"), ParticipationType::Other);
}
