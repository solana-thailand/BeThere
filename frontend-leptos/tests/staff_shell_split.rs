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

/// The staff wasm stays small only while it carries no attendee page: lib.rs
/// takes every page except `/login` from `staff_routes`, which hands them off
/// to the attendee shell in the staff build. A page imported straight from
/// `crate::pages` would link it back into the staff wasm.
#[test]
fn lib_rs_takes_only_login_straight_from_pages() {
    let direct: Vec<&str> = LIB_RS
        .lines()
        .filter(|l| l.trim_start().starts_with("use crate::pages"))
        .collect();
    assert_eq!(
        direct,
        ["use crate::pages::login::Login;"],
        "route pages go through staff_routes so the staff build can hand them off"
    );
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

/// index.html links the attendee stylesheets: every `styles/style-*.css`
/// except the `.staff.css` ones, sorted, because the prefix is the cascade
/// order. `staff_shell_html.py` builds the staff list from the same directory,
/// so an unlinked or misordered sheet here would differ between the shells.
#[test]
fn index_html_links_every_attendee_stylesheet_in_sorted_order() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("styles");
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .expect("read styles/")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.starts_with("style-") && n.ends_with(".css") && !n.ends_with(".staff.css"))
        .collect();
    on_disk.sort();
    let linked: Vec<&str> = INDEX_HTML
        .lines()
        .filter_map(|l| {
            l.trim()
                .strip_prefix(r#"<link data-trunk rel="css" href="styles/"#)?
                .strip_suffix(r#"" />"#)
        })
        .collect();
    assert_eq!(
        linked, on_disk,
        "index.html stylesheet links drifted from styles/"
    );
    assert!(
        on_disk.len() < std::fs::read_dir(&dir).expect("read styles/").count(),
        "styles/ must still hold the staff-only sheets"
    );
    assert!(BUILD_SH.contains("staff_shell_html.py") && BUILD_SH.contains("staff-shell.html"));
}
