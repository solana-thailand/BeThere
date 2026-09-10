//! Guard: every path that flips an attendee to checked-in runs the same gates.
//!
//! Three paths set `checked_in_at`: the staff scan (`handlers/checkin.rs`, both
//! the on-site and the `online=true` branch), the self-serve adventure quest
//! completion (`handlers/adventure.rs::quest_complete_checkin`) and the claim
//! mint's auto virtual check-in (`claim/mint/execute.rs`).
//!
//! Plan 022 §2 found the three carrying different gates: `quest_complete_checkin`
//! never checked `approval_status`, and the claim mint path never did either —
//! it read a set `checked_in_at` as proof that an approval-gated path had
//! produced one, the same transitive-trust shape the deposit verification paths
//! had (see `deposit_verify_guard.rs`).
//!
//! The two self-serve paths now delegate to `virtual_checkin::commit_virtual_
//! check_in`, which owns the online-track and `can_check_in_virtually` gates
//! (pinned behaviourally by `domain/tests/virtual_checkin_gate.rs`) and writes
//! D1 before Sheets. These tests fail if a caller stops delegating, if the
//! module stops gating, or if a *fourth* virtual writer appears.

use std::path::{Path, PathBuf};

const CHECKIN: &str = include_str!("../src/handlers/checkin.rs");
const ADVENTURE: &str = include_str!("../src/handlers/adventure.rs");
const CLAIM_EXECUTE: &str = include_str!("../src/claim/mint/execute.rs");
const VIRTUAL_CHECKIN: &str = include_str!("../src/virtual_checkin.rs");

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
fn the_shared_writer_gates_before_it_writes() {
    let code = code_only(VIRTUAL_CHECKIN);

    let gate = code.find("can_check_in_virtually()").expect(
        "virtual_checkin.rs must run the domain gate — it is the whole point of the module",
    );
    let online = code
        .find("event_format.has_online()")
        .expect("virtual_checkin.rs must gate on the event having an online track");
    let write = code
        .find("check_in_attendee(")
        .expect("virtual_checkin.rs is expected to write the check-in via check_in_attendee");

    assert!(
        gate < write && online < write,
        "virtual_checkin.rs writes the check-in before gating it — the gates \
         must refuse the write, not annotate it after the fact"
    );
}

#[test]
fn the_self_serve_paths_delegate_to_the_shared_writer() {
    for (name, source) in [
        ("handlers/adventure.rs", ADVENTURE),
        ("claim/mint/execute.rs", CLAIM_EXECUTE),
    ] {
        let code = code_only(source);

        assert!(
            code.contains("commit_virtual_check_in("),
            "{name} no longer delegates its virtual check-in to \
             virtual_checkin::commit_virtual_check_in — the two self-serve \
             paths must share one gate set so a rule added to either applies \
             to both"
        );
        assert!(
            !code.contains("check_in_attendee("),
            "{name} writes the check-in directly again. That is how the two \
             paths drifted apart: a guard that is copied is a guard that will \
             diverge (plan 022)."
        );
    }
}

#[test]
fn quest_complete_verifies_the_adventure_before_checking_in() {
    let code = code_only(ADVENTURE);

    let verify = code
        .find("let quest_status =")
        .expect("quest_complete_checkin must read the adventure status before checking anyone in");
    let commit = code
        .find("commit_virtual_check_in(")
        .expect("quest_complete_checkin must delegate the write (see the sibling test)");

    assert!(
        verify < commit,
        "handlers/adventure.rs checks the adventure status after it has already \
         checked the attendee in — `checked_in_at` is the attendance signal every \
         dashboard counts, so an unstarted adventure must not set it"
    );
    assert!(
        code.contains("AdventureStatus::Passed"),
        "handlers/adventure.rs no longer requires `AdventureStatus::Passed` — \
         the endpoint's doc comment promises the required levels are verified"
    );
}

#[test]
fn online_checkin_branch_shares_the_domain_gate() {
    let code = code_only(CHECKIN);

    assert!(
        code.contains("can_check_in_virtually()"),
        "handlers/checkin.rs no longer calls `can_check_in_virtually` for the \
         `online=true` branch — the staff and self-serve virtual check-ins must \
         share one gate so a rule added to either applies to both"
    );

    // A hand-rolled copy is how the paths drifted apart in the first place: the
    // online branch open-coded `is_checked_in` + `is_approved` and the adventure
    // path copied only the first half.
    assert!(
        !code.contains("attendee.is_approved()"),
        "handlers/checkin.rs open-codes the approval check again — express it \
         through `can_check_in_virtually` / `can_check_in` so the rule lives in \
         one place"
    );
}

/// The broad form of the guard: catch a *fourth* virtual check-in writer, not
/// just a regression of the two known ones. Any file that writes the Sheets
/// virtual check-in columns must either be the shared writer, the Sheets layer
/// it calls, or it is a new unguarded path.
#[test]
fn no_other_module_writes_a_virtual_check_in() {
    const LICENSED: [&str; 3] = [
        "src/virtual_checkin.rs",
        "src/sheets/bg_sync.rs",
        "src/sheets/write/checkin.rs",
    ];

    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    visit(&src, &mut |path, contents| {
        let rel = path
            .strip_prefix(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if LICENSED.contains(&rel.as_str()) {
            return;
        }
        if code_only(contents).contains("mark_virtual_checked_in(") {
            offenders.push(rel);
        }
    });

    assert!(
        offenders.is_empty(),
        "these modules write a virtual check-in without going through \
         virtual_checkin::commit_virtual_check_in, so they carry their own \
         (possibly missing) approval gate: {offenders:?}"
    );
}

fn visit(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    let entries = std::fs::read_dir(dir).expect("worker/src must be readable");
    for entry in entries.flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => visit(&path, f),
            false if path.extension().is_some_and(|e| e == "rs") => {
                let contents =
                    std::fs::read_to_string(&path).expect("source file must be readable");
                f(&path, &contents);
            }
            false => {}
        }
    }
}
