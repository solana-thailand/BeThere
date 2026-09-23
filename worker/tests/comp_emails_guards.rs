//! Guards for the deposit-waived email list (`EventConfig::comp_emails`).
//!
//! The organizer names guests who do not pay — speakers, sponsors, VIPs. An
//! email on that list takes the same path staff already take: a ฿0 comp is
//! recorded at signup, the ticket QR is issued, and no refund is ever owed.
//!
//! Two properties matter and neither is visible to the compiler:
//!
//! 1. **It must grant nothing.** The obvious way to waive someone's deposit
//!    before this existed was to add them to the staff list — and `is_staff`
//!    gates the ENTIRE protected admin router (`auth.rs`). A guest given staff
//!    access to admit them free is a much worse outcome than charging them.
//! 2. **Matching must be case-insensitive.** The list is typed by a human into
//!    an admin form; the email arrives from an OAuth provider. If those two
//!    disagree on casing the waiver silently does nothing, and the VIP is asked
//!    for ฿500 at the door with no indication why.

use std::fs;
use std::path::Path;

fn repo_file(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn strip_rs_comments(code: &str) -> String {
    code.lines()
        .map(|l| match l.trim_start().starts_with("//") {
            true => "",
            false => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The list waives the deposit and does nothing else. If `comp_emails` ever
/// appears in the auth layer, someone has turned a guest list into a
/// permission.
#[test]
fn the_waiver_list_grants_no_privileges() {
    for file in ["src/auth.rs", "src/handlers/mod.rs", "src/state.rs"] {
        let code = strip_rs_comments(&repo_file(file));
        assert!(
            !code.contains("comp_emails"),
            "{file} must not consult comp_emails. It waives a deposit; it is not a \
             role. `is_staff` gates the entire admin router, and conflating the two \
             is exactly what this list exists to avoid"
        );
    }
}

/// It must be read where the waiver is decided, or it does nothing at all.
#[test]
fn signup_consults_the_waiver_list() {
    let code = strip_rs_comments(&repo_file("src/handlers/register/signup.rs"));
    assert!(
        code.contains("config.comp_emails.contains(&email.to_lowercase())"),
        "register::signup must include comp_emails in `deposit_waived`, matched \
         lowercase — the list is stored normalised so this is a plain membership test"
    );
}

/// Stored lowercase, trimmed and deduped exactly once, on write. Normalising on
/// read instead would mean every future reader has to remember to do it, and
/// one that forgets fails open — the VIP is charged.
#[test]
fn the_list_is_normalised_when_it_is_written() {
    let code = strip_rs_comments(&repo_file("src/event_store/write/update.rs"));
    for expected in [
        "e.trim().to_lowercase()",
        "filter(|e| !e.is_empty())",
        "comp_emails.dedup()",
    ] {
        assert!(
            code.contains(expected),
            "apply_update must normalise comp_emails ({expected} missing). A list \
             typed by a human and compared against an OAuth email will not match on \
             casing alone"
        );
    }
}

/// `None` leaves the list untouched; only an explicit list replaces it. Without
/// this, every unrelated event edit — renaming it, changing the venue — would
/// silently clear the guest list.
#[test]
fn an_unrelated_event_edit_cannot_clear_the_list() {
    let code = strip_rs_comments(&repo_file("src/event_store/write/update.rs"));
    assert!(
        code.contains("if let Some(ref emails) = req.comp_emails"),
        "apply_update must only touch comp_emails when the request carries it. A \
         bare assignment would let any other event edit wipe the guest list"
    );
}

/// The field must default, or every event config already in KV fails to
/// deserialize the moment this ships — which is how `staff_sheet_name` broke a
/// local fixture during this work.
#[test]
fn the_field_defaults_so_existing_events_still_parse() {
    let code = repo_file("../domain/src/models/event/config.rs");
    let idx = code
        .find("pub comp_emails")
        .expect("comp_emails exists on EventConfig");
    let preceding = &code[idx.saturating_sub(200)..idx];
    assert!(
        preceding.contains("serde(default"),
        "EventConfig::comp_emails must be #[serde(default)] — every event config \
         already stored in KV predates this field and must still parse"
    );
}
