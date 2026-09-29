//! The attendee/staff shell split (.issues/169) is held together by three
//! lists: `STAFF_PATHS`, the `_redirects` 200 rewrites that serve the staff
//! shell, and the staff routes in `lib.rs`. A path missing from `_redirects`
//! loads the attendee shell, which cannot render that page.

use event_checkin_frontend::staff_routes::STAFF_PATHS;

const REDIRECTS: &str = include_str!("../_redirects");
const LIB_RS: &str = include_str!("../src/lib.rs");
const INDEX_HTML: &str = include_str!("../index.html");
const BUILD_SH: &str = include_str!("../build.sh");

/// `(from, to, status)` for every rule line in `_redirects`.
fn redirect_rules() -> Vec<(&'static str, &'static str, &'static str)> {
    REDIRECTS
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            assert_eq!(parts.len(), 3, "_redirects rule needs from/to/status: {l}");
            (parts[0], parts[1], parts[2])
        })
        .collect()
}

#[test]
fn redirects_serve_the_staff_shell_for_exactly_the_staff_paths() {
    let rules = redirect_rules();
    let from: Vec<&str> = rules.iter().map(|(f, _, _)| *f).collect();
    assert_eq!(
        from, STAFF_PATHS,
        "_redirects and STAFF_PATHS drifted apart"
    );
    for (path, to, status) in rules {
        assert_eq!(to, "/staff-app", "{path} must rewrite to the staff shell");
        assert_eq!(status, "200", "{path} must be a rewrite, not a redirect");
    }
}

#[test]
fn every_staff_path_is_a_route_backed_by_staff_routes() {
    for path in STAFF_PATHS {
        let route = format!("<Route path=path!(\"{path}\") view=Protected");
        assert!(
            LIB_RS.contains(&route),
            "lib.rs has no staff route for {path}"
        );
    }
}

#[test]
fn redirects_ship_and_build_produces_the_staff_shell() {
    assert!(
        INDEX_HTML.contains(r#"<link data-trunk rel="copy-file" href="_redirects" />"#),
        "index.html must copy _redirects into dist/"
    );
    assert!(BUILD_SH.contains("--features staff"));
    assert!(BUILD_SH.contains("staff-app.html"));
}
