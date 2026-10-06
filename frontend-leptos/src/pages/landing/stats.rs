//! `GET /api/public/stats` for the landing, fetched once and shared by every
//! section that shows a number (build plan rule 1: nothing hard-coded). A
//! failed fetch leaves `None`, and each section then hides its numbers rather
//! than show a stale or made-up one.

use event_checkin_domain::models::public_stats::PublicStats;
use leptos::prelude::*;

use crate::api::ApiResponse;
use crate::i18n::Locale;

/// The landing's copy of the stats; `None` until loaded, or if it failed.
#[derive(Clone, Copy)]
pub struct LandingStats(pub ReadSignal<Option<PublicStats>>);

/// Fetch once and put the result in context for the sections below.
pub fn provide_landing_stats() {
    let (stats, set_stats) = signal(None::<PublicStats>);
    provide_context(LandingStats(stats));
    leptos::task::spawn_local(async move {
        let url = format!("{}/public/stats", crate::api::api_base());
        match crate::api::fetch::get(&url, &[]).await {
            Ok(resp) if resp.status() == 200 => {
                match crate::api::fetch::response_json::<ApiResponse<PublicStats>>(&resp).await {
                    Ok(wrapper) => set_stats.set(wrapper.data),
                    Err(e) => log::warn!("[landing] stats parse: {e}"),
                }
            }
            Ok(resp) => log::warn!("[landing] stats returned {}", resp.status()),
            Err(e) => log::warn!("[landing] stats fetch: {e}"),
        }
    });
}

pub fn use_landing_stats() -> ReadSignal<Option<PublicStats>> {
    use_context::<LandingStats>()
        .map(|s| s.0)
        .unwrap_or_else(|| signal(None).0)
}

/// "6 min" under an hour and a half, else whole hours: "67 h".
pub fn duration_label(minutes: u32, locale: Locale) -> String {
    match (minutes < 90, locale) {
        (true, Locale::en) => format!("{minutes} min"),
        (true, Locale::th) => format!("{minutes} นาที"),
        (false, Locale::en) => format!("{} h", (minutes + 30) / 60),
        (false, Locale::th) => format!("{} ชม.", (minutes + 30) / 60),
    }
}

/// `measured_at` (RFC 3339) in the reader's date format; the raw string if it
/// does not parse, so the time is never silently dropped.
pub fn measured_at_label(measured_at: &str) -> String {
    let ms = js_sys::Date::parse(measured_at);
    match ms.is_nan() {
        true => measured_at.to_string(),
        false => crate::utils::format_event_datetime(ms as i64),
    }
}

/// The "of the people who paid a deposit, N of M came" line and its caveat
/// are drawn only when someone paid: "0 of 0 came" reads as a failure, not
/// as a count.
pub fn payers_line_shown(stats: &PublicStats) -> bool {
    stats.deposit_payers > 0
}

/// `34000` → `"34,000"`. Thai uses the same grouping.
pub fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The count-up's curve: fast, then settling (ASKS-4 §3: one ease, no springs).
pub fn ease_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// The number shown `t` of the way through the count-up; exact at the end.
pub fn count_at(value: u64, t: f64) -> u64 {
    match t >= 1.0 {
        true => value,
        false => (value as f64 * ease_out(t)).round() as u64,
    }
}
