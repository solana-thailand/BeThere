//! The static home (.plans/045 R4.9): `/` from the Worker carries the
//! promise, the payers who came (the hall's own numbers), the open events or
//! the cadence line, stays under 20 KB, escapes what organizers typed, and
//! keeps the block the app removes when it mounts.

use event_checkin_domain::models::facts::ladder_total;
use event_checkin_worker::home::{
    HOME_HTML_BUDGET, HomeEvent, home_events, keep_open_public, splice_summary, summary_html,
};
use serde_json::json;

fn index_html() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../frontend-leptos/index.html"
    ))
    .unwrap()
}

fn event(i: i64) -> HomeEvent {
    HomeEvent {
        name: format!("Solana x AI Builders: The Road to Mainnet #{i} (Bangkok) — a long title"),
        slug: format!("rtm-{i}"),
        start_ms: 1_793_512_800_000 + i * 86_400_000,
        location: "FabCafe Bangkok: 3F, Rear Wing · Bangkok General Post Office".into(),
    }
}

#[test]
fn same_numbers_as_the_hall_and_the_promise() {
    let (paid, came) = ladder_total();
    let html = summary_html(&[]);
    assert!(html.contains(&format!("<strong>{came}/{paid}</strong>")));
    assert!(html.contains("Show up. Get it all back."));
    // nothing open: the cadence line from the catalogue
    assert!(
        html.contains("Road to Mainnet has run 6 times, about every 4 weeks"),
        "{html}"
    );
}

#[test]
fn open_events_are_links_and_what_organizers_typed_is_escaped() {
    let mut evil = event(1);
    evil.name = "<script>alert(1)</script> & \"x\"".into();
    let html = summary_html(&[evil, event(2)]);
    assert!(html.contains(
        "<a href=\"/e/rtm-1\">&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;x&quot;</a>"
    ));
    assert!(html.contains("(Bangkok, GMT+7)"));
    assert!(!html.contains("<script>"));
}

#[test]
fn spliced_page_keeps_the_block_and_the_budget() {
    let stock = index_html();
    // the stock block has no nested div, so the first </div> closes it
    let block = &stock[stock.find("<div id=\"boot-summary\"").unwrap()..];
    let inner = &block[..block.find("</div>").unwrap()];
    assert!(
        !inner[1..].contains("<div"),
        "boot-summary must not nest a div"
    );
    let events: Vec<HomeEvent> = (1..=8).map(event).collect();
    let page = splice_summary(&stock, &summary_html(&events)).unwrap();
    assert_eq!(page.matches("id=\"boot-summary\"").count(), 1);
    assert!(
        page.contains("rtm-5") && !page.contains("rtm-6"),
        "five listed at most"
    );
    assert!(page.len() <= HOME_HTML_BUDGET, "{} bytes", page.len());
    assert!(page.contains("<link data-trunk") || page.contains("</body>"));
    assert!(splice_summary("<html>no block</html>", "x").is_none());
}

#[test]
fn one_rule_for_open_events() {
    let now = 1_800_000_000_000;
    let mut rows = vec![
        json!({"slug": "later", "status": "active", "visibility": "public", "event_start_ms": now + 9, "event_end_ms": now + 10}),
        json!({"slug": "draft", "status": "draft", "visibility": "public", "event_start_ms": now, "event_end_ms": now + 10}),
        json!({"slug": "private", "status": "active", "visibility": "private", "event_start_ms": now, "event_end_ms": now + 10}),
        json!({"slug": "over", "status": "active", "visibility": "public", "event_start_ms": now - 9, "event_end_ms": now - 1}),
        json!({"slug": "soon", "status": "active", "visibility": "public", "event_start_ms": now + 1, "event_end_ms": now + 10}),
        json!({"slug": "bad slug!", "status": "active", "visibility": "public", "event_start_ms": now + 2, "event_end_ms": now + 10}),
    ];
    keep_open_public(&mut rows, now);
    let slugs: Vec<String> = home_events(&rows).into_iter().map(|e| e.slug).collect();
    assert_eq!(slugs, ["soon", "later"]);
    // the API uses the same function
    let api = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/handlers/public_event.rs"
    ))
    .unwrap();
    assert!(api.contains("crate::home::keep_open_public(&mut public_events, now_ms)"));
}
