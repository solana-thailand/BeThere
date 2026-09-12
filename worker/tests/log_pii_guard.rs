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
    // Capability values. `claim_token_log_guard.rs` covers these too, but only
    // in their `= %` form; the Issue 070 probe found one recorded without a
    // sigil (`claim_token = attendee.claim_token.as_deref()...`), which is why
    // `renders_field` below accepts any `=`.
    "claim_token",
    "token",
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
    tracing_bodies_with(source, true)
}

/// Same walk, but with string literals preserved, so a message-string capture
/// (`"... {email}"`) and the macro's trailing arguments stay visible.
fn tracing_bodies_raw(source: &str) -> Vec<String> {
    tracing_bodies_with(source, false)
}

fn tracing_bodies_with(source: &str, blank_literals: bool) -> Vec<String> {
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
                    // Literals never contribute to paren depth. In blanked mode
                    // nothing of the contents survives, so message text can
                    // never be mistaken for a field; in raw mode the literal is
                    // kept so inline `{email}` captures stay visible.
                    let literal_start = index;
                    index += 1;
                    while index < bytes.len() && bytes[index] as char != '"' {
                        index += if bytes[index] as char == '\\' { 2 } else { 1 };
                    }
                    index = (index + 1).min(bytes.len());
                    match blank_literals {
                        true => body.push_str("\"\""),
                        false => body.push_str(&source[literal_start..index]),
                    }
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

/// True when `body` records `field` as a tracing field, under any sigil.
///
/// Requires a non-identifier character before the name so `staff_fingerprint`
/// does not match `staff`, and `.email` (a struct access inside a redactor
/// call) does not match `email`. Any `field = …` counts: every name in
/// [`FORBIDDEN_FIELDS`] identifies a person or a capability, so the value is
/// wrong whether it is rendered with `%`, `?`, or recorded directly.
fn renders_field(body: &str, field: &str) -> bool {
    let mut search = 0;
    while let Some(found) = body[search..].find(field) {
        let start = search + found;
        let end = start + field.len();
        let before_ok = start == 0
            || !matches!(body.as_bytes()[start - 1] as char, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '.');
        let after = body[end..].trim_start();
        // Either `field = <expr>` in any form, or the `%field` shorthand.
        let assigned = after.starts_with('=') && !after.starts_with("==");
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

/// Expressions that read a personal or durable identifier. Unlike
/// [`FORBIDDEN_FIELDS`] these are matched anywhere in a tracing body, so a
/// value interpolated into the *message* (`"requested by: {}", claims.email`)
/// is caught too — that class slipped past the field scan and reached the log
/// stream until the Issue 070 runtime probe found it.
const FORBIDDEN_EXPRESSIONS: &[&str] = &[
    ".email",
    "_email,",
    "_email)",
    ".wallet_address",
    ".claim_token",
    ".phone",
    ".contact_handle",
    ".bank_account_number",
    ".first_name",
    ".last_name",
    "{email}",
    "{email:",
    "{wallet}",
    "{phone}",
    "{claim_token}",
    "{signature}",
];

/// Strip every `<name>(<balanced>)` call whose name ends with one of `markers`.
///
/// A value handed to the redactor — or to a deliberate masker — is exactly what
/// the issue asks for, so the scans must not see it. Removing the whole call
/// rather than allow-listing call sites keeps the check independent of how the
/// helper is reached (`state.log_fingerprint(..)`, `redactor.fingerprint(..)`,
/// `crypto::identity_fingerprint(..)`).
fn without_calls(body: &str, markers: &[&str]) -> String {
    let mut current = body.to_string();

    for marker in markers {
        let mut stripped = String::with_capacity(current.len());
        let mut rest = current.as_str();

        while let Some(found) = rest.find(marker) {
            let open = found + marker.len();
            stripped.push_str(&rest[..open - marker.len()]);
            let mut depth = 1usize;
            let mut index = open;
            for byte in rest[open..].bytes() {
                index += 1;
                match byte {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            rest = &rest[index.min(rest.len())..];
        }

        stripped.push_str(rest);
        current = stripped;
    }

    current
}

/// Strip keyed fingerprints only — the sanitizer the log stream is built on.
fn without_fingerprint_calls(body: &str) -> String {
    without_calls(body, &["fingerprint("])
}

/// A tracing body must not read an identifier outside a fingerprint call —
/// including through the message string, which the field scan cannot see.
#[test]
fn identifiers_are_not_interpolated_into_log_messages() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&source_root, &mut sources);
    assert!(!sources.is_empty(), "no Rust sources found under src/");

    let mut violations = Vec::new();
    for path in &sources {
        let source = fs::read_to_string(path).expect("Rust source is UTF-8");
        for body in tracing_bodies_raw(&source) {
            let scanned = without_fingerprint_calls(&body);
            for expression in FORBIDDEN_EXPRESSIONS {
                if scanned.contains(expression) {
                    violations.push(format!("{}: `{expression}`", path.display()));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "raw identifiers reach the Worker log stream through a tracing \
         message or argument — fingerprint them first:\n{}",
        violations.join("\n")
    );
}

/// Request paths carry capability tokens (`/api/claim/{token}`) and wallet
/// addresses (`/api/wallet/{address}/nfts`), so logging `uri().path()` verbatim
/// reintroduces both. `middleware::correlation::redact_path` is the one place
/// allowed to read the path, and it drops opaque segments first.
#[test]
fn request_paths_are_redacted_before_logging() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&source_root, &mut sources);

    for path in &sources {
        let source = fs::read_to_string(path).expect("Rust source is UTF-8");
        for body in tracing_bodies_raw(&source) {
            assert!(
                !body.contains("uri().path()") && !body.contains("uri.path()"),
                "raw request path in a tracing body in {} — log the redacted \
                 path instead",
                path.display()
            );
        }
    }

    let correlation = fs::read_to_string(source_root.join("middleware/correlation.rs"))
        .expect("correlation middleware is readable");
    assert!(
        correlation.contains("redact_path(req.uri().path())"),
        "the correlation middleware must redact the request path before logging it"
    );
}

/// Identifier names that must not appear in an error message an operator can
/// read. Matched against format-capture names and argument expressions only —
/// never the literal prose, so `"D1 find_attendee_by_wallet bind: {e:?}"`
/// stays legal while `"contact not found: {email_lower}"` does not.
const IDENTIFIER_TOKENS: &[&str] = &[
    "email",
    "wallet",
    "claim_token",
    "phone",
    "telegram_id",
    "contact_handle",
    "signature",
];

/// Calls whose argument becomes an error value, and therefore reaches the log
/// stream through a caller's `error = %e`. A `format!` counts only when it sits
/// *inside* the call's parentheses, so `Err(e) => { .. }` match arms and a
/// `map_err(AppError::Internal)?` on the previous line are not error positions.
const ERROR_CONSTRUCTORS: &[&str] = &["map_err(", "ok_or_else(", ".context(", "anyhow!(", "Err("];

/// Contents of the balanced `(..)` opened at `open`, string literals included.
fn balanced_args(source: &str, open: usize) -> (String, usize) {
    let bytes = source.as_bytes();
    let mut index = open + 1;
    let mut depth = 1usize;
    let start = index;

    while index < bytes.len() && depth > 0 {
        match bytes[index] as char {
            '"' => {
                index += 1;
                while index < bytes.len() && bytes[index] as char != '"' {
                    index += if bytes[index] as char == '\\' { 2 } else { 1 };
                }
            }
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        index += 1;
    }

    let end = index.saturating_sub(1).min(bytes.len());
    (source[start..end].to_string(), index)
}

/// Capture names (`{email_lower}` → `email_lower`) plus everything after the
/// message literal, which is where positional arguments live.
fn format_captures_and_args(body: &str) -> String {
    let mut interesting = String::new();
    let mut rest = body;

    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let capture = &after[..close];
                // `{{` is an escaped brace, and a format spec is not a name.
                let name = capture.split(':').next().unwrap_or_default();
                interesting.push(' ');
                interesting.push_str(name);
                rest = &after[close + 1..];
            }
            None => break,
        }
    }

    // Positional args: everything past the closing quote of the message.
    if let Some(literal_start) = body.find('"') {
        let mut index = literal_start + 1;
        let bytes = body.as_bytes();
        while index < bytes.len() && bytes[index] as char != '"' {
            index += if bytes[index] as char == '\\' { 2 } else { 1 };
        }
        if index + 1 < body.len() {
            interesting.push(' ');
            interesting.push_str(&body[index + 1..]);
        }
    }

    interesting
}

/// `mask_wallet` / `mask_email` are reviewed, user-facing partial renderings
/// (`Ab12…9xYz`), not raw identifiers, so a message built from one is legal.
const SANITIZERS: &[&str] = &["fingerprint(", "mask_wallet(", "mask_email("];

fn embeds_identifier(body: &str) -> bool {
    let scanned = format_captures_and_args(&without_calls(body, SANITIZERS));
    IDENTIFIER_TOKENS
        .iter()
        .any(|token| scanned.contains(token))
}

/// Byte spans covered by an error-constructing call's parentheses.
fn error_spans(source: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();

    for needle in ERROR_CONSTRUCTORS {
        let mut search = 0;
        while let Some(at) = source[search..].find(needle) {
            let open = search + at + needle.len() - 1;
            let (_, end) = balanced_args(source, open);
            spans.push((open, end));
            search = open + 1;
        }
    }

    // `AppError::<Variant>(..)` carries its message directly.
    let mut search = 0;
    while let Some(at) = source[search..].find("AppError::") {
        let start = search + at;
        let mut cursor = start + "AppError::".len();
        while source[cursor..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            cursor += 1;
        }
        if source[cursor..].starts_with('(') {
            let (_, end) = balanced_args(source, cursor);
            spans.push((cursor, end));
        }
        search = start + "AppError::".len();
    }

    spans
}

/// Every `format!` body that sits inside an error-constructing call.
fn error_format_bodies(source: &str) -> Vec<String> {
    let spans = error_spans(source);
    let mut bodies = Vec::new();
    let mut search = 0;

    while let Some(found) = source[search..].find("format!(") {
        let macro_start = search + found;
        let open = macro_start + "format!(".len() - 1;
        let (body, next) = balanced_args(source, open);
        if spans
            .iter()
            .any(|(start, end)| *start < macro_start && macro_start < *end)
        {
            bodies.push(body);
        }
        search = next.max(macro_start + "format!(".len());
    }

    bodies
}

/// An identifier written into an error message reaches the log stream as soon
/// as any caller logs `error = %e` — which every fallible helper's caller in
/// this Worker does. The field and message scans above cannot see it, because
/// the identifier is added in a different function from the tracing call.
///
/// Found by review after the Issue 070 runtime probe: the probe kept Google
/// credentials unusable, so Sheets helpers failed at the token fetch and their
/// post-token error paths (`"contact not found: {email_lower}"`, logged by four
/// deposit-credit call sites) never executed.
#[test]
fn error_messages_do_not_embed_identifiers() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&source_root, &mut sources);
    assert!(!sources.is_empty(), "no Rust sources found under src/");

    let mut violations = Vec::new();
    for path in &sources {
        let source = fs::read_to_string(path).expect("Rust source is UTF-8");
        for body in error_format_bodies(&source) {
            if embeds_identifier(&body) {
                violations.push(format!("{}: {}", path.display(), body.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "identifier embedded in an error message — callers log these as \
         `error = %e`, so describe the failure and let the call site's \
         fingerprint field carry the identity:\n{}",
        violations.join("\n")
    );
}

/// Replace comment bodies and string-literal contents with spaces, so a scan
/// for code shapes cannot trip over prose that mentions them. Byte lengths are
/// preserved, and multi-byte characters are copied whole.
fn code_only(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut index = 0;

    while index < source.len() {
        let rest = &source[index..];
        let blanked = match () {
            () if rest.starts_with("//") => rest.find('\n').unwrap_or(rest.len()),
            () if rest.starts_with("/*") => rest.find("*/").map_or(rest.len(), |at| at + 2),
            () if rest.starts_with("r#\"") => {
                rest[3..].find("\"#").map_or(rest.len(), |at| at + 3 + 2)
            }
            () if rest.starts_with('"') => {
                let bytes = rest.as_bytes();
                let mut cursor = 1;
                while cursor < bytes.len() && bytes[cursor] as char != '"' {
                    cursor += if bytes[cursor] as char == '\\' { 2 } else { 1 };
                }
                (cursor + 1).min(rest.len())
            }
            () => 0,
        };

        match blanked {
            0 => {
                let character = rest.chars().next().expect("index is a char boundary");
                out.push(character);
                index += character.len_utf8();
            }
            length => {
                out.push_str(&" ".repeat(length));
                index += length;
            }
        }
    }

    out
}

const TRACING_LEVELS: &[&str] = &["info", "warn", "error", "debug", "trace"];

/// True when `source` invokes a tracing level macro without the `tracing::`
/// path — `use tracing::info; info!(..)`.
fn bare_tracing_calls(source: &str) -> Vec<String> {
    let code = code_only(source);
    let mut found = Vec::new();

    for level in TRACING_LEVELS {
        let needle = format!("{level}!(");
        let mut search = 0;
        while let Some(at) = code[search..].find(&needle) {
            let start = search + at;
            let preceding = code[..start].chars().next_back();
            let qualified = code[..start].ends_with("::");
            let part_of_name = preceding.is_some_and(|character| {
                character.is_alphanumeric() || character == '_' || character == '.'
            });
            if !qualified && !part_of_name {
                found.push(needle.clone());
            }
            search = start + needle.len();
        }
    }

    found
}

/// Every guard in this file extracts `tracing::<level>!(` bodies, so a bare
/// `info!(email = %claims.email, ..)` reached through `use tracing::info` would
/// be invisible to all of them. Keeping the path qualified is what makes the
/// scans exhaustive rather than best-effort.
#[test]
fn tracing_macros_stay_path_qualified() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&source_root, &mut sources);

    let mut violations = Vec::new();
    for path in &sources {
        let source = fs::read_to_string(path).expect("Rust source is UTF-8");
        for call in bare_tracing_calls(&source) {
            violations.push(format!("{}: `{call}`", path.display()));
        }
    }

    assert!(
        violations.is_empty(),
        "unqualified tracing macro — write `tracing::info!(..)`; the PII guards \
         in this file only see path-qualified calls:\n{}",
        violations.join("\n")
    );
}

/// The guards above scan `worker/src` only. The `domain` crate is pure types
/// and validation with no logging and no `tracing` dependency, which is why
/// that scope is complete; if domain starts logging, extend the scans to it
/// rather than deleting this assertion.
#[test]
fn the_domain_crate_stays_log_free() {
    let domain_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("domain")
        .join("src");
    let mut sources = Vec::new();
    rust_sources(&domain_root, &mut sources);
    assert!(
        !sources.is_empty(),
        "no Rust sources found under domain/src/"
    );

    for path in &sources {
        let source = fs::read_to_string(path).expect("Rust source is UTF-8");
        assert!(
            tracing_bodies_raw(&source).is_empty() && bare_tracing_calls(&source).is_empty(),
            "{} logs — extend the Issue 070 guards to cover domain/src",
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
        // Recorded without a sigil — the form the runtime probe caught.
        r#"tracing::info!(claim_token = attendee.claim_token.as_deref().unwrap_or(""), "x");"#,
    ];
    for source in positives {
        let flagged = tracing_bodies(source)
            .iter()
            .any(|body| FORBIDDEN_FIELDS.iter().any(|f| renders_field(body, f)));
        assert!(flagged, "guard failed to flag: {source}");
    }

    let message_positives = [
        r#"tracing::info!("listing attendees (requested by: {})", claims.email);"#,
        r#"tracing::info!("login successful: {}", user_info.email);"#,
        r#"tracing::warn!("no wallet for {email}");"#,
    ];
    for source in message_positives {
        let flagged = tracing_bodies_raw(source).iter().any(|body| {
            let scanned = without_fingerprint_calls(body);
            FORBIDDEN_EXPRESSIONS.iter().any(|e| scanned.contains(e))
        });
        assert!(
            flagged,
            "guard failed to flag message interpolation: {source}"
        );
    }

    let message_negatives = [
        r#"tracing::info!("actor {}", state.log_fingerprint(&claims.email));"#,
        r#"tracing::info!(staff_fingerprint = %state.log_fingerprint(&claims.email), "x");"#,
        r#"tracing::warn!("no email on file for this attendee");"#,
    ];
    for source in message_negatives {
        let flagged = tracing_bodies_raw(source).iter().any(|body| {
            let scanned = without_fingerprint_calls(body);
            FORBIDDEN_EXPRESSIONS.iter().any(|e| scanned.contains(e))
        });
        assert!(!flagged, "guard false-positived on message: {source}");
    }

    let error_positives = [
        r#".ok_or_else(|| format!("contact not found: {email_lower}"))?;"#,
        r#".map_err(|e| format!("sync failed for {}: {e}", attendee.email))?;"#,
        r#"return Err(AppError::Validation(format!("bad wallet {wallet_address}")));"#,
    ];
    for source in error_positives {
        let flagged = error_format_bodies(source)
            .iter()
            .any(|b| embeds_identifier(b));
        assert!(flagged, "guard failed to flag error message: {source}");
    }

    let error_negatives = [
        // Prose that merely names a column or query, with no identifier value.
        r#".map_err(|e| format!("D1 find_attendee_by_wallet bind: {e:?}"))?;"#,
        r#".ok_or_else(|| "contact not found in contacts sheet".to_string())?;"#,
        r#".map_err(|e| format!("failed for {}", state.log_fingerprint(&a.email)))?;"#,
        // Not an error position: a KV key and a response body may hold the value.
        r#"let kv_key = format!("siws_msg_{}", req.wallet_address);"#,
    ];
    for source in error_negatives {
        let flagged = error_format_bodies(source)
            .iter()
            .any(|b| embeds_identifier(b));
        assert!(!flagged, "guard false-positived on error message: {source}");
    }

    let bare_positives = [
        r#"info!(email = %claims.email, "x");"#,
        "    warn!(\n        wallet = %w,\n        \"x\"\n    );",
    ];
    for source in bare_positives {
        assert!(
            !bare_tracing_calls(source).is_empty(),
            "guard failed to flag unqualified macro: {source}"
        );
    }

    let bare_negatives = [
        r#"tracing::info!(event_id = %id, "x");"#,
        r#"let message = "see tracing::info!( for the format";"#,
        "// prefer info!( over println!(\n",
        r#"self.error!(x);"#,
    ];
    for source in bare_negatives {
        assert!(
            bare_tracing_calls(source).is_empty(),
            "guard false-positived on qualified/quoted macro: {source}"
        );
    }

    let negatives = [
        r#"tracing::info!(staff_fingerprint = %state.log_fingerprint(&claims.email), "x");"#,
        r#"tracing::info!(wallet_fingerprint = %redactor.fingerprint(w), "x");"#,
        r#"tracing::info!(sig_fingerprint = %redactor.fingerprint(&e.signature), "x");"#,
        r#"tracing::info!(event_id = %id, name = %config.name, "event created");"#,
        r#"tracing::info!(claim_token_fingerprint = %crypto::claim_token_fingerprint(t), "x");"#,
        r#"tracing::warn!("no email on file for this attendee");"#,
    ];
    for source in negatives {
        let flagged = tracing_bodies(source)
            .iter()
            .any(|body| FORBIDDEN_FIELDS.iter().any(|f| renders_field(body, f)));
        assert!(!flagged, "guard false-positived on: {source}");
    }
}
