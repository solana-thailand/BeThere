//! The refund card names the day the deposit was returned (.plans/048).

use event_checkin_frontend::api::DepositInfo;
use event_checkin_frontend::pages::ticket::action_cards::refund_date_line;

#[test]
fn an_rfc3339_time_shows_its_day() {
    assert_eq!(
        refund_date_line("2026-05-25T09:30:00+00:00").as_deref(),
        Some("2026-05-25")
    );
}

#[test]
fn empty_or_garbled_means_no_date_line() {
    for bad in [
        "",
        "soon",
        "25/05/2026 x",
        "2026-5-25xxxx",
        "ภาษาไทยภาษาไทย",
    ] {
        assert_eq!(refund_date_line(bad), None, "{bad:?}");
    }
}

#[test]
fn an_older_worker_without_the_field_still_parses() {
    let d: DepositInfo = serde_json::from_str(
        r#"{"method":"thb","verified":true,"currency":"THB","refunded":true}"#,
    )
    .expect("parse");
    assert_eq!(d.refunded_at, None);
}
