//! `.issues/114` — R2 objects are cached according to what they are.
//!
//! `serve_r2_object` used to send `Cache-Control: public, max-age=86400` for
//! every prefix, so staff-only payment slips and refund receipts told shared
//! caches they could keep a copy (RFC 9111 §3.5). Posters, meanwhile, carried
//! no validator, so a 3.9 MB poster was downloaded again in full every day.

use event_checkin_worker::storage::{Visibility, if_none_match_hits};

#[test]
fn private_documents_are_never_stored() {
    let value = Visibility::Private.cache_control();
    assert!(value.contains("no-store"), "{value}");
    assert!(value.contains("private"), "{value}");
    assert!(!value.contains("public"), "{value}");
}

#[test]
fn public_assets_stay_cacheable() {
    assert_eq!(Visibility::Public.cache_control(), "public, max-age=86400");
}

#[test]
fn if_none_match_uses_weak_comparison_over_a_list() {
    let etag = "\"abc123\"";
    assert!(if_none_match_hits("\"abc123\"", etag));
    assert!(if_none_match_hits("W/\"abc123\"", etag));
    assert!(if_none_match_hits("\"old\", \"abc123\"", etag));
    assert!(if_none_match_hits("*", etag));
    assert!(!if_none_match_hits("\"old\"", etag));
    assert!(!if_none_match_hits("", etag));
    assert!(
        !if_none_match_hits("abc123", etag),
        "quotes are part of the tag"
    );
}

/// The policy is only as good as its call sites: each route must name its
/// visibility, and the financial routes must name `Private`.
#[test]
fn every_route_names_its_visibility() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/storage.rs"))
        .expect("worker/src/storage.rs must be readable");
    let route = |name: &str| {
        let start = source
            .find(&format!("pub async fn {name}("))
            .unwrap_or_else(|| panic!("{name} not found"));
        let end = source[start..].find("\n}\n").expect("fn end") + start;
        source[start..end].to_string()
    };
    for private in ["serve_slip", "serve_refund"] {
        assert!(
            route(private).contains("Visibility::Private"),
            "{private} must be Private"
        );
    }
    for public in ["serve_poster", "serve_badge"] {
        assert!(
            route(public).contains("Visibility::Public"),
            "{public} must be Public"
        );
    }
    let code: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        code.matches("public, max-age").count(),
        1,
        "a cache directive outside `Visibility::cache_control` bypasses the policy"
    );
}
