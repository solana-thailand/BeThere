//! The Release 4 site pages (.plans/045 R4.0): each one is a route, the
//! framed ones carry the language switch, the doors never point at the page
//! you are on, and nothing links to the devnet sandbox before it exists.

use event_checkin_frontend::locale::FRAMED_PATHS;
use event_checkin_frontend::pages::site::doors::{
    SitePage, TRY_LIVE, doors_from, try_band, try_line,
};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{ROOT}/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn router_paths() -> Vec<String> {
    read("src/lib.rs")
        .split("path!(\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next().map(str::to_owned))
        .collect()
}

#[test]
fn every_site_page_is_a_route() {
    let router = router_paths();
    assert!(router.len() > 10, "router parse broke: {router:?}");
    for page in SitePage::ALL {
        assert!(
            router.iter().any(|p| p == page.path()),
            "{} is not in the router",
            page.path()
        );
    }
}

#[test]
fn doors_show_every_other_page_in_order() {
    for here in SitePage::ALL {
        let doors: Vec<_> = doors_from(here).collect();
        assert_eq!(doors.len(), SitePage::ALL.len() - 1);
        assert!(!doors.contains(&here), "{here:?} links to itself");
        let expected: Vec<_> = SitePage::ALL.into_iter().filter(|p| *p != here).collect();
        assert_eq!(doors, expected);
    }
}

/// `FRAMED_PATHS` (no language bar, the switch is in the header) must be
/// exactly the pages that render inside `SiteFrame`, read from the sources.
#[test]
fn framed_paths_match_the_pages_in_the_frame() {
    let mut sources = String::new();
    for dir in ["src/pages/landing", "src/pages/site"] {
        for entry in std::fs::read_dir(format!("{ROOT}/{dir}"))
            .expect("dir")
            .flatten()
        {
            sources.push_str(&std::fs::read_to_string(entry.path()).unwrap_or_default());
        }
    }
    let mut framed: Vec<&str> = SitePage::ALL
        .into_iter()
        .filter(|p| sources.contains(&format!("<SiteFrame here=SitePage::{p:?}")))
        .map(SitePage::path)
        .collect();
    let mut expected = FRAMED_PATHS.to_vec();
    framed.sort_unstable();
    expected.sort_unstable();
    assert_eq!(framed, expected);
}

/// SG3 (the devnet sandbox) is not built: no `/try` route, and so no band
/// or line may link to it.
#[test]
fn nothing_links_to_the_sandbox_before_it_exists() {
    assert!(try_band(false).is_none());
    assert!(try_line(false).is_none());
    let has_try_route = router_paths().iter().any(|p| p == "/try");
    assert_eq!(
        TRY_LIVE, has_try_route,
        "TRY_LIVE must flip together with the /try route"
    );
}

/// `/sponsors` keeps the `#contact` card, so old `/#contact` links have a
/// page to land on.
#[test]
fn sponsors_page_keeps_the_contact_anchor() {
    assert!(read("src/pages/site/sponsors.rs").contains("<Sponsors headless=true />"));
    assert!(read("src/pages/landing/sponsors.rs").contains("id=\"contact\""));
}
