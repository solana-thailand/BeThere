//! The 404 rule and the crawler documents (`src/crawl.rs`).
//!
//! A path that is not a file and not a page now answers 404 instead of the
//! SPA shell with a 200. The page list must match the Leptos router exactly:
//! a page missing here would 404 on a deep link, an extra one would serve a
//! blank shell.

use event_checkin_worker::crawl::{
    APP_ROUTES, CANONICAL_ORIGIN, OpenEvent, RouteKind, is_app_route, llms_txt, robots_txt,
    route_kind, sitemap_xml,
};

const ROUTER: &str = include_str!("../../frontend-leptos/src/lib.rs");

fn router_paths() -> Vec<String> {
    ROUTER
        .split("path!(\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_string)
        .collect()
}

#[test]
fn page_list_matches_the_leptos_router_both_ways() {
    let mut router = router_paths();
    let mut ours: Vec<String> = APP_ROUTES.iter().map(|r| r.to_string()).collect();
    router.sort();
    ours.sort();
    assert!(router.len() > 10, "router parse broke: {router:?}");
    assert_eq!(ours, router);
}

#[test]
fn deep_links_are_pages() {
    for path in [
        "/",
        "/discover",
        "/discover/",
        "/feedback",
        "/e/rtm-6",
        "/e/event-a1b2c3",
        "/admin",
        "/staff",
        "/ticket/0190e2e0-0000-7000-8000-000000000001",
        "/events/rtm-6/recap",
        "/events/abc/summary",
        "/dashboard/live",
    ] {
        assert_eq!(route_kind(path), RouteKind::App, "{path}");
    }
}

#[test]
fn unknown_paths_are_404() {
    for path in [
        "/.well-known/mcp.json",
        "/wp-login.php",
        "/e",
        "/e/",
        "/e/a/b",
        "/discoverx",
        "/events/x",
        "/index.php",
    ] {
        assert_eq!(route_kind(path), RouteKind::NotFound, "{path}");
        assert!(!is_app_route(path), "{path}");
    }
}

#[test]
fn api_and_documents_go_to_the_router() {
    for path in ["/api/health", "/robots.txt", "/sitemap.xml", "/llms.txt"] {
        assert_eq!(route_kind(path), RouteKind::Worker, "{path}");
    }
}

#[test]
fn only_prod_is_open_to_search() {
    let prod = robots_txt(CANONICAL_ORIGIN);
    assert!(prod.contains("Allow: /\n"));
    assert!(prod.contains(&format!("Sitemap: {CANONICAL_ORIGIN}/sitemap.xml")));
    for private in ["/ticket/", "/deposit/", "/claim/", "/api/", "/admin"] {
        assert!(prod.contains(&format!("Disallow: {private}")), "{private}");
    }
    let staging = robots_txt("https://bethere-staging.solana-thailand.workers.dev");
    assert_eq!(staging, "User-agent: *\nDisallow: /\n");
}

fn sample() -> Vec<OpenEvent> {
    vec![OpenEvent {
        name: "RTM [#7] <Bangkok>".to_string(),
        slug: "rtm-7".to_string(),
        starts: "2026-11-01 18:00 (UTC+7)".to_string(),
        location: "Bangkok".to_string(),
    }]
}

#[test]
fn sitemap_lists_pages_and_open_events_escaped() {
    let xml = sitemap_xml(CANONICAL_ORIGIN, &sample());
    assert!(xml.starts_with("<?xml"));
    assert!(xml.contains(&format!("<loc>{CANONICAL_ORIGIN}/</loc>")));
    assert!(xml.contains(&format!("<loc>{CANONICAL_ORIGIN}/discover</loc>")));
    assert!(xml.contains(&format!("<loc>{CANONICAL_ORIGIN}/e/rtm-7</loc>")));
    let thai = OpenEvent {
        slug: "งาน".to_string(),
        ..sample()[0].clone()
    };
    assert!(sitemap_xml(CANONICAL_ORIGIN, &[thai]).contains("/e/%E0%B8%87"));
}

/// The landing's wording rules hold in `llms.txt` too.
#[test]
fn llms_txt_states_today_s_rule_and_no_promises() {
    let text = llms_txt(CANONICAL_ORIGIN, &sample());
    assert!(text.starts_with("# BeThere\n\n> "));
    for fact in ["PromptPay", "refunds it after the event", "devnet only"] {
        assert!(text.contains(fact), "{fact}");
    }
    for banned in [
        "automated",
        "automatic",
        "instantly",
        "never forfeited",
        "credit",
    ] {
        assert!(
            !text.to_lowercase().contains(banned),
            "llms.txt says {banned:?}"
        );
    }
    assert!(text.contains(&format!(
        "- [RTM #7 <Bangkok>]({CANONICAL_ORIGIN}/e/rtm-7): 2026-11-01 18:00 (UTC+7), Bangkok"
    )));
    assert!(llms_txt(CANONICAL_ORIGIN, &[]).contains("No open events right now."));
}
