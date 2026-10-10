//! `/` from the Worker (.plans/045 R4.9): the open events from D1 (the same
//! read and rule as `GET /api/public/events`), spliced into the shell by
//! `home.rs`. Any miss serves the stock shell: the static home must never
//! cost the page itself.

use worker::Env;

use crate::home::{HOME_HTML_BUDGET, home_events, keep_open_public, splice_summary, summary_html};

/// The home with its server-rendered opening, or `None` for the stock page.
pub async fn render(env: &Env, stock: &str) -> Option<String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut events = match env.d1("DB") {
        Ok(d1) => match crate::db::events::list_public_events_raw(&d1, now_ms).await {
            Ok(rows) => rows,
            Err(e) => {
                tracing::warn!(error = %e, "home: open events read failed, listing none");
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    };
    keep_open_public(&mut events, now_ms);
    let page = splice_summary(stock, &summary_html(&home_events(&events)))?;
    match page.len() <= HOME_HTML_BUDGET {
        true => Some(page),
        false => {
            tracing::warn!(
                bytes = page.len(),
                "home: over the 20 KB budget, serving the stock page"
            );
            None
        }
    }
}
