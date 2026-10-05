//! Refund queue: the "Checked in" filter and row badge.

use event_checkin_domain::models::attendee::ParticipationType;
use event_checkin_domain::models::deposit::RefundQueueContext;
use event_checkin_frontend::pages::admin_refund_queue_filter::RefundQueueFilter;

fn ctx(checked_in: bool) -> RefundQueueContext {
    RefundQueueContext {
        participation_type: ParticipationType::InPerson,
        checked_in,
        attendance_answer: None,
    }
}

#[test]
fn checked_in_filter_keeps_only_people_who_came() {
    assert!(RefundQueueFilter::CheckedIn.matches(Some(&ctx(true))));
    assert!(!RefundQueueFilter::CheckedIn.matches(Some(&ctx(false))));
    assert!(RefundQueueFilter::NotCheckedIn.matches(Some(&ctx(false))));
}

#[test]
fn unknown_context_never_counts_as_checked_in() {
    assert!(!RefundQueueFilter::CheckedIn.matches(None));
    assert!(!RefundQueueFilter::row_checked_in(None));
    assert!(RefundQueueFilter::row_checked_in(Some(&ctx(true))));
}
