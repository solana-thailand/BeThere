//! Courses (.plans/045 R4.4) and the `.issues/068` rule: watching a
//! recording is never check-in, attendance, a deposit or badge eligibility.
//! The course code touches only its own two tables, sits behind sign-in,
//! and PDPA erasure covers it.

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn course_sql_touches_only_course_tables() {
    let src = read("src/courses.rs");
    let sql: String = src
        .lines()
        .filter(|l| l.contains("\"") && !l.trim_start().starts_with("//"))
        .collect();
    for forbidden in [
        "attendees",
        "checked_in",
        "check_in",
        "thb_deposits",
        "deposit_statuses",
        "credit_ledger",
        "nft_mint",
        "claim",
        "quiz",
    ] {
        assert!(
            !sql.contains(forbidden),
            "courses.rs SQL mentions {forbidden}"
        );
    }
    for table in ["course_enrolments", "course_progress"] {
        assert!(sql.contains(table));
    }
}

#[test]
fn course_routes_need_a_signed_in_identity() {
    let routes = read("src/handlers/mod.rs");
    let at = routes
        .find("\"/courses/{course}/progress\"")
        .expect("course routes");
    // The attendee-authed router starts before them and the staff router after.
    let authed = routes
        .find("Developer profile (attendee-authed")
        .expect("attendee router marker");
    assert!(
        at < authed,
        "course routes sit in the attendee-authed router"
    );
    let public_end = routes
        .find("// Waitlist signup (public)")
        .expect("public router marker");
    assert!(at > public_end, "course routes are not public");
}

#[test]
fn erasure_and_the_input_check_cover_courses() {
    assert!(read("src/handlers/privacy.rs").contains("crate::courses::erase(db, &email)"));
    let handler = read("src/handlers/courses.rs");
    // shape before D1; registration needs an active campaign; a watched mark
    // needs an episode of it, in SQL
    assert!(handler.contains("is_course_id(slug)"));
    assert!(handler.contains("crate::courses::is_open(db, &slug)"));
    let sql = read("src/courses.rs");
    let watched = &sql[sql.find("pub const WATCHED_SQL").unwrap()..];
    let watched = &watched[..watched.find(";\n").unwrap()];
    assert!(watched.contains("FROM course_enrolments WHERE email = ?1 AND course = ?2"));
    assert!(watched.contains("ce.campaign_id = ?2 AND e.slug = ?3 AND e.visibility = 'public'"));
}
