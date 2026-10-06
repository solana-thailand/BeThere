//! What crawlers and agents get without running the SPA: `robots.txt`,
//! `sitemap.xml`, `llms.txt`, and a real 404 for paths that are neither a
//! file nor a page.
//!
//! Until 2026-10-06 every unknown path (these three included) answered 200
//! with `index.html`, whose body is empty without JavaScript, so an agent saw
//! only the meta description. `[assets] not_found_handling = "none"` now
//! sends unmatched paths here; [`route_kind`] decides page, document or 404.
//!
//! Wording follows the landing (build plan rules): deposits are THB by
//! PromptPay and the organizer refunds them after the event; the USDC escrow
//! is devnet only. No "automated", no "instantly", no promise about people
//! who cannot come beyond "the organizer sets the rule".

/// The production origin. Only it is open to search engines; staging and
/// local answer `Disallow: /` so they are never indexed next to prod.
pub const CANONICAL_ORIGIN: &str = "https://bethere.solana-thailand.workers.dev";

/// Every page the Leptos router knows (`frontend-leptos/src/lib.rs`), with
/// `:name` for a path parameter. `tests/crawl_routes.rs` pins this list to the
/// router in both directions.
pub const APP_ROUTES: [&str; 23] = [
    "/",
    "/login",
    "/claim/:token",
    "/deposit/:attendee_id",
    "/ticket/:attendee_id",
    "/e/:slug",
    "/past-events",
    "/events/:slug/recap",
    "/events/:slug/post-event-register",
    "/feedback",
    "/discover",
    "/privacy",
    "/faq",
    "/data-privacy",
    "/adventure",
    "/dashboard",
    "/profile",
    "/checkin/nfc",
    "/staff",
    "/admin",
    "/dashboard/live",
    "/events/:id/summary",
    "/events/:id/pr-pack",
];

/// Documents the Worker writes for crawlers and agents.
pub const ROBOTS_PATH: &str = "/robots.txt";
pub const SITEMAP_PATH: &str = "/sitemap.xml";
pub const LLMS_PATH: &str = "/llms.txt";

/// Public pages listed in the sitemap and in `llms.txt`, besides the events.
pub const PUBLIC_PAGES: [(&str, &str); 4] = [
    ("/", "Home"),
    ("/discover", "Discover events"),
    ("/faq", "FAQ"),
    ("/privacy", "Privacy notice"),
];

/// How the Worker answers a path that is not a static file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    /// A page of the app: the SPA shell.
    App,
    /// `/api/*` or a crawler document: the router.
    Worker,
    /// Neither: 404.
    NotFound,
}

pub fn route_kind(path: &str) -> RouteKind {
    if path.starts_with("/api/") || matches!(path, ROBOTS_PATH | SITEMAP_PATH | LLMS_PATH) {
        return RouteKind::Worker;
    }
    match is_app_route(path) {
        true => RouteKind::App,
        false => RouteKind::NotFound,
    }
}

/// Whether `path` is one of [`APP_ROUTES`]. A trailing slash is ignored, a
/// parameter matches one non-empty segment.
pub fn is_app_route(path: &str) -> bool {
    let trimmed = match path {
        "/" => "/",
        p => p.strip_suffix('/').unwrap_or(p),
    };
    let segments: Vec<&str> = trimmed.split('/').collect();
    APP_ROUTES.iter().any(|route| {
        let pattern: Vec<&str> = route.split('/').collect();
        pattern.len() == segments.len()
            && pattern
                .iter()
                .zip(&segments)
                .all(|(want, got)| match want.strip_prefix(':') {
                    Some(_) => !got.is_empty(),
                    None => want == got,
                })
    })
}

/// `robots.txt` for `origin`.
pub fn robots_txt(origin: &str) -> String {
    if origin != CANONICAL_ORIGIN {
        return "User-agent: *\nDisallow: /\n".to_string();
    }
    // Pages that carry a person's id, and the staff side, stay out.
    format!(
        "User-agent: *\n\
         Allow: /\n\
         Disallow: /api/\n\
         Disallow: /ticket/\n\
         Disallow: /deposit/\n\
         Disallow: /claim/\n\
         Disallow: /admin\n\
         Disallow: /staff\n\
         Disallow: /dashboard\n\
         Disallow: /profile\n\
         \n\
         Sitemap: {origin}{SITEMAP_PATH}\n"
    )
}

/// An open event as the crawler documents show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenEvent {
    pub name: String,
    pub slug: String,
    /// Start, already formatted for people (`2026-10-12 18:00 (UTC+7)`).
    pub starts: String,
    pub location: String,
}

/// Events that have a page to link to. The four slug builders keep ASCII
/// only, so an all-Thai event name has an empty slug until build plan item
/// 1.0 lands, and `/e/` with nothing after it is not a page.
fn linkable(events: &[OpenEvent]) -> impl Iterator<Item = &OpenEvent> {
    events.iter().filter(|e| !e.slug.trim().is_empty())
}

/// `sitemap.xml`: the public pages and every open event page.
pub fn sitemap_xml(origin: &str, events: &[OpenEvent]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    let pages = PUBLIC_PAGES
        .iter()
        .map(|(path, _)| format!("{origin}{path}"));
    let event_pages =
        linkable(events).map(|e| format!("{origin}/e/{}", urlencoding::encode(&e.slug)));
    for url in pages.chain(event_pages) {
        out.push_str(&format!("  <url><loc>{}</loc></url>\n", xml_escape(&url)));
    }
    out.push_str("</urlset>\n");
    out
}

/// `llms.txt` (llmstxt.org): what BeThere is, the deposit rule as it is
/// today, and links to the open events.
pub fn llms_txt(origin: &str, events: &[OpenEvent]) -> String {
    let mut out = String::from(
        "# BeThere\n\
         \n\
         > Free events. Hold your seat with a deposit, show up, get it all back.\n\
         \n\
         BeThere is an event check-in app run with Solana Developer Thailand. \
         Events are free to attend; a refundable deposit holds an in-person seat, \
         and the door scan at the event records who came.\n\
         \n\
         How the deposit works today:\n\
         \n\
         - It is paid in Thai baht (THB) by PromptPay to the organizer, who checks the slip.\n\
         - The organizer refunds it after the event.\n\
         - Each organizer sets the rule for people who cannot come.\n\
         - A USDC escrow on Solana exists on devnet only: it moves no real money yet.\n\
         \n\
         ## Open events\n\
         \n",
    );
    let listed: Vec<&OpenEvent> = linkable(events).collect();
    match listed.is_empty() {
        true => out.push_str(&format!(
            "No open events right now. Check [Discover events]({origin}/discover).\n"
        )),
        false => {
            for e in listed {
                let place = match e.location.trim() {
                    "" => String::new(),
                    place => format!(", {place}"),
                };
                out.push_str(&format!(
                    "- [{}]({origin}/e/{}): {}{place}\n",
                    markdown_text(&e.name),
                    urlencoding::encode(&e.slug),
                    e.starts
                ));
            }
        }
    }
    out.push_str("\n## Pages\n\n");
    for (path, title) in PUBLIC_PAGES {
        out.push_str(&format!("- [{title}]({origin}{path})\n"));
    }
    out.push_str(&format!(
        "\n## Optional\n\n- [Open events as JSON]({origin}/api/public/events)\n"
    ));
    out
}

/// The body of the 404 page: plain, no script, a link home.
pub const NOT_FOUND_HTML: &str = "<!doctype html>\n<html lang=\"en\">\n<head>\
<meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<meta name=\"robots\" content=\"noindex\"><title>Not found · BeThere</title></head>\n\
<body style=\"font-family:system-ui,sans-serif;background:#f4f0e6;color:#121212;margin:0;padding:48px 24px\">\
<h1>Not found · ไม่พบหน้านี้</h1>\
<p>This page does not exist. · ไม่มีหน้านี้</p>\
<p><a href=\"/\" style=\"color:#5a54cb\">BeThere home · กลับหน้าแรก</a></p>\
</body>\n</html>\n";

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Organizer text inside a markdown link label: brackets and newlines would
/// break the link.
fn markdown_text(text: &str) -> String {
    text.replace(['[', ']'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
