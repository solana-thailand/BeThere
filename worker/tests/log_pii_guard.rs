//! Personal and durable identifiers must never reach the Worker log stream.
//!
//! Issue 070. The D1 audit log stays the access-controlled, actor-attributable
//! record for authorized operational actions; the general log stream carries
//! only keyed, one-way fingerprints (`AppState::log_fingerprint` /
//! `crypto::LogRedactor`), plus event/org-level identifiers that are not
//! personal. This guard fails the suite if a raw identifier is reintroduced.
//!
//! Companion to `claim_token_log_guard.rs`, which covers capability tokens.

use std::{fs, path::Path};

/// Tracing field names that would carry a personal or durable identifier.
///
/// `name` is deliberately absent: it is also used for event and organization
/// names, which the Issue 070 decision keeps readable. Attendee display names
/// are logged as `name_fingerprint`, and `assert_no_raw_display_name` below
/// covers the specific expression that produces one.
const FORBIDDEN_FIELDS: &[&str] = &[
    "email",
    "staff_email",
    "attendee_email",
    "admin_email",
    "target_email",
    "user_email",
    "owner_email",
    "claims_email",
    "dev_email",
    "wallet",
    "wallet_address",
    "signature",
    "tx_signature",
    "sig",
    "staff",
    "contact",
    "phone",
    "telegram_id",
    "github",
    "handle",
];

fn rust_sources(path: &Path, output: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).expect("source directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            rust_sources(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}

/// Every `tracing::<level>!( .. )` body in the file, with string literals
/// blanked so message text can never be mistaken for a field.
///
/// Paren depth is tracked so multi-line macros are captured whole — the
/// original line-anchored scan for this issue under-counted by more than 2x
/// because it could not see fields on continuation lines.
fn tracing_bodies(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut bodies = Vec::new();
    let mut search = 0;

    while let Some(found) = source[search..].find("tracing::") {
        let macro_start = search + found;
        let Some(open_offset) = source[macro_start..].find('(') else {
            break;
        };
        let open = macro_start + open_offset;
        // Only `tracing::<ident>!(` counts; `tracing::Subscriber::foo(` does not.
        if !source[macro_start..open].ends_with('!') {
            search = macro_start + "tracing::".len();
            continue;
        }

        let mut index = open + 1;
        let mut depth = 1usize;
        let mut body = String::new();
        while index < bytes.len() && depth > 0 {
            match bytes[index] as char {
                '"' => {
                    // Blank the literal, preserving nothing of its contents.
                    index += 1;
                    while index < bytes.len() && bytes[index] as char != '"' {
                        index += if bytes[index] as char == '\\' { 2 } else { 1 };
                    }
                    index += 1;
                    body.push_str("\"\"");
                    continue;
                }
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        index += 1;
                        break;
                    }
                }
                _ => {}
            }
            body.push(bytes[index] as char);
            index += 1;
        }

        bodies.push(body);
        search = index.max(macro_start + "tracing::".len());
    }

    bodies
}

/// True when `body` renders `field` with `%` or `?` as a tracing field.
///
/// Requires a non-identifier character before the name so `staff_fingerprint`
/// does not match `staff`, and `.email` (a struct access inside a redactor
/// call) does not match `email`.
fn renders_field(body: &str, field: &str) -> bool {
    let mut search = 0;
    while let Some(found) = body[search..].find(field) {
        let start = search + found;
        let end = start + field.len();
        let before_ok = start == 0
            || !matches!(body.as_bytes()[start - 1] as char, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '.');
        let after = body[end..].trim_start();
        // Either `field = %expr` / `field = ?expr`, or the `%field` shorthand.
        let assigned = after.starts_with("= %") || after.starts_with("= ?");
        let shorthand = start > 0
            && body.as_bytes()[start - 1] as char == '%'
            && after.starts_with(',')
            && (start == 1
                || !matches!(body.as_bytes()[start - 2] as char, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'));
        if before_ok && assigned || shorthand {
            return true;
        }
        search = end;
    }
    false
}

#[test]
fn identifiers_are_fingerprinted_before_logging() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&source_root, &mut sources);
    assert!(!sources.is_empty(), "no Rust sources found under src/");

    let mut violations = Vec::new();
    for path in &sources {
        let source = fs::read_to_string(path).expect("Rust source is UTF-8");
        for body in tracing_bodies(&source) {
            for field in FORBIDDEN_FIELDS {
                if renders_field(&body, field) {
                    violations.push(format!("{}: tracing field `{field}`", path.display()));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "raw identifiers in Worker tracing fields — log the keyed fingerprint \
         instead (AppState::log_fingerprint / LogRedactor::fingerprint):\n{}",
        violations.join("\n")
    );
}

/// An attendee's display name is personal data, so it must be fingerprinted
/// even though the bare field name `name` stays legal for events and orgs.
#[test]
fn attendee_display_names_are_fingerprinted_before_logging() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&source_root, &mut sources);

    for path in &sources {
        let source = fs::read_to_string(path).expect("Rust source is UTF-8");
        assert!(
            !source.contains("name = %attendee.display_name()")
                && !source.contains("name = %display_name")
                && !source.contains("name = %walkin_attendee.name"),
            "raw attendee display name in a tracing field in {}",
            path.display()
        );
    }
}

#[test]
fn the_guard_detects_a_reintroduced_identifier() {
    // Without this, a bug in `tracing_bodies` or `renders_field` would make the
    // guard silently pass on everything.
    let positives = [
        r#"tracing::info!(email = %claims.email, "x");"#,
        r#"tracing::warn!(wallet = %req.wallet_address, "x");"#,
        r#"tracing::info!(%email, "x");"#,
        r#"tracing::error!(tx_signature = ?deposit.tx_signature, "x");"#,
        "tracing::info!(\n    attendee_id = %id,\n    staff_email = %claims.email,\n    \"x\"\n);",
    ];
    for source in positives {
        let flagged = tracing_bodies(source)
            .iter()
            .any(|body| FORBIDDEN_FIELDS.iter().any(|f| renders_field(body, f)));
        assert!(flagged, "guard failed to flag: {source}");
    }

    let negatives = [
        r#"tracing::info!(staff_fingerprint = %state.log_fingerprint(&claims.email), "x");"#,
        r#"tracing::info!(wallet_fingerprint = %redactor.fingerprint(w), "x");"#,
        r#"tracing::info!(sig_fingerprint = %redactor.fingerprint(&e.signature), "x");"#,
        r#"tracing::info!(event_id = %id, name = %config.name, "event created");"#,
        r#"tracing::warn!("no email on file for this attendee");"#,
    ];
    for source in negatives {
        let flagged = tracing_bodies(source)
            .iter()
            .any(|body| FORBIDDEN_FIELDS.iter().any(|f| renders_field(body, f)));
        assert!(!flagged, "guard false-positived on: {source}");
    }
}
