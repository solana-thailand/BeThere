//! Live aggregate dashboard page — big-screen view for the in-room demo.
//!
//! Route: `/dashboard/live?event_id={id}`
//!
//! Polls `GET /api/dashboard/live` every 2.5 seconds and renders:
//!   - Headline tiles: registered, deposits, USDC locked, checked-in, NFT claims
//!   - A registration → deposit → check-in → claim funnel
//!   - A live activity feed sourced from the audit log
//!
//! Designed to be displayed full-screen on the demo room projector. All API
//! requests bypass the browser HTTP cache via `api_get_no_cache`, so the room
//! always sees the freshest D1 snapshot.
//!
//! Auth: registered behind `ProtectedRoute` in `lib.rs`, so staff JWT is
//! enforced before this component mounts.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_query_map;

use crate::api::{
    self, action_emoji, format_usdc, ActivityEntry, DashboardTotals, FunnelStage,
    LiveDashboardResponse,
};
use wasm_bindgen::JsValue;

/// Poll interval for the live dashboard. Tuned for "feels live" UX without
/// hammering D1 — a demo room of 40 people produces <1 mutation/sec, so 2.5s
/// polling catches every meaningful state change within one render frame.
const POLL_INTERVAL_MS: u32 = 2500;

/// Coarse tick for "Xs ago" badge refresh. Decoupled from `POLL_INTERVAL_MS`
/// so the age badge visibly updates between polls (otherwise a 4s-old poll
/// would display "0s" until the next fetch lands).
const AGE_TICK_MS: u32 = 1000;

/// Coarse load state surfaced to the UI. Distinct from per-poll success/failure:
/// once we have *any* data, transient poll errors keep the last good snapshot
/// on screen rather than blanking the dashboard mid-demo.
#[derive(Debug, Clone, PartialEq)]
enum DashboardLoadState {
    /// No fetch has been initiated yet.
    Idle,
    /// Initial load in progress (no data on screen yet).
    Loading,
    /// At least one successful fetch has landed.
    Loaded,
    /// The most recent poll failed. The UI may still be showing stale data.
    Failed(String),
}

// ---------------------------------------------------------------------------
// Page component
// ---------------------------------------------------------------------------

/// Live aggregate dashboard page — `/dashboard/live`.
///
/// Polls the backend every `POLL_INTERVAL_MS` and renders the demo room's
/// big-screen view. The route is registered behind `ProtectedRoute` in
/// `lib.rs`, so staff auth is enforced before this component mounts.
#[component]
pub fn DashboardLive() -> impl IntoView {
    // Read event_id once at mount, then store in a signal. Signals are `Copy`
    // (they clone their handle, not the underlying value), so they can be
    // captured by multiple `move` closures — the polling `Effect` and the
    // refresh-handler factory — without any of them consuming the value.
    // This replaces an earlier `Rc<Option<String>>` approach that failed
    // Leptos's `Send + Sync` bounds for view children closures.
    let query = use_query_map();
    let initial_event_id: Option<String> = query
        .get()
        .get("event_id")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let (event_id, _set_event_id) = signal(initial_event_id);

    let (data, set_data) = signal(Option::<LiveDashboardResponse>::None);
    let (load_state, set_load_state) = signal(DashboardLoadState::Idle);
    let (last_poll_ms, set_last_poll_ms) = signal(Option::<f64>::None);
    let (polling_active, set_polling_active) = signal(true);
    // Tracks the wall-clock time so "Xs ago" badges refresh between polls.
    let (now_ms, set_now_ms) = signal(js_sys::Date::now());

    // Polling loop — fires immediately, then every POLL_INTERVAL_MS while active.
    // The Effect body captures signals by value (they are Copy) and does not
    // read them synchronously, so toggling `polling_active` does not re-fire
    // the Effect (which would spawn duplicate loops).
    Effect::new(move |_| {
        let eid = event_id.get();
        let polling = polling_active;
        let set_d = set_data;
        let set_ls = set_load_state;
        let set_lpm = set_last_poll_ms;
        leptos::task::spawn_local(async move {
            // Mark Loading on first poll so the spinner renders immediately.
            // Subsequent polls keep the previous snapshot on screen
            // (`fetch_dashboard` never clears `set_d` on error), so this
            // initial transition is the only place Loading is entered.
            set_ls.set(DashboardLoadState::Loading);
            // First poll fires immediately so the room doesn't wait 2.5s.
            fetch_dashboard(eid.as_deref(), set_d, set_ls, set_lpm).await;

            loop {
                // Pause-aware sleep: short 500ms re-checks while paused so
                // resume takes effect promptly without busy-spinning.
                loop {
                    let wait = if polling.get() {
                        POLL_INTERVAL_MS
                    } else {
                        500
                    };
                    gloo_timers::future::TimeoutFuture::new(wait).await;
                    if polling.get() {
                        break;
                    }
                }
                fetch_dashboard(eid.as_deref(), set_d, set_ls, set_lpm).await;
            }
        });
    });

    // Wall-clock tick so age badges update between polls.
    Effect::new(move |_| {
        let set_n = set_now_ms;
        leptos::task::spawn_local(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(AGE_TICK_MS).await;
                set_n.set(js_sys::Date::now());
            }
        });
    });

    // Refresh handler factory — builds a fresh `move` closure per call so two
    // buttons (header refresh + error retry) each get an independent handler
    // with their own captured `event_id` clone. A single `move` closure can't
    // be shared across multiple `on:click` bindings because each binding moves
    // the handler. The outer closure is non-`move`, capturing `event_id` by
    // shared reference and cloning it on each invocation; it therefore
    // implements `Fn` and can be called any number of times.
    let build_refresh_handler = move || {
        // `.get()` on a signal returns an owned `Option<String>` (the signal's
        // `Copy` clone is the handle, not the inner value), so each button
        // captures its own fresh `eid` without aliasing the underlying data.
        let eid = event_id.get();
        let set_d = set_data;
        let set_ls = set_load_state;
        let set_lpm = set_last_poll_ms;
        move |_ev: web_sys::MouseEvent| {
            let eid = eid.clone();
            leptos::task::spawn_local(async move {
                fetch_dashboard(eid.as_deref(), set_d, set_ls, set_lpm).await;
            });
        }
    };

    // Pause/resume toggle. `set_polling_active` is the WriteSignal; the loop
    // reads `polling_active.get()` so toggling takes effect on the next iteration.
    let on_toggle_polling = move |_ev: web_sys::MouseEvent| {
        set_polling_active.update(|p: &mut bool| *p = !*p);
    };

    let is_initial_loading = move || {
        matches!(
            load_state.get(),
            DashboardLoadState::Idle | DashboardLoadState::Loading
        ) && data.get().is_none()
    };
    let is_hard_failure = move || {
        matches!(load_state.get(), DashboardLoadState::Failed(_)) && data.get().is_none()
    };
    let has_data = move || data.get().is_some();

    view! {
        <Title text="Live Dashboard — BeThere" />
        <div class="dashboard-live-page">
            // ---------- Header: event meta + controls ----------
            <header class="dashboard-header">
                <div class="dashboard-header-info">
                    {move || match data.get().as_ref() {
                        Some(resp) => view! {
                            <div class="dashboard-event-title">
                                <h1>{resp.event.name.clone()}</h1>
                                <span class="dashboard-event-subtitle">
                                    {format!(
                                        "Capacity: {}",
                                        match resp.event.in_person_capacity {
                                            Some(v) if v >= 0 => v.to_string(),
                                            _ => "Unlimited".to_string(),
                                        }
                                    )}
                                    " · "
                                    {format!(
                                        "Deposit: {} USDC",
                                        format_usdc(resp.event.deposit_amount_usdc as u64)
                                    )}
                                </span>
                            </div>
                        }
                        .into_any(),
                        None => view! {
                            <div class="dashboard-event-title">
                                <h1>"Loading event…"</h1>
                            </div>
                        }
                        .into_any(),
                    }}
                </div>
                <div class="dashboard-controls">
                    {move || match last_poll_ms.get() {
                        Some(t) => {
                            let age = now_ms.get() - t;
                            view! {
                                <span class="dashboard-last-updated">
                                    "Updated " {relative_time(age)}
                                </span>
                            }
                            .into_any()
                        }
                        None => view! { <span class="dashboard-last-updated"></span> }.into_any(),
                    }}
                    <button
                        class="dashboard-btn dashboard-btn-refresh"
                        on:click=build_refresh_handler()
                        disabled=move || {
                            matches!(load_state.get(), DashboardLoadState::Loading)
                        }
                    >
                        "↻ Refresh"
                    </button>
                    <button
                        class="dashboard-btn dashboard-btn-toggle"
                        on:click=on_toggle_polling
                    >
                        {move || if polling_active.get() { "⏸ Pause" } else { "▶ Resume" } }
                    </button>
                </div>
            </header>

            // ---------- Initial loading state ----------
            <Show when=move || is_initial_loading() fallback=|| view! { <div></div> }>
                <div class="dashboard-state dashboard-state-loading">
                    <div class="dashboard-spinner"></div>
                    <p>"Loading live data…"</p>
                </div>
            </Show>

            // ---------- Hard failure state (no cached data yet) ----------
            <Show when=move || is_hard_failure() fallback=|| view! { <div></div> }>
                <div class="dashboard-state dashboard-state-error">
                    <p>"Failed to load dashboard data."</p>
                    <p class="dashboard-state-error-detail">
                        {move || match load_state.get() {
                            DashboardLoadState::Failed(msg) => msg,
                            _ => String::new(),
                        }}
                    </p>
                    <button class="dashboard-btn dashboard-btn-refresh" on:click=build_refresh_handler()>
                        "Try again"
                    </button>
                </div>
            </Show>

            // ---------- Main content (cached data on screen) ----------
            <Show when=move || has_data() fallback=|| view! { <div></div> }>
                {move || {
                    let resp = data.get().unwrap_or_default();
                    let totals = resp.totals.clone();
                    let funnel = resp.funnel.clone();
                    let activity = resp.recent_activity.clone();
                    let generated = resp.generated_at.clone();
                    view! {
                        <BigNumberTiles totals=totals />
                        <FunnelView stages=funnel />
                        <ActivityFeed entries=activity now_ms=now_ms generated_at=generated />
                    }
                    .into_any()
                }}
            </Show>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Polling fetch helper
// ---------------------------------------------------------------------------

/// Run a single dashboard fetch and update the four state signals.
///
/// On success: caches the snapshot and marks `Loaded`. On error: records the
/// failure message but **does not** clear `set_data` — the previous snapshot
/// stays on screen so a transient D1 blip can't blank the big-screen view
/// mid-demo. The error is still surfaced via `load_state` so the UI can show
/// a non-blocking "last poll failed" hint if desired.
async fn fetch_dashboard(
    event_id: Option<&str>,
    set_data: WriteSignal<Option<LiveDashboardResponse>>,
    set_state: WriteSignal<DashboardLoadState>,
    set_last_poll: WriteSignal<Option<f64>>,
) {
    // The UI already treats `Idle` as "loading" (see `is_initial_loading`),
    // so we don't need to read state here — we only transition forward to
    // `Loaded` or `Failed` once the fetch resolves. This avoids the
    // ReadSignal/WriteSignal split that would otherwise require passing
    // both ends of the signal into the helper.
    match api::get_live_dashboard(event_id).await {
        Ok(resp) => {
            set_data.set(Some(resp));
            set_state.set(DashboardLoadState::Loaded);
            set_last_poll.set(Some(js_sys::Date::now()));
        }
        Err(e) => {
            set_state.set(DashboardLoadState::Failed(e.message));
            set_last_poll.set(Some(js_sys::Date::now()));
        }
    }
}

// ---------------------------------------------------------------------------
// Big-number tiles
// ---------------------------------------------------------------------------

/// Five headline tiles showing the live counts that matter for the demo.
///
/// Each tile is color-coded so the presenter can scan the board at a glance:
/// registered (blue), deposits (gold), USDC locked (green), checked-in (purple),
/// NFTs minted (pink). Order matches the canonical attendee lifecycle.
#[component]
fn BigNumberTiles(totals: DashboardTotals) -> impl IntoView {
    let usdc_display = format_usdc(totals.usdc_locked_total);
    view! {
        <section class="dashboard-tiles">
            <Tile
                label="Registered"
                value=format!("{}", totals.registered)
                sub="approved"
                emoji="📝"
                class_name="tile-blue"
            />
            <Tile
                label="Deposits"
                value=format!("{}", totals.deposits_verified)
                sub="verified"
                emoji="💰"
                class_name="tile-gold"
            />
            <Tile
                label="USDC Locked"
                value=usdc_display
                sub="USDC"
                emoji="🎯"
                class_name="tile-green"
            />
            <Tile
                label="Checked In"
                value=format!("{}", totals.checked_in)
                sub="live"
                emoji="✅"
                class_name="tile-purple"
            />
            <Tile
                label="NFTs Minted"
                value=format!("{}", totals.claims_minted)
                sub="claimed"
                emoji="🎖️"
                class_name="tile-pink"
            />
        </section>
    }
}

/// A single headline tile. `class_name` selects the color theme; all other
/// props are content. Kept as a component so the five tiles share one render
/// path and one set of class names.
#[component]
fn Tile(
    label: &'static str,
    value: String,
    sub: &'static str,
    emoji: &'static str,
    class_name: &'static str,
) -> impl IntoView {
    view! {
        <div class=format!("dashboard-tile {class_name}")>
            <div class="dashboard-tile-emoji">{emoji}</div>
            <div class="dashboard-tile-value">{value}</div>
            <div class="dashboard-tile-label">{label}</div>
            <div class="dashboard-tile-sub">{sub}</div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Funnel visualization
// ---------------------------------------------------------------------------

/// Per-row funnel data pre-computed before the stage vec is consumed by the
/// render closure. Carrying the conversion + bar width alongside the stage
/// avoids the borrow-after-move trap that comes from indexing into a vec
/// while simultaneously iterating it.
struct FunnelRow {
    stage: FunnelStage,
    /// Conversion percentage from the previous stage, or `None` for stage 0.
    /// `None` when the previous stage is zero (avoid divide-by-zero).
    conversion: Option<f64>,
    /// Bar width as a percentage of the maximum stage count (0.0–100.0).
    bar_width_pct: f64,
}

/// Four-stage conversion funnel: registered → deposited → checked_in → claimed_nft.
///
/// Each row shows its absolute count plus the conversion percentage from the
/// previous stage (the first row has no predecessor and renders no percentage).
/// Bar widths are proportional to the maximum stage count, so the funnel
/// narrows visibly as attendees progress.
#[component]
fn FunnelView(stages: Vec<FunnelStage>) -> impl IntoView {
    let rows = build_funnel_rows(&stages);
    view! {
        <section class="dashboard-funnel">
            <h2 class="dashboard-section-title">"Conversion Funnel"</h2>
            <div class="dashboard-funnel-stages">
                {rows
                    .into_iter()
                    .map(|row| {
                        let bar_style = format!("width: {:.1}%", row.bar_width_pct);
                        view! {
                            <div class="dashboard-funnel-stage">
                                <div class="dashboard-funnel-stage-header">
                                    <span class="dashboard-funnel-stage-emoji">
                                        {stage_emoji(&row.stage.stage)}
                                    </span>
                                    <span class="dashboard-funnel-stage-label">
                                        {stage_label(&row.stage.stage)}
                                    </span>
                                    <span class="dashboard-funnel-stage-count">
                                        {format!("{}", row.stage.count)}
                                    </span>
                                    {match row.conversion {
                                        Some(p) => view! {
                                            <span class="dashboard-funnel-stage-conv">
                                                {format!("{p:.0}%")}
                                            </span>
                                        }
                                        .into_any(),
                                        None => view! {
                                            <span class="dashboard-funnel-stage-conv-spacer"></span>
                                        }
                                        .into_any(),
                                    }}
                                </div>
                                <div class="dashboard-funnel-bar-track">
                                    <div class="dashboard-funnel-bar-fill" style=bar_style></div>
                                </div>
                            </div>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </section>
    }
}

/// Build the pre-computed funnel rows from a stage slice.
///
/// Conversion is computed against the immediate predecessor (not the first
/// stage), so each row answers "of the people who got this far, how many
/// advanced?" — the standard funnel semantic. Bar width normalizes against
/// the maximum count so the largest stage fills the track.
fn build_funnel_rows(stages: &[FunnelStage]) -> Vec<FunnelRow> {
    let max_count = stages.iter().map(|s| s.count).max().unwrap_or(0);
    stages
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let bar_width_pct = if max_count > 0 {
                (stage.count as f64 / max_count as f64) * 100.0
            } else {
                0.0
            };
            let conversion = if i == 0 {
                None
            } else {
                let prev = stages.get(i - 1).map(|s| s.count).unwrap_or(0);
                if prev > 0 {
                    Some((stage.count as f64 / prev as f64) * 100.0)
                } else {
                    None
                }
            };
            FunnelRow {
                stage: stage.clone(),
                conversion,
                bar_width_pct,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Activity feed
// ---------------------------------------------------------------------------

/// Live activity feed rendered from the audit log.
///
/// Each entry shows an action-specific emoji, the human-readable description,
/// and the actor + action type as metadata. The "Snapshot Xs ago" badge uses
/// the `generated_at` timestamp from the server so the room can tell if the
/// dashboard has gone stale (e.g., worker isolate cold-started).
#[component]
fn ActivityFeed(
    entries: Vec<ActivityEntry>,
    now_ms: ReadSignal<f64>,
    generated_at: String,
) -> impl IntoView {
    let empty = entries.is_empty();
    view! {
        <section class="dashboard-activity">
            <div class="dashboard-activity-header">
                <h2 class="dashboard-section-title">"Live Activity"</h2>
                {move || match parse_iso_to_epoch_ms(&generated_at) {
                    Some(g) => {
                        let age = now_ms.get() - g;
                        view! {
                            <span class="dashboard-activity-stamp">
                                "Snapshot " {relative_time(age)}
                            </span>
                        }
                        .into_any()
                    }
                    None => view! { <span class="dashboard-activity-stamp"></span> }.into_any(),
                }}
            </div>
            {if empty {
                view! {
                    <div class="dashboard-activity-empty">
                        <p>"No activity yet. Check-ins, deposits, and claims will appear here in real time."</p>
                    </div>
                }
                .into_any()
            } else {
                view! {
                    <ul class="dashboard-activity-list">
                        {entries
                            .into_iter()
                            .map(|e| {
                                let emoji = action_emoji(&e.action);
                                view! {
                                    <li class="dashboard-activity-entry">
                                        <span class="dashboard-activity-entry-emoji">{emoji}</span>
                                        <div class="dashboard-activity-entry-body">
                                            <div class="dashboard-activity-entry-desc">
                                                {e.description.clone()}
                                            </div>
                                            <div class="dashboard-activity-entry-meta">
                                                <span class="dashboard-activity-entry-action">
                                                    {e.action.clone()}
                                                </span>
                                                <span class="dashboard-activity-entry-actor">
                                                    {e.actor.clone()}
                                                </span>
                                            </div>
                                        </div>
                                    </li>
                                }
                            })
                            .collect::<Vec<_>>()}
                    </ul>
                }
                .into_any()
            }}
        </section>
    }
}

// ---------------------------------------------------------------------------
// UI helpers
// ---------------------------------------------------------------------------

/// Format a millisecond age as a human-friendly "Xs ago" / "Xm ago" string.
///
/// Used for both the "Updated Xs ago" header badge (delta from last poll) and
/// the "Snapshot Xs ago" activity feed badge (delta from server-generated time).
/// Negative deltas (clock skew) clamp to "just now" so a slightly-off server
/// clock can't render "Updated -3s ago".
fn relative_time(age_ms: f64) -> String {
    let secs = (age_ms.max(0.0) / 1000.0) as u64;
    if secs < 5 {
        "just now".to_string()
    } else if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

/// Parse an RFC 3339 timestamp (as produced by `chrono::Utc::now().to_rfc3339()`
/// on the worker) into epoch milliseconds.
///
/// Returns `None` on parse failure rather than panicking — a malformed
/// `generated_at` should produce a blank badge, not a crashed dashboard.
fn parse_iso_to_epoch_ms(iso: &str) -> Option<f64> {
    // `js_sys::Date::new(&JsValue)` parses ISO 8601 / RFC 3339 in the browser.
    // `get_time()` returns epoch ms; NaN indicates parse failure.
    let parsed = js_sys::Date::new(&JsValue::from_str(iso));
    let ms = parsed.get_time();
    if ms.is_nan() {
        None
    } else {
        Some(ms)
    }
}

/// Map a funnel stage identifier to an emoji for the funnel renderer.
///
/// Stage identifiers match the strings emitted by the worker's
/// `LiveDashboardResponse.funnel[].stage` field.
fn stage_emoji(stage: &str) -> &'static str {
    match stage {
        "registered" => "📝",
        "deposited" => "💰",
        "checked_in" => "✅",
        "claimed_nft" => "🎖️",
        _ => "•",
    }
}

/// Map a funnel stage identifier to a human-readable label.
/// Returns `String` rather than `&'static str` because the fallback arm
/// echoes the input `&str` — returning it as `&'static` would be unsound.
/// Callers in the view macro accept `String` transparently.
fn stage_label(stage: &str) -> String {
    match stage {
        "registered" => "Registered".to_string(),
        "deposited" => "Deposited".to_string(),
        "checked_in" => "Checked In".to_string(),
        "claimed_nft" => "NFT Claimed".to_string(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_time_just_now_for_small_deltas() {
        assert_eq!(relative_time(0.0), "just now");
        assert_eq!(relative_time(4_000.0), "just now");
    }

    #[test]
    fn relative_time_seconds() {
        assert_eq!(relative_time(5_000.0), "5s ago");
        assert_eq!(relative_time(59_000.0), "59s ago");
    }

    #[test]
    fn relative_time_minutes() {
        assert_eq!(relative_time(60_000.0), "1m ago");
        assert_eq!(relative_time(120_000.0), "2m ago");
    }

    #[test]
    fn relative_time_hours_and_days() {
        assert_eq!(relative_time(3_600_000.0), "1h ago");
        assert_eq!(relative_time(86_400_000.0), "1d ago");
    }

    #[test]
    fn relative_time_clamps_negative_clock_skew() {
        assert_eq!(relative_time(-3_000.0), "just now");
    }

    #[test]
    fn stage_emoji_known_stages() {
        assert_eq!(stage_emoji("registered"), "📝");
        assert_eq!(stage_emoji("deposited"), "💰");
        assert_eq!(stage_emoji("checked_in"), "✅");
        assert_eq!(stage_emoji("claimed_nft"), "🎖️");
    }

    #[test]
    fn stage_emoji_unknown_falls_back_to_pulse() {
        assert_eq!(stage_emoji("something_new"), "•");
    }

    #[test]
    fn stage_label_known_stages() {
        assert_eq!(stage_label("registered"), "Registered");
        assert_eq!(stage_label("claimed_nft"), "NFT Claimed");
    }

    #[test]
    fn stage_label_unknown_passes_through() {
        assert_eq!(stage_label("custom_stage"), "custom_stage");
    }

    #[test]
    fn funnel_rows_first_stage_has_no_conversion() {
        let stages = vec![
            FunnelStage {
                stage: "registered".to_string(),
                count: 100,
            },
            FunnelStage {
                stage: "deposited".to_string(),
                count: 50,
            },
        ];
        let rows = build_funnel_rows(&stages);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].conversion.is_none());
    }

    #[test]
    fn funnel_rows_conversion_uses_immediate_predecessor() {
        let stages = vec![
            FunnelStage {
                stage: "registered".to_string(),
                count: 100,
            },
            FunnelStage {
                stage: "deposited".to_string(),
                count: 50,
            },
            FunnelStage {
                stage: "checked_in".to_string(),
                count: 25,
            },
        ];
        let rows = build_funnel_rows(&stages);
        // Stage 1: 50 / 100 = 50%
        assert_eq!(rows[1].conversion, Some(50.0));
        // Stage 2: 25 / 50 = 50%
        assert_eq!(rows[2].conversion, Some(50.0));
    }

    #[test]
    fn funnel_rows_zero_predecessor_yields_none() {
        let stages = vec![
            FunnelStage {
                stage: "registered".to_string(),
                count: 0,
            },
            FunnelStage {
                stage: "deposited".to_string(),
                count: 5,
            },
        ];
        let rows = build_funnel_rows(&stages);
        // Predecessor is 0 — would divide by zero, so None.
        assert!(rows[1].conversion.is_none());
    }

    #[test]
    fn funnel_rows_bar_widths_normalize_to_max() {
        let stages = vec![
            FunnelStage {
                stage: "registered".to_string(),
                count: 100,
            },
            FunnelStage {
                stage: "deposited".to_string(),
                count: 25,
            },
        ];
        let rows = build_funnel_rows(&stages);
        assert_eq!(rows[0].bar_width_pct, 100.0);
        assert_eq!(rows[1].bar_width_pct, 25.0);
    }

    #[test]
    fn funnel_rows_empty_stages_yield_empty_rows() {
        let stages: Vec<FunnelStage> = vec![];
        let rows = build_funnel_rows(&stages);
        assert!(rows.is_empty());
    }
}
