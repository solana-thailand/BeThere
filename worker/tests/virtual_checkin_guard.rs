//! Guard: every handler that flips an attendee to checked-in must run an
//! approval gate.
//!
//! Three paths set `checked_in_at`: the staff scan (`handlers/checkin.rs`,
//! both the on-site and the `online=true` branch), the self-serve adventure
//! quest completion (`handlers/adventure.rs::quest_complete_checkin`) and the
//! claim mint's auto virtual check-in (`claim/mint/execute.rs`).
//!
//! The claim mint path deliberately does *not* re-check approval. It reads a
//! set `checked_in_at` as proof that an approval-gated path produced it — the
//! same transitive-trust shape that the deposit verification paths had (see
//! `deposit_verify_guard.rs`). `quest_complete_checkin` broke that trust: it
//! gated on the event format, the adventure config and idempotency, but never
//! on `approval_status`, so a pending or invited registrant could set their
//! own `checked_in_at` and inherit the claim path's trust in it.
//!
//! Both handlers now call `Attendee::can_check_in_virtually` (pinned
//! behaviourally by `domain/tests/virtual_checkin_gate.rs`). These tests fail
//! if a path stops calling it, or hand-rolls the check again.

const CHECKIN: &str = include_str!("../src/handlers/checkin.rs");
const ADVENTURE: &str = include_str!("../src/handlers/adventure.rs");
const CLAIM_EXECUTE: &str = include_str!("../src/claim/mint/execute.rs");

/// Source with `//`, `///` and `//!` lines stripped, so a rule that talks about
/// code is never satisfied (or broken) by prose describing it.
fn code_only(source: &str) -> String {
    source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn quest_complete_runs_the_approval_gate() {
    let code = code_only(ADVENTURE);

    assert!(
        code.contains("can_check_in_virtually()"),
        "handlers/adventure.rs writes `checked_in_at` but never calls \
         `can_check_in_virtually` — a pending or invited registrant can then \
         check themselves in, and claim/mint/execute.rs reads that as approval"
    );
}

#[test]
fn quest_complete_gates_before_it_writes() {
    let code = code_only(ADVENTURE);

    let gate = code
        .find("can_check_in_virtually()")
        .expect("adventure.rs must call the gate (see the sibling test)");
    let write = code
        .find("check_in_attendee(")
        .expect("adventure.rs is expected to write the check-in via check_in_attendee");

    assert!(
        gate < write,
        "handlers/adventure.rs calls `check_in_attendee` before \
         `can_check_in_virtually` — the gate must refuse the write, not \
         annotate it after the fact"
    );
}

#[test]
fn online_checkin_branch_shares_the_domain_gate() {
    let code = code_only(CHECKIN);

    assert!(
        code.contains("can_check_in_virtually()"),
        "handlers/checkin.rs no longer calls `can_check_in_virtually` for the \
         `online=true` branch — the two virtual check-in paths must share one \
         gate so a rule added to either applies to both"
    );

    // A hand-rolled copy is how the two drifted apart in the first place: the
    // online branch open-coded `is_checked_in` + `is_approved` and the adventure
    // path copied only the first half.
    assert!(
        !code.contains("attendee.is_approved()"),
        "handlers/checkin.rs open-codes the approval check again — express it \
         through `can_check_in_virtually` / `can_check_in` so the rule lives in \
         one place"
    );
}

#[test]
fn the_claim_path_still_relies_on_checked_in_at() {
    let code = code_only(CLAIM_EXECUTE);

    // This is the premise the two guards above protect. If the claim path ever
    // grows its own approval check, that is fine — but then this test should be
    // rewritten deliberately, not silently left asserting a stale assumption.
    assert!(
        !code.contains("is_approved()"),
        "claim/mint/execute.rs now checks approval directly. The virtual \
         check-in guards above exist because it did not; re-read them and \
         decide whether they are still the right shape before deleting this."
    );

    assert!(
        code.contains("attendee.checked_in_at.is_none()"),
        "claim/mint/execute.rs no longer branches on `checked_in_at` — the \
         transitive trust these guards protect has moved, so re-derive them"
    );
}
