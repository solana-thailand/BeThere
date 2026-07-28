use super::participation::normalize_override;
use event_checkin_domain::models::attendee::ParticipationType;
use event_checkin_domain::models::error::AppError;

#[test]
fn normalize_override_accepts_display_case() {
    // Frontend currently sends display-case — must keep working.
    assert_eq!(
        normalize_override("In-Person").unwrap(),
        ParticipationType::InPerson
    );
    assert_eq!(
        normalize_override("Online").unwrap(),
        ParticipationType::Online
    );
}

#[test]
fn normalize_override_accepts_canonical() {
    // New canonical form (post-Tier-B) also accepted.
    assert_eq!(
        normalize_override("in_person").unwrap(),
        ParticipationType::InPerson
    );
    assert_eq!(
        normalize_override("online").unwrap(),
        ParticipationType::Online
    );
}

#[test]
fn normalize_override_accepts_case_variants_and_synonyms() {
    assert_eq!(
        normalize_override("IN-PERSON").unwrap(),
        ParticipationType::InPerson
    );
    assert_eq!(
        normalize_override("physical").unwrap(),
        ParticipationType::InPerson
    );
    assert_eq!(
        normalize_override("Virtual").unwrap(),
        ParticipationType::Online
    );
}

#[test]
fn normalize_override_rejects_walkin_sentinel() {
    // `walkin` is a status sentinel, not a participation mode — this endpoint
    // must never set it (would corrupt walk-in detection).
    assert!(matches!(
        normalize_override("walkin"),
        Err(AppError::Validation(_))
    ));
}

#[test]
fn normalize_override_rejects_junk_and_empty() {
    assert!(matches!(
        normalize_override("test"),
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        normalize_override(""),
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        normalize_override("TBD"),
        Err(AppError::Validation(_))
    ));
}
