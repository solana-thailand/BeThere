//! Source-scan guards for linked emails outside the credit ledger (plan 025).
//!
//! Both rules below live in wasm-only paths that fail SILENTLY if the call is
//! dropped: a duplicate registration just creates a second row, and a second
//! badge just mints. `credit_ledger_guards.rs` covers the money half.

use std::fs;
use std::path::Path;

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Registering with a second linked email must return the person's existing
/// row, not open a second registration for the same event.
#[test]
fn signup_dedup_looks_at_the_linked_set() {
    let signup = read("src/handlers/register/signup.rs");
    assert!(
        signup.contains("crate::db::person::emails_of(db, &email)"),
        "signup dedup must resolve the person's emails"
    );
    let exact = signup
        .find("a.email.to_lowercase() == email")
        .expect("the attendee's own email is still matched first");
    let linked = signup
        .find("person_emails.contains(&a.email.to_lowercase())")
        .expect("the linked set must be consulted");
    assert!(
        exact < linked,
        "own email must win over a linked sibling, or an attendee with two rows \
         gets the wrong one"
    );
    assert!(
        signup.contains("vec![email.clone()]"),
        "a D1 failure must degrade to exact-email dedup, never block a registration"
    );
}

/// One badge per person per event: a linked sibling row that already claimed
/// blocks this claim. The check must sit AFTER the per-row `claimed_at` check
/// (which is the primary rule) and BEFORE the mint.
#[test]
fn claim_blocks_a_second_badge_for_the_same_person() {
    let execute = read("src/claim/mint/execute.rs");
    let own_row = execute
        .find("if attendee.claimed_at.is_some()")
        .expect("per-row claimed check exists");
    let person = execute
        .find("crate::db::person::claimed_elsewhere(")
        .expect("person-level claim check exists");
    let recipient = execute
        .find("// 7. Resolve the recipient wallet")
        .expect("wallet resolution follows");
    assert!(
        own_row < person && person < recipient,
        "the linked-email claim check belongs between the per-row check and the mint"
    );
    assert!(
        execute.contains("not blocking"),
        "a failed lookup must not block a legitimate claim (the D1 attendee \
         mirror is a second line of defence, not the source of truth)"
    );

    // The walk-in path returns before the pre-registered checks, so it needs
    // its own copy or a person could claim twice: once on the registered row,
    // once on a walk-in row. Walk-ins are the only rows that may repeat an
    // email in one event (idx_attendees_unique_event_email).
    let walkin = read("src/claim/mint/walkin.rs");
    let walkin_check = walkin
        .find("crate::db::person::claimed_elsewhere(")
        .expect("walk-in claims must run the person-level check too");
    let lock = walkin
        .find("acquire_claim_lock(")
        .expect("walk-in claim takes the dedup lock");
    assert!(
        walkin_check < lock,
        "the walk-in person check must run before the lock and the mint"
    );

    let person_src = read("src/db/person.rs");
    for needle in [
        "a.claimed_at IS NOT NULL AND a.claimed_at <> ''",
        "a.id <> ?3 AND COALESCE(a.claim_token, '') <> ?4",
        "person_emails_of!(\"?2\")",
    ] {
        assert!(
            person_src.contains(needle),
            "claimed_elsewhere SQL: missing `{needle}`"
        );
    }
}
