//! The static home (.plans/045 R4.9): what `/` says before the wasm boots,
//! and all a crawler, an agent or a no-JS reader ever sees. The Worker
//! serves `/` (`run_worker_first`) with the embedded `index.html`, whose
//! `#boot-summary` block is replaced here by the promise, the payers who came
//! (the same `facts::ladder_total` the hall draws) and the open events, else
//! how often the busiest course runs (from D1, `courses.rs`). The app removes the block when it mounts
//! (`remove_boot_summary`), exactly as it removes the stock one.
//!
//! Pure (tested natively in `tests/static_home.rs`); `home_page.rs` reads the
//! open events and calls it.

use event_checkin_domain::models::course::Cadence;
use event_checkin_domain::models::facts::ladder_total;
use serde_json::Value;

use crate::og_meta::html_escape;
use crate::subscribers::copy::when;

/// The page budget (.plans/045 R4.9): the whole HTML under 20 KB.
pub const HOME_HTML_BUDGET: usize = 20 * 1024;
/// Open events listed; the rest are a link away.
const LISTED: usize = 5;
const BLOCK_START: &str = "<div id=\"boot-summary\"";

/// One open event as the static home lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomeEvent {
    pub name: String,
    pub slug: String,
    pub start_ms: i64,
    pub location: String,
}

/// Keep the active, public events that have not ended, nearest first: the
/// rule of `GET /api/public/events`, which calls this too.
pub fn keep_open_public(events: &mut Vec<Value>, now_ms: i64) {
    events.retain(|e| {
        let status = e.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let end_ms = e.get("event_end_ms").and_then(|v| v.as_i64()).unwrap_or(0);
        let vis = e.get("visibility").and_then(|v| v.as_str()).unwrap_or("");
        status == "active" && end_ms > now_ms && vis == "public"
    });
    events.sort_by_key(|e| {
        e.get("event_start_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(i64::MAX)
    });
}

/// The open events (already filtered and sorted) as the home lists them.
pub fn home_events(events: &[Value]) -> Vec<HomeEvent> {
    let text = |e: &Value, k: &str| e.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    events
        .iter()
        .map(|e| HomeEvent {
            name: text(e, "name"),
            slug: text(e, "slug"),
            start_ms: e
                .get("event_start_ms")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            location: text(e, "location"),
        })
        .filter(|e| !e.slug.is_empty() && crate::og_meta::is_safe_id(&e.slug))
        .collect()
}

/// The block that replaces `#boot-summary`. Same id and the same inline
/// style as the stock block, so the app removes it and nothing jumps.
/// `quiet` is the busiest course's title and cadence, for when nothing is open.
pub fn summary_html(events: &[HomeEvent], quiet: Option<(&str, Cadence)>) -> String {
    let (paid, came) = ladder_total();
    let open = match events.is_empty() {
        true => match quiet {
            Some((title, c)) => format!(
                "<p>Nothing open yet. {} has run {} times, about every {} weeks, last on {}.</p>",
                html_escape(title),
                c.times,
                c.every_weeks,
                html_escape(&when(c.last_ms, false))
            ),
            None => "<p>Nothing open yet.</p>".to_string(),
        },
        false => {
            let items: String = events
                .iter()
                .take(LISTED)
                .map(|e| {
                    let place = match e.location.trim() {
                        "" => String::new(),
                        loc => format!(" · {}", html_escape(loc)),
                    };
                    format!(
                        "<li><a href=\"/e/{}\">{}</a> · {}{place}</li>",
                        html_escape(&e.slug),
                        html_escape(&e.name),
                        html_escape(&when(e.start_ms, false))
                    )
                })
                .collect();
            format!("<ul>{items}</ul>")
        }
    };
    format!(
        "{BLOCK_START} style=\"max-width:640px;margin:0 auto;padding:48px 24px;font-family:system-ui,sans-serif;color:#121212\">\
<h1>Show up. Get it all back.</h1>\
<p>Free events. Hold your seat with a deposit, show up, get it all back.</p>\
<p><strong>{came}/{paid}</strong> who paid a deposit came (RTM #1–#6, staff not counted).</p>\
<h2>Open events</h2>{open}\
<p>Today the deposit is paid in Thai baht by PromptPay, and the organizer refunds it after the event. The USDC escrow on Solana runs on devnet only.</p>\
<p><a href=\"/events\">Events and courses</a> · <a href=\"/organizers\">For organizers</a> · <a href=\"/sponsors\">For sponsors</a> · <a href=\"/llms.txt\">llms.txt</a></p>\
</div>"
    )
}

/// `index_html` with its `#boot-summary` block replaced by `summary`, or
/// `None` if the block is missing (the stock page is served then).
pub fn splice_summary(index_html: &str, summary: &str) -> Option<String> {
    let start = index_html.find(BLOCK_START)?;
    let end = start + index_html[start..].find("</div>")? + "</div>".len();
    Some(format!(
        "{}{summary}{}",
        &index_html[..start],
        &index_html[end..]
    ))
}
