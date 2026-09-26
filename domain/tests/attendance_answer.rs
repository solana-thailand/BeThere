//! `AttendanceAnswer` and the fields that carry it (migration 0052). The
//! wire strings are also the D1 values and the CHECK list in the migration,
//! so they are pinned here.

use event_checkin_domain::models::api::AttendeeListItem;
use event_checkin_domain::models::attendee::{AttendanceAnswer, ParticipationType};
use event_checkin_domain::models::deposit::{RefundQueueContext, RefundQueueResponse};

#[test]
fn wire_strings_match_the_migration_check() {
    let migration = include_str!("../../worker/migrations/0052_attendance_answers.sql");
    for answer in AttendanceAnswer::ALL {
        let quoted = format!("'{}'", answer.as_str());
        assert!(
            migration.contains(&quoted),
            "{quoted} missing from the CHECK"
        );
        let json = serde_json::to_string(&answer).expect("serialize");
        assert_eq!(json, format!("\"{}\"", answer.as_str()));
    }
}

#[test]
fn parse_is_the_strict_inverse_of_as_str() {
    for answer in AttendanceAnswer::ALL {
        assert_eq!(AttendanceAnswer::parse(answer.as_str()), Some(answer));
    }
    assert_eq!(
        AttendanceAnswer::parse(" coming "),
        Some(AttendanceAnswer::Coming)
    );
    assert_eq!(AttendanceAnswer::parse("Coming"), None);
    assert_eq!(AttendanceAnswer::parse("maybe"), None);
    assert_eq!(AttendanceAnswer::parse(""), None);
}

#[test]
fn roster_item_omits_a_missing_answer_and_reads_old_payloads() {
    let item: AttendeeListItem = serde_json::from_value(serde_json::json!({
        "api_id": "a", "name": "n", "email": "e", "ticket_name": "", "approval_status": "approved",
        "checked_in_at": null, "checked_in_by": null, "qr_code_url": null,
        "participation_type": "in_person", "row_index": 0
    }))
    .expect("an old payload without the field still parses");
    assert_eq!(item.attendance_answer, None);
    let json = serde_json::to_value(&item).expect("serialize");
    assert!(json.get("attendance_answer").is_none());
}

#[test]
fn refund_queue_context_defaults_when_absent() {
    let old: RefundQueueResponse =
        serde_json::from_str(r#"{"pending": []}"#).expect("old response parses");
    assert!(old.context.is_empty());

    let ctx: RefundQueueContext = serde_json::from_str(
        r#"{"participation_type": "online", "checked_in": false, "attendance_answer": "not_coming"}"#,
    )
    .expect("context parses");
    assert_eq!(ctx.participation_type, ParticipationType::Online);
    assert_eq!(ctx.attendance_answer, Some(AttendanceAnswer::NotComing));
}
