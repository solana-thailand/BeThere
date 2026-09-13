//! Plan 014 follow-up — mirror **field-type** drift guard (SSOT discipline).
//!
//! Companion to `ssot_mirror_audit.rs`, which compares *predicates* and
//! explicitly cannot see field types. This guard closes that blind spot for
//! the one class of drift that silently corrupts values: **scalar type
//! mismatches** between a frontend mirror struct and its domain SSOT struct.
//!
//! ## The regression this exists for
//!
//! `pages/public_event/types.rs::PublicEventData` mirrored
//! `deposit_amount_usdc` and `deposit_amount_thb` as `f64` while the domain
//! SSOT (`domain::models::event::config::EventConfig`) and the wire payload
//! (`worker/src/handlers/public_event.rs`) both use `u64`. serde happily
//! accepted the integer into an `f64`, so nothing failed — but the frontend
//! then could not tell atomic USDC from whole dollars and shipped a
//! magnitude-guessing formatter (`if val > 1000.0 { val / 1_000_000.0 }`) on
//! the public registration page. Fixed 2026-09-04 (commit `1914308`); this
//! guard makes the class non-recurring.
//!
//! ## Scope — deliberately narrow, deliberately sharp
//!
//! For every pair in [`MIRROR_STRUCT_PAIRS`], every field name present on
//! **both** sides is compared. A pair of types is a violation when either
//! side's core type (after unwrapping `Option<..>`) is a Rust scalar
//! (integer, float, `bool`) and the two core types differ.
//!
//! Out of scope, by construction:
//!
//! - **Non-scalar divergence** — `status: String` mirroring a typed
//!   `EventStatus`, `Vec<CommunityLink>` mirroring `Vec<domain::CommunityLink>`,
//!   nested mirror structs. These are the intentional mirror-type pattern
//!   (defensive `#[serde(default)]` + UI helpers) documented in
//!   `ssot_mirror_audit.rs`; flagging them would bury the scalar signal under
//!   dozens of allowlist entries with the same reason.
//! - **Fields that exist on only one side.** A mirror carrying extra UI-only
//!   fields, or omitting fields it does not render, is normal and cheap to
//!   get right. [`minimum_overlap`] guards the opposite failure — a rename
//!   that silently drops a pair's coverage to zero.
//! - **Optionality** in the safe direction. Frontend `Option<T>` over domain
//!   `T` is the defensive-deserialization pattern and is allowed. The unsafe
//!   direction (domain `Option<T>`, frontend `T`) IS flagged: the field can
//!   legitimately be absent and the mirror would fail to deserialize.
//!
//! ## Run
//!
//! ```sh
//! cargo test --test mirror_field_types
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

/// One frontend mirror struct paired with the domain struct it mirrors.
///
/// `min_shared_fields` is a floor on how many field names must appear on both
/// sides. It fails loudly when a rename on either side quietly reduces the
/// pair to zero comparisons — the guard's silent-death mode.
struct MirrorPair {
    /// Path of the frontend file, relative to `frontend-leptos/`.
    frontend_file: &'static str,
    /// Struct name in the frontend file.
    frontend_struct: &'static str,
    /// Path of the domain file, relative to the workspace root.
    domain_file: &'static str,
    /// Struct name in the domain file.
    domain_struct: &'static str,
    /// Floor on the shared-field count for this pair.
    min_shared_fields: usize,
}

/// The audited mirror pairs. Adding a pair widens coverage; removing one
/// narrows it and needs a documented reason in `.plans/014_ssot_audit.md`.
const MIRROR_STRUCT_PAIRS: &[MirrorPair] = &[
    MirrorPair {
        frontend_file: "src/pages/public_event/types.rs",
        frontend_struct: "PublicEventData",
        domain_file: "domain/src/models/event/config.rs",
        domain_struct: "EventConfig",
        min_shared_fields: 15,
    },
    MirrorPair {
        frontend_file: "src/api/types.rs",
        frontend_struct: "AttendeeListItem",
        domain_file: "domain/src/models/api.rs",
        domain_struct: "AttendeeListItem",
        min_shared_fields: 5,
    },
    MirrorPair {
        frontend_file: "src/api/types.rs",
        frontend_struct: "AttendeeResponse",
        domain_file: "domain/src/models/api.rs",
        domain_struct: "AttendeeResponse",
        min_shared_fields: 5,
    },
    MirrorPair {
        frontend_file: "src/api/types.rs",
        frontend_struct: "StatsResponse",
        domain_file: "domain/src/models/api.rs",
        domain_struct: "StatsResponse",
        min_shared_fields: 4,
    },
    MirrorPair {
        frontend_file: "src/api/types.rs",
        frontend_struct: "RecentCheckIn",
        domain_file: "domain/src/models/api.rs",
        domain_struct: "RecentCheckIn",
        min_shared_fields: 2,
    },
    MirrorPair {
        frontend_file: "src/api/types.rs",
        frontend_struct: "QrGenerationDetail",
        domain_file: "domain/src/models/api.rs",
        domain_struct: "QrGenerationDetail",
        min_shared_fields: 2,
    },
    MirrorPair {
        frontend_file: "src/api/types.rs",
        frontend_struct: "CommunityLink",
        domain_file: "domain/src/models/event/config.rs",
        domain_struct: "CommunityLink",
        min_shared_fields: 2,
    },
    MirrorPair {
        frontend_file: "src/api/event/types.rs",
        frontend_struct: "EventMeta",
        domain_file: "domain/src/models/event/config.rs",
        domain_struct: "EventMeta",
        min_shared_fields: 4,
    },
    MirrorPair {
        frontend_file: "src/api/event/types.rs",
        frontend_struct: "EventDetail",
        domain_file: "domain/src/models/event/config.rs",
        domain_struct: "EventConfig",
        min_shared_fields: 15,
    },
    MirrorPair {
        frontend_file: "src/api/event/types.rs",
        frontend_struct: "CreateEventBody",
        domain_file: "domain/src/models/event/requests.rs",
        domain_struct: "CreateEventRequest",
        min_shared_fields: 10,
    },
    MirrorPair {
        frontend_file: "src/api/event/types.rs",
        frontend_struct: "UpdateEventBody",
        domain_file: "domain/src/models/event/requests.rs",
        domain_struct: "UpdateEventRequest",
        min_shared_fields: 10,
    },
    MirrorPair {
        frontend_file: "src/api/event/summary.rs",
        frontend_struct: "FunnelSnapshotData",
        domain_file: "domain/src/models/event_summary.rs",
        domain_struct: "FunnelSnapshot",
        min_shared_fields: 4,
    },
    MirrorPair {
        frontend_file: "src/api/event/summary.rs",
        frontend_struct: "FinancialSnapshotData",
        domain_file: "domain/src/models/event_summary.rs",
        domain_struct: "FinancialSnapshot",
        min_shared_fields: 3,
    },
    MirrorPair {
        frontend_file: "src/api/event/summary.rs",
        frontend_struct: "EventSummaryPayload",
        domain_file: "domain/src/models/event_summary.rs",
        domain_struct: "EventSummary",
        min_shared_fields: 3,
    },
    MirrorPair {
        frontend_file: "src/api/event/recap.rs",
        frontend_struct: "EventRecapPayload",
        domain_file: "domain/src/models/event_summary.rs",
        domain_struct: "EventRecap",
        min_shared_fields: 3,
    },
    MirrorPair {
        frontend_file: "src/pages/public_event/types.rs",
        frontend_struct: "FormFieldConfig",
        domain_file: "domain/src/models/event/form.rs",
        domain_struct: "FormFieldConfig",
        min_shared_fields: 4,
    },
    MirrorPair {
        frontend_file: "src/pages/public_event/types.rs",
        frontend_struct: "RegistrationFormConfig",
        domain_file: "domain/src/models/event/form.rs",
        domain_struct: "RegistrationFormConfig",
        min_shared_fields: 2,
    },
    MirrorPair {
        frontend_file: "src/api/admin.rs",
        frontend_struct: "FormFieldConfigAdmin",
        domain_file: "domain/src/models/event/form.rs",
        domain_struct: "FormFieldConfig",
        min_shared_fields: 4,
    },
    MirrorPair {
        frontend_file: "src/api/admin.rs",
        frontend_struct: "RegistrationFormConfigAdmin",
        domain_file: "domain/src/models/event/form.rs",
        domain_struct: "RegistrationFormConfig",
        min_shared_fields: 2,
    },
    MirrorPair {
        frontend_file: "src/api/admin.rs",
        frontend_struct: "QuizQuestionAdmin",
        domain_file: "domain/src/models/api.rs",
        domain_struct: "QuizQuestion",
        min_shared_fields: 3,
    },
    MirrorPair {
        frontend_file: "src/api/admin.rs",
        frontend_struct: "QuizConfigAdmin",
        domain_file: "domain/src/models/api.rs",
        domain_struct: "QuizConfig",
        min_shared_fields: 2,
    },
    MirrorPair {
        frontend_file: "src/api/admin.rs",
        frontend_struct: "AdventureConfigData",
        domain_file: "domain/src/models/adventure.rs",
        domain_struct: "AdventureConfig",
        min_shared_fields: 2,
    },
    MirrorPair {
        frontend_file: "src/api/event/pr_pack.rs",
        frontend_struct: "PrPack",
        domain_file: "domain/src/pr_pack.rs",
        domain_struct: "PrPack",
        min_shared_fields: 2,
    },
    MirrorPair {
        frontend_file: "src/api/event/recap.rs",
        frontend_struct: "PublicRecapFunnel",
        domain_file: "domain/src/models/event_summary.rs",
        domain_struct: "FunnelSnapshot",
        min_shared_fields: 2,
    },
];

/// A scalar type divergence that is INTENTIONAL. Every entry must carry a
/// non-empty reason; the manifest self-check rejects placeholders.
///
/// Before adding an entry, ask whether the mirror can simply adopt the domain
/// type. Retyping the mirror is nearly always the right fix — that is what
/// commit `1914308` did.
struct AllowedTypeDivergence {
    frontend_struct: &'static str,
    field: &'static str,
    reason: &'static str,
}

/// Currently empty: every audited pair agrees on every shared scalar field.
/// Kept (rather than deleted) so a future intentional divergence has a
/// documented home instead of a weakened guard.
const ALLOWED_TYPE_DIVERGENCES: &[AllowedTypeDivergence] = &[];

// ---------------------------------------------------------------------------
// Layer 1 — the guard itself
// ---------------------------------------------------------------------------

#[test]
fn mirror_scalar_field_types_match_domain() {
    let mut violations: Vec<String> = Vec::new();

    for pair in MIRROR_STRUCT_PAIRS {
        let frontend = parse_struct_fields(
            &Path::new(FRONTEND_ROOT).join(pair.frontend_file),
            pair.frontend_struct,
        );
        let domain =
            parse_struct_fields(&workspace_root().join(pair.domain_file), pair.domain_struct);

        for (name, frontend_ty) in &frontend {
            let Some((_, domain_ty)) = domain.iter().find(|(n, _)| n == name) else {
                continue;
            };
            if let Some(kind) = scalar_divergence(frontend_ty, domain_ty) {
                let allowed = ALLOWED_TYPE_DIVERGENCES
                    .iter()
                    .any(|a| a.frontend_struct == pair.frontend_struct && a.field == name);
                if !allowed {
                    violations.push(format!(
                        "  - {}::{name}: frontend `{frontend_ty}` vs domain \
                         {}::{}::{name} `{domain_ty}` ({kind})",
                        pair.frontend_struct, pair.domain_file, pair.domain_struct
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "frontend mirror struct diverges from the domain SSOT on a scalar \
         field type.\n\n\
         serde will silently accept most of these (an integer deserializes \
         into an f64 without error), so the drift shows up as wrong numbers \
         in the UI, not as a failure. Either:\n  \
         (a) retype the mirror field to match the domain type (preferred), or\n  \
         (b) add the field to ALLOWED_TYPE_DIVERGENCES in this test with a \
         non-empty reason.\n\n\
         Divergences:\n{}",
        violations.join("\n")
    );
}

// ---------------------------------------------------------------------------
// Layer 2 — coverage floors (the guard's silent-death mode)
// ---------------------------------------------------------------------------

#[test]
fn every_pair_shares_a_minimum_number_of_fields() {
    for pair in MIRROR_STRUCT_PAIRS {
        let frontend = parse_struct_fields(
            &Path::new(FRONTEND_ROOT).join(pair.frontend_file),
            pair.frontend_struct,
        );
        let domain =
            parse_struct_fields(&workspace_root().join(pair.domain_file), pair.domain_struct);
        let shared = frontend
            .iter()
            .filter(|(n, _)| domain.iter().any(|(dn, _)| dn == n))
            .count();

        assert!(
            shared >= pair.min_shared_fields,
            "pair {} <-> {}::{} shares only {shared} field names \
             (floor {}). A rename on either side has silently cut this \
             pair's coverage — realign the field names or update the floor \
             with a documented reason.",
            pair.frontend_struct,
            pair.domain_file,
            pair.domain_struct,
            pair.min_shared_fields
        );
    }
}

#[test]
fn manifest_entries_are_well_formed() {
    assert!(
        !MIRROR_STRUCT_PAIRS.is_empty(),
        "pair manifest is empty — the guard would pass vacuously"
    );

    let mut seen: Vec<(&str, &str)> = Vec::new();
    for pair in MIRROR_STRUCT_PAIRS {
        let key = (pair.frontend_struct, pair.domain_struct);
        assert!(
            !seen.contains(&key),
            "duplicate pair entry for {} <-> {}",
            pair.frontend_struct,
            pair.domain_struct
        );
        seen.push(key);
    }

    for entry in ALLOWED_TYPE_DIVERGENCES {
        assert!(
            !entry.reason.is_empty() && entry.reason != "unspecified",
            "divergence allowlist entry {}::{} has an empty or placeholder \
             reason — every intentional divergence must say why",
            entry.frontend_struct,
            entry.field
        );
        assert!(
            MIRROR_STRUCT_PAIRS
                .iter()
                .any(|p| p.frontend_struct == entry.frontend_struct),
            "divergence allowlist entry names struct `{}`, which is not in \
             MIRROR_STRUCT_PAIRS — the entry can never apply",
            entry.frontend_struct
        );
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Rust scalar type names. A divergence is only reported when at least one
/// side is one of these — see the module docs for why non-scalars are out of
/// scope.
const SCALAR_TYPES: &[&str] = &[
    "bool", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
    "u128", "usize",
];

fn is_scalar(ty: &str) -> bool {
    SCALAR_TYPES.contains(&ty)
}

/// Strip one layer of `Option<..>`, returning the inner type and whether a
/// layer was removed.
fn unwrap_option(ty: &str) -> (&str, bool) {
    match ty.strip_prefix("Option<").and_then(|r| r.strip_suffix('>')) {
        Some(inner) => (inner, true),
        None => (ty, false),
    }
}

/// Classify the divergence between a frontend and a domain field type.
/// Returns `None` when the pair is fine or out of scope.
fn scalar_divergence(frontend_ty: &str, domain_ty: &str) -> Option<&'static str> {
    let (frontend_core, frontend_opt) = unwrap_option(frontend_ty);
    let (domain_core, domain_opt) = unwrap_option(domain_ty);

    match (
        is_scalar(frontend_core) || is_scalar(domain_core),
        frontend_core == domain_core,
        domain_opt && !frontend_opt,
    ) {
        // Neither side is a scalar — out of scope (see module docs).
        (false, _, _) => None,
        // Core types differ: the value-corrupting case (u64 read as f64, ...).
        (true, false, _) => Some("scalar type mismatch"),
        // Same core type, but domain says the field may be absent while the
        // mirror demands it — deserialization can fail at runtime.
        (true, true, true) => Some("domain field is optional, mirror is not"),
        (true, true, false) => None,
    }
}

/// Extract `(field_name, type)` pairs from the named struct in `path`.
///
/// Deliberately strict: it panics when the struct is missing or when a `pub`
/// field line cannot be parsed, so a layout change fails loudly instead of
/// quietly reducing coverage to zero.
fn parse_struct_fields(path: &Path, struct_name: &str) -> Vec<(String, String)> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));

    let header = format!("pub struct {struct_name}");
    let mut lines = source.lines();
    let found = lines.by_ref().any(|line| {
        line.trim_start().starts_with(&header)
            && line.trim_start().get(header.len()..).is_some_and(|rest| {
                rest.starts_with(' ') || rest.starts_with('{') || rest.starts_with('<')
            })
    });
    assert!(
        found,
        "struct `{struct_name}` not found in {} — the manifest has drifted \
         from the source layout",
        path.display()
    );

    let mut fields = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "}" {
            break;
        }
        let Some(rest) = trimmed.strip_prefix("pub ") else {
            continue;
        };
        let parsed = parse_field_decl(rest);
        let (name, ty) = parsed.unwrap_or_else(|| {
            panic!(
                "could not parse field declaration `{trimmed}` in \
                 `{struct_name}` ({}) — extend parse_field_decl rather than \
                 letting the field drop out of the audit",
                path.display()
            )
        });
        fields.push((name, ty));
    }

    assert!(
        !fields.is_empty(),
        "struct `{struct_name}` in {} parsed to zero fields",
        path.display()
    );
    fields
}

/// Parse `name: Type,` (the remainder of a line after `pub `) into its parts.
/// Whitespace inside the type is collapsed so `Vec < u8 >` and `Vec<u8>`
/// compare equal.
fn parse_field_decl(rest: &str) -> Option<(String, String)> {
    let (name, ty) = rest.split_once(':')?;
    let name = name.trim();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let ty = ty.trim().trim_end_matches(',').trim();
    if ty.is_empty() {
        return None;
    }
    let ty: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
    Some((name.to_string(), ty))
}

// ---------------------------------------------------------------------------
// Self-tests — prove the comparison logic catches the real regression.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod self_tests {
    use super::*;

    #[test]
    fn the_public_event_data_regression_would_be_caught() {
        // The exact drift shipped before commit 1914308.
        assert_eq!(
            scalar_divergence("f64", "u64"),
            Some("scalar type mismatch")
        );
    }

    #[test]
    fn integer_width_drift_is_caught() {
        assert_eq!(
            scalar_divergence("u32", "u64"),
            Some("scalar type mismatch")
        );
        assert_eq!(
            scalar_divergence("Option<i64>", "Option<u64>"),
            Some("scalar type mismatch")
        );
    }

    #[test]
    fn defensive_optionality_is_allowed_but_the_unsafe_direction_is_not() {
        // Frontend widens to Option<T> for partial-JSON safety: fine.
        assert_eq!(scalar_divergence("Option<u64>", "u64"), None);
        // Domain says the field may be absent, mirror demands it: not fine.
        assert_eq!(
            scalar_divergence("u64", "Option<u64>"),
            Some("domain field is optional, mirror is not")
        );
    }

    #[test]
    fn non_scalar_divergence_is_out_of_scope() {
        assert_eq!(scalar_divergence("String", "EventStatus"), None);
        assert_eq!(
            scalar_divergence("Vec<CommunityLink>", "Vec<crate::CommunityLink>"),
            None
        );
        assert_eq!(scalar_divergence("bool", "bool"), None);
    }

    #[test]
    fn field_declarations_parse_into_name_and_type() {
        assert_eq!(
            parse_field_decl("deposit_amount_usdc: u64,"),
            Some(("deposit_amount_usdc".to_string(), "u64".to_string()))
        );
        assert_eq!(
            parse_field_decl("links: Vec< CommunityLink >,"),
            Some(("links".to_string(), "Vec<CommunityLink>".to_string()))
        );
        assert_eq!(parse_field_decl("fn helper() {"), None);
    }
}
