//! Writing off an approved deposit must say why.
//!
//! An approved THB deposit sits in the refund queue as money the organizer has
//! promised back. `POST /api/deposit/thb/comp` can reclassify it as a comp
//! (nothing owed), which the Refund Queue now offers. That is a decision about
//! real money after the fact, so the handler refuses it without a reason, and
//! the reason is what the audit entry records. A pending slip's comp (admit at
//! the door) stays reason-optional.

use std::{fs, path::Path};

const HANDLER: &str = "handlers/deposit/thb/handlers/comp.rs";

fn source(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel);
    let body = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{rel} is unreadable: {e} — was the module moved?"));
    // Drop `//` comments so prose is not read as code.
    body.lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn an_approved_deposit_needs_a_reason_to_be_comped() {
    let code = source(HANDLER);
    assert!(
        code.contains("if deposit.verified && reason.is_none()"),
        "{HANDLER}: comping an approved deposit must be refused without a reason"
    );
}

#[test]
fn the_reason_check_runs_before_the_write() {
    let code = source(HANDLER);
    let check = code
        .find("if deposit.verified && reason.is_none()")
        .unwrap_or_else(|| panic!("{HANDLER}: the reason check is gone"));
    let write = code
        .find("save_thb_deposit(")
        .unwrap_or_else(|| panic!("{HANDLER}: the deposit write is gone"));
    assert!(
        check < write,
        "{HANDLER}: the reason check must run before the deposit is rewritten"
    );
}

#[test]
fn the_audit_entry_carries_the_checked_reason() {
    let code = source(HANDLER);
    assert!(
        code.contains("match reason {"),
        "{HANDLER}: the audit entry must use the same trimmed reason the check saw"
    );
}
