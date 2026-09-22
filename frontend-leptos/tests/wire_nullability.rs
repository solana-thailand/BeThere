//! Wire-nullability guard — `null` introduced by a **handler expression**.
//!
//! Companion to `mirror_field_types.rs`, which compares a frontend mirror
//! struct against its domain SSOT struct. That guard already flags the unsafe
//! optionality direction (domain `Option<T>` vs frontend `T`) — but it reads
//! *struct definitions*, so it is blind to a field that is a plain `String` in
//! the domain and becomes `null` on the wire because the handler passed it
//! through a helper that returns `Option`.
//!
//! ## The regression this exists for
//!
//! `worker/src/handlers/attendee/read.rs` built the ticket payload with
//! `"event_location_map_url": safe_map_url(&event.location_map_url)`.
//! `domain::models::event::config::EventConfig::location_map_url` is a plain
//! `String`, so `mirror_field_types` saw `String` on both sides and passed —
//! and `AttendeeData` is not one of its pairs in any case. But
//! `safe_map_url` returns `Option<String>` and yields `None` for an empty
//! stored value, so `json!` emitted a literal `null`, while the client
//! declared `#[serde(default)] pub event_location_map_url: String`.
//!
//! **`#[serde(default)]` fires only when a key is ABSENT, never on an explicit
//! `null`.** Deserialization of the whole struct failed, so the entire ticket
//! page died with "Failed to parse ticket data: invalid type: null, expected a
//! string" for every event with no venue map link — which is every online-only
//! event, the exact case nobody sets a map link for. See `.issues/133`.
//!
//! The sibling path got it right, which is why it went unnoticed:
//! `worker/src/handlers/public_event.rs` emits the same nullable value and the
//! public-event client declares `Option<String>`. Opening the public event page
//! by hand — the obvious manual test — works fine.
//!
//! ## What this checks
//!
//! For each pair in [`PAYLOAD_PAIRS`]: every `"key": expr` entry of the
//! handler's `json!` payload whose `expr` calls a `pub fn … -> Option<…>`
//! declared anywhere under `domain/src` must either flatten the `Option`
//! (`.unwrap_or*` / `.map_or*`) or be declared `Option<…>` on the client.
//!
//! The helper list is derived from the domain source rather than hard-coded,
//! so a new `Option`-returning helper is covered the day it is written.
//!
//! ## Scope — deliberately narrow
//!
//! - Only **single-line** `"key": expr` entries are inspected. A multi-line
//!   expression (e.g. the `match` that picks `ticket_note`) is skipped.
//! - Only calls to domain helpers are treated as nullable. A handler-local
//!   `let` binding of `Option` type is **not** tracked, and is **checked by
//!   nothing** — not by this guard, and not by `mirror_field_types.rs`, which
//!   parses struct *definitions* and so cannot see a local binding by
//!   construction (nor does it list `AttendeeData` as a pair at all).
//!   Concretely, of the nine nullable values in the ticket payload this guard
//!   covers exactly one — `event_location_map_url`, the one that arrives via a
//!   domain helper. The other eight (`qr_image`, `deposit_info`,
//!   `claimed_asset_id`, `cluster`, `rollover_target_event`,
//!   `in_person_available`, `refund_link`, `deposit_deadline_hours`) are all
//!   correctly `Option` client-side today, but nothing keeps them that way.
//!   Do not read a pass here as "the payload is safe".
//! - The `Option`-flattening check (`.unwrap_or*` / `.map_or*`) matches
//!   anywhere in the expression, not just the outermost call, so a construction
//!   like `safe_map_url(&x.unwrap_or_default())` would be waved through. No
//!   such expression exists today; tighten this if one appears.
//! - A field absent from the client struct is not a violation — clients may
//!   legitimately ignore payload fields.
//!
//! ## Run
//!
//! ```sh
//! cargo test --test wire_nullability
//! ```

use std::fs;
use std::path::{Path, PathBuf};

/// Root of the `frontend-leptos` crate.
const FRONTEND_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"));

/// Workspace root, resolved as the parent of `frontend-leptos/`.
fn workspace_root() -> PathBuf {
    Path::new(FRONTEND_ROOT)
        .parent()
        .expect("frontend-leptos should have a parent directory (the workspace root)")
        .to_path_buf()
}

/// One worker `json!` payload paired with the client struct that parses it.
struct PayloadPair {
    /// Handler file, relative to the workspace root.
    worker_file: &'static str,
    /// Client file, relative to `frontend-leptos/`.
    frontend_file: &'static str,
    /// Client struct name.
    frontend_struct: &'static str,
    /// Floor on inspected payload entries — fails loudly if the `json!` block
    /// moves or is renamed and the guard silently checks nothing.
    min_entries: usize,
}

/// The audited payloads. Adding a pair widens coverage.
const PAYLOAD_PAIRS: &[PayloadPair] = &[
    PayloadPair {
        worker_file: "worker/src/handlers/attendee/read.rs",
        frontend_file: "src/api/types.rs",
        frontend_struct: "AttendeeData",
        min_entries: 20,
    },
    PayloadPair {
        worker_file: "worker/src/handlers/public_event.rs",
        frontend_file: "src/pages/public_event/types.rs",
        frontend_struct: "PublicEventData",
        min_entries: 10,
    },
];

/// Recursively collect `.rs` files under `dir`.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => collect_rs_files(&path, out),
            false => {
                if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
    }
}

/// Names of every `pub fn … -> Option<…>` declared under `domain/src`.
///
/// The signature is matched on a whitespace-flattened copy of the file so a
/// multi-line signature is found too.
fn option_returning_helpers(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_rs_files(&root.join("domain/src"), &mut files);

    let mut out = Vec::new();
    for file in &files {
        let Ok(src) = fs::read_to_string(file) else {
            continue;
        };
        let flat = src.split_whitespace().collect::<Vec<_>>().join(" ");
        for (idx, _) in flat.match_indices("pub fn ") {
            let rest = &flat[idx + "pub fn ".len()..];
            let Some(brace) = rest.find('{') else {
                continue;
            };
            let sig = &rest[..brace];
            if !sig.contains("-> Option<") {
                continue;
            }
            let Some(paren) = sig.find('(') else {
                continue;
            };
            let name = sig[..paren].split('<').next().unwrap_or("").trim();
            if name.is_empty() || name.contains(' ') {
                continue;
            }
            out.push(name.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Every `json!({ … })` block in `src`, brace-matched.
fn json_payloads(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (idx, _) in src.match_indices("json!({") {
        let start = idx + "json!(".len();
        let mut depth = 0usize;
        let mut end = None;
        for (i, ch) in src[start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(e) = end {
            out.push(src[start..e].to_string());
        }
    }
    out
}

/// Single-line `"key": expr` entries of a payload block.
fn payload_entries(payload: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in payload.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix('"') else {
            continue;
        };
        let Some(quote) = rest.find('"') else {
            continue;
        };
        let key = &rest[..quote];
        let Some(expr) = rest[quote + 1..].strip_prefix(':') else {
            continue;
        };
        let expr = expr.trim().trim_end_matches(',').trim();
        if expr.is_empty() {
            continue;
        }
        out.push((key.to_string(), expr.to_string()));
    }
    out
}

/// Declared type of `field` on `struct_name` in `frontend_src`.
fn field_type(frontend_src: &str, struct_name: &str, field: &str) -> Option<String> {
    let needle = format!("pub struct {struct_name} {{");
    let start = frontend_src.find(&needle)?;
    let body = &frontend_src[start..];
    let end = body.find("\n}")?;
    let body = &body[..end];
    let key = format!("pub {field}:");
    let pos = body.find(&key)?;
    let rest = &body[pos + key.len()..];
    let stop = rest.find(',')?;
    Some(rest[..stop].trim().to_string())
}

/// Violations for one payload/client pair.
fn violations(
    payload: &str,
    helpers: &[String],
    frontend_src: &str,
    struct_name: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    for (key, expr) in payload_entries(payload) {
        let Some(helper) = helpers.iter().find(|h| expr.contains(&format!("{h}("))) else {
            continue;
        };
        if expr.contains(".unwrap_or") || expr.contains(".map_or") {
            continue;
        }
        match field_type(frontend_src, struct_name, &key) {
            // Not mirrored by this client — ignoring a payload field is fine.
            None => {}
            Some(ty) if ty.starts_with("Option<") => {}
            Some(ty) => out.push(format!(
                "{struct_name}.{key}: worker calls `{helper}` (returns Option, so \
                 `json!` emits null) but the client declares `{ty}`. \
                 `#[serde(default)]` does not cover an explicit null — parsing the \
                 whole struct fails. Flatten with `.unwrap_or_default()` or declare \
                 the field `Option<{ty}>`."
            )),
        }
    }
    out
}

#[test]
fn nullable_helper_results_are_declared_optional() {
    let root = workspace_root();
    let helpers = option_returning_helpers(&root);
    let mut failures = Vec::new();
    let mut inspected_total = 0usize;

    for pair in PAYLOAD_PAIRS {
        let worker_src = fs::read_to_string(root.join(pair.worker_file))
            .unwrap_or_else(|e| panic!("read {}: {e}", pair.worker_file));
        let frontend_src = fs::read_to_string(Path::new(FRONTEND_ROOT).join(pair.frontend_file))
            .unwrap_or_else(|e| panic!("read {}: {e}", pair.frontend_file));

        // `field_type` returns `None` both for "client does not mirror this
        // field" (fine) and for "the struct was not found" (the guard has gone
        // silent). The payload side has `min_entries`; this is the client-side
        // equivalent, so a rename of the struct fails loudly instead of
        // passing vacuously.
        assert!(
            frontend_src.contains(&format!("pub struct {} {{", pair.frontend_struct)),
            "{}: `pub struct {} {{` not found — every field lookup for this pair \
             would return None and the guard would pass while checking nothing",
            pair.frontend_file,
            pair.frontend_struct
        );

        let payloads = json_payloads(&worker_src);
        assert!(
            !payloads.is_empty(),
            "{}: no `json!({{ … }})` block found — the guard would check nothing",
            pair.worker_file
        );

        let entries: usize = payloads.iter().map(|p| payload_entries(p).len()).sum();
        assert!(
            entries >= pair.min_entries,
            "{}: only {entries} payload entries inspected, floor is {} — the block \
             probably moved and this guard has gone silent",
            pair.worker_file,
            pair.min_entries
        );
        inspected_total += entries;

        for payload in &payloads {
            failures.extend(violations(
                payload,
                &helpers,
                &frontend_src,
                pair.frontend_struct,
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "wire nullability violations ({}):\n  - {}",
        failures.len(),
        failures.join("\n  - ")
    );
    assert!(inspected_total > 0, "guard inspected nothing");
}

#[test]
fn domain_exposes_option_returning_helpers() {
    let helpers = option_returning_helpers(&workspace_root());
    assert!(
        helpers.iter().any(|h| h == "safe_map_url"),
        "`safe_map_url` not found among domain Option-returning helpers ({} found) — \
         the derivation broke and the guard is inspecting an empty helper set",
        helpers.len()
    );
}

// --- Probe validation: the guard must fire, not just stay quiet. ---

#[test]
fn guard_flags_a_bare_option_call() {
    let payload = "{\n    \"location_map_url\": safe_map_url(&e.x),\n}";
    let frontend = "pub struct T {\n    pub location_map_url: String,\n}";
    let found = violations(payload, &["safe_map_url".to_string()], frontend, "T");
    assert_eq!(
        found.len(),
        1,
        "guard failed to flag the known defect: {found:?}"
    );
}

#[test]
fn guard_accepts_flattened_or_optional() {
    let helpers = ["safe_map_url".to_string()];

    let flattened = "{\n    \"location_map_url\": safe_map_url(&e.x).unwrap_or_default(),\n}";
    let as_string = "pub struct T {\n    pub location_map_url: String,\n}";
    assert!(violations(flattened, &helpers, as_string, "T").is_empty());

    let bare = "{\n    \"location_map_url\": safe_map_url(&e.x),\n}";
    let as_option = "pub struct T {\n    pub location_map_url: Option<String>,\n}";
    assert!(violations(bare, &helpers, as_option, "T").is_empty());
}
