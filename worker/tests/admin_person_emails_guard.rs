//! Super-admin email linking stays super-admin, reasoned and audited.
//!
//! Plan 025 §6.1 / §7.4 (`.issues/122`). Linking two emails shares rolling
//! credit between them, so a link written by the wrong person, or without a
//! trace, moves money. The SQL rules (admin proof, the negative-balance refusal
//! on unlink) are behaviour-tested against SQLite in
//! `tests/security/test_person_emails.py`; this guard pins the HTTP wiring
//! around them.

use std::{fs, path::Path};

const HANDLER: &str = "handlers/admin_person_emails.rs";
const ROUTER: &str = "handlers/mod.rs";

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

fn function_body<'a>(code: &'a str, signature: &str) -> &'a str {
    let start = code
        .find(signature)
        .unwrap_or_else(|| panic!("{HANDLER}: `{signature}` is gone — was it renamed?"));
    let rest = &code[start..];
    let end = rest.find("\n}\n").unwrap_or(rest.len());
    &rest[..end]
}

const HANDLERS: [&str; 3] = [
    "pub async fn list_person_emails(",
    "pub async fn link_person_emails(",
    "pub async fn unlink_person_email(",
];

#[test]
fn every_handler_checks_super_admin_first() {
    let code = source(HANDLER);
    for signature in HANDLERS {
        let body = function_body(&code, signature);
        let check = body
            .find("require_super_admin(")
            .unwrap_or_else(|| panic!("{HANDLER}: `{signature}` does not require super admin"));
        let first_db = body.find("person::").unwrap_or(body.len());
        assert!(
            check < first_db,
            "{HANDLER}: `{signature}` must check super admin before touching D1"
        );
    }
    let helper = function_body(&code, "async fn require_super_admin(");
    assert!(
        helper.contains("UserRole::SuperAdmin => Ok(())"),
        "{HANDLER}: require_super_admin must admit only UserRole::SuperAdmin"
    );
}

#[test]
fn changes_require_a_reason_and_are_audited() {
    let code = source(HANDLER);
    for (signature, action) in [
        (
            "pub async fn link_person_emails(",
            "PersonEmailsLinkedByAdmin",
        ),
        (
            "pub async fn unlink_person_email(",
            "PersonEmailUnlinkedByAdmin",
        ),
    ] {
        let body = function_body(&code, signature);
        assert!(
            body.contains("reason_arg("),
            "{HANDLER}: `{signature}` must require a reason"
        );
        assert!(
            body.contains(action) && body.contains("audit("),
            "{HANDLER}: `{signature}` must write the {action} audit entry"
        );
    }
}

#[test]
fn an_admin_link_is_recorded_as_admin_proof() {
    let code = source(HANDLER);
    let body = function_body(&code, "pub async fn link_person_emails(");
    assert!(
        body.contains("LinkProof::Admin"),
        "{HANDLER}: an admin link must store proof = admin, not google"
    );
}

#[test]
fn the_routes_sit_in_the_authed_routers() {
    let code = source(ROUTER);
    // A router's routes run from its `let NAME = Router::new()` to the next
    // `let` at the same indent.
    let router_span = |name: &str| {
        let start = code
            .find(&format!("let {name} = Router::new()"))
            .unwrap_or_else(|| panic!("{ROUTER}: the {name} router is gone"));
        let end = code[start + 1..]
            .find("\n    let ")
            .map_or(code.len(), |i| start + 1 + i);
        start..end
    };
    for (route, name) in [
        (
            "admin_person_emails::list_person_emails",
            "protected_no_store",
        ),
        ("admin_person_emails::link_person_emails", "protected"),
        ("admin_person_emails::unlink_person_email", "protected"),
    ] {
        let at = code
            .find(route)
            .unwrap_or_else(|| panic!("{ROUTER}: `{route}` is not routed"));
        assert!(
            router_span(name).contains(&at),
            "{ROUTER}: `{route}` must be in the {name} router (Extension<Claims> \
             handlers 500 in the public router)"
        );
    }
}
