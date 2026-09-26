//! The slip agent's deterministic checker (`.plans/033` W1): the part that
//! decides, so the part that must not accept what it cannot see.

use chrono::{DateTime, TimeZone, Utc};
use event_checkin_domain::slip_proposal::{
    Check, Expectations, Outcome, SlipFacts, Verdict, evaluate, parse_thb_satang, verdict_of,
};

fn at(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 1, hour, 0, 0).unwrap()
}

fn expectations() -> Expectations {
    Expectations {
        deposit_amount_thb: 500,
        window_start: Some(at(8)),
        window_end: at(12),
        ref_claimed_elsewhere: false,
        promptpay_id: "081-234-5678".to_string(),
    }
}

fn full_facts() -> SlipFacts {
    SlipFacts {
        bank_ref: Some("014:0002123123121200011".to_string()),
        amount_satang: Some(50_000),
        transferred_at: Some(at(10)),
        receiver_account: Some("xxx-xxx-5678".to_string()),
    }
}

#[test]
fn a_slip_that_matches_everything_is_accepted() {
    let eval = evaluate(&full_facts(), &expectations());
    assert_eq!(eval.verdict, Verdict::Accepted);
    for check in Check::ALL {
        assert_eq!(eval.outcome(check), Outcome::Pass, "{check:?}");
    }
}

/// The QR carries only the reference. A QR-only slip must go to a human, or
/// the checker would accept a slip for any amount.
#[test]
fn a_qr_only_slip_needs_review_never_accepted() {
    let facts = SlipFacts {
        bank_ref: Some("002:1".to_string()),
        ..SlipFacts::default()
    };
    let eval = evaluate(&facts, &expectations());
    assert_eq!(eval.outcome(Check::RefNew), Outcome::Pass);
    assert_eq!(eval.outcome(Check::Amount), Outcome::Unknown);
    assert_eq!(eval.verdict, Verdict::NeedsReview);
}

#[test]
fn the_doctored_slips_are_rejected() {
    let wrong_amount = SlipFacts {
        amount_satang: Some(5_000),
        ..full_facts()
    };
    let early = SlipFacts {
        transferred_at: Some(at(7)),
        ..full_facts()
    };
    let after_upload = SlipFacts {
        transferred_at: Some(at(13)),
        ..full_facts()
    };
    let other_receiver = SlipFacts {
        receiver_account: Some("xxx-xxx-9999".to_string()),
        ..full_facts()
    };
    for (facts, check) in [
        (wrong_amount, Check::Amount),
        (early, Check::InWindow),
        (after_upload, Check::InWindow),
        (other_receiver, Check::Receiver),
    ] {
        let eval = evaluate(&facts, &expectations());
        assert_eq!(eval.outcome(check), Outcome::Fail, "{check:?}");
        assert_eq!(eval.verdict, Verdict::Rejected, "{check:?}");
    }
}

#[test]
fn a_reused_reference_is_rejected() {
    let expect = Expectations {
        ref_claimed_elsewhere: true,
        ..expectations()
    };
    let eval = evaluate(&full_facts(), &expect);
    assert_eq!(eval.outcome(Check::RefNew), Outcome::Fail);
    assert_eq!(eval.verdict, Verdict::Rejected);
}

/// With no reference there is nothing to compare, so "claimed elsewhere" can't
/// be answered; that is unknown, not a pass.
#[test]
fn no_reference_is_unknown_even_if_nothing_is_claimed() {
    let facts = SlipFacts {
        bank_ref: None,
        ..full_facts()
    };
    let eval = evaluate(&facts, &expectations());
    assert_eq!(eval.outcome(Check::RefNew), Outcome::Unknown);
    assert_eq!(eval.verdict, Verdict::NeedsReview);
}

#[test]
fn a_receiver_tail_too_short_or_oddly_masked_is_unknown() {
    for printed in ["xxx-xxx-x78", "XXX-X-X1234-X", "", "xxxxxxxx"] {
        let facts = SlipFacts {
            receiver_account: Some(printed.to_string()),
            ..full_facts()
        };
        let eval = evaluate(&facts, &expectations());
        assert_eq!(
            eval.outcome(Check::Receiver),
            Outcome::Unknown,
            "{printed:?}"
        );
    }
}

#[test]
fn a_fail_outranks_an_unknown() {
    assert_eq!(
        verdict_of([Outcome::Unknown, Outcome::Fail, Outcome::Pass]),
        Verdict::Rejected
    );
    assert_eq!(
        verdict_of([Outcome::Pass, Outcome::Unknown]),
        Verdict::NeedsReview
    );
    assert_eq!(verdict_of([]), Verdict::Accepted);
}

#[test]
fn slip_amounts_parse_exactly() {
    assert_eq!(parse_thb_satang("500"), Some(50_000));
    assert_eq!(parse_thb_satang("500.00"), Some(50_000));
    assert_eq!(parse_thb_satang("1,500.5"), Some(150_050));
    assert_eq!(parse_thb_satang("฿500.00"), Some(50_000));
    assert_eq!(parse_thb_satang("500.00 บาท"), Some(50_000));
    assert_eq!(parse_thb_satang(" 500.01 THB "), Some(50_001));
}

#[test]
fn vague_or_malformed_amounts_do_not_become_numbers() {
    for bad in [
        "",
        "about 500",
        "-500",
        "500.001",
        "5e2",
        ".50",
        "500..0",
        "๕๐๐",
    ] {
        assert_eq!(parse_thb_satang(bad), None, "{bad:?}");
    }
}
