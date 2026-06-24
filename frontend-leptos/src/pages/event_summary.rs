//! Post-event summary page — Plan 008 Phase 1.
//!
//! Route: `/events/:id/summary` (organizer+ only, wrapped in `ProtectedRoute`).
//!
//! Shows a frozen or live-preview snapshot of an event's funnel and
//! financials. When the event has ended and the snapshot is not yet frozen,
//! the organizer can permanently freeze it via a confirm dialog — after
//! freezing, later refunds no longer change these numbers.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

use crate::api::{self, format_usdc};
use crate::components;

/// Route parameters for `/events/:id/summary`.
#[derive(Params, PartialEq, Clone, Debug)]
struct EventSummaryParams {
    id: Option<String>,
}

/// Coarse load state surfaced to the UI. Once we have any data we keep it on
/// screen, mirroring the live dashboard's stale-on-error behavior so a
/// transient blip never blanks the summary.
#[derive(Debug, Clone, PartialEq)]
enum SummaryLoadState {
    Loading,
    Loaded,
    Failed(String),
}

/// Post-event summary page.
///
/// On mount, fetches `GET /events/{id}/summary`. The snapshot is either frozen
/// (permanent) or a live preview (`frozen == false`). The freeze button only
/// renders when the event has ended (`now_ms >= event_end_ms`) and the
/// snapshot is not yet frozen — backend re-validates both, but we hide the
/// button to avoid tempting the organizer into a guaranteed-400 click.
#[component]
pub fn EventSummary() -> impl IntoView {
    let params = use_params::<EventSummaryParams>();

    // Resolve the event id once at mount. The route param is the only source;
    // an empty/missing id renders a "not found" state instead of an API call.
    let event_id: String = match params.get() {
        Ok(p) => p.id.unwrap_or_default(),
        Err(_) => String::new(),
    };

    let (data, set_data) = signal(Option::<api::EventSummaryData>::None);
    let (load_state, set_load_state) = signal(SummaryLoadState::Loading);
    let (toast, set_toast) = signal(None::<components::ToastMessage>);
    // Refresh counter drives reloads. The derived `do_reload` closure captures
    // only `set_refresh_counter` (a Copy WriteSignal), so the closure itself
    // is Copy — that lets the freeze handler capture it by value into a
    // `'static` future without borrowing or Clone gymnastics. Mirrors the
    // pattern in `events_page.rs`.
    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let do_reload = move || {
        set_refresh_counter.update(|n| *n += 1);
    };

    // Fetch on mount and on each reload. Reading `refresh_counter` in the
    // Effect body re-runs the fetch whenever the counter changes.
    Effect::new(move |_| {
        let _ = refresh_counter.get();
        if event_id.is_empty() {
            set_load_state.set(SummaryLoadState::Failed(
                "Missing event id in URL".to_string(),
            ));
            return;
        }
        let id = event_id.clone();
        let set_d = set_data;
        let set_ls = set_load_state;
        set_ls.set(SummaryLoadState::Loading);
        leptos::task::spawn_local(async move {
            match api::get_event_summary(&id).await {
                Ok(payload) => {
                    set_d.set(Some(payload));
                    set_ls.set(SummaryLoadState::Loaded);
                }
                Err(e) => {
                    log::error!("[event-summary] load failed: {e}");
                    set_ls.set(SummaryLoadState::Failed(format!("{e}")));
                }
            }
        });
    });

    let is_loading = move || {
        matches!(load_state.get(), SummaryLoadState::Loading) && data.get().is_none()
    };
    let is_hard_failure = move || {
        matches!(load_state.get(), SummaryLoadState::Failed(_)) && data.get().is_none()
    };

    view! {
        <Title text="Event Summary — BeThere" />
        <components::Toast toast_signal=toast />

        <div class="center-page">
            <div class="container layout-col-center">
                // ---------- Back link ----------
                <div class="flex-row-gap" style="margin-bottom:1rem;width:100%;justify-content:flex-start;">
                    <a href="/admin" class="btn btn-outline btn-sm">"← Back"</a>
                </div>

                // ---------- Loading state ----------
                <Show when=move || is_loading() fallback=|| view! { <div></div> }>
                    <div class="page-loading">
                        <span class="spinner spinner-lg"></span>
                        "Loading summary..."
                    </div>
                </Show>

                // ---------- Hard failure (no cached data) ----------
                <Show when=move || is_hard_failure() fallback=|| view! { <div></div> }>
                    <div class="card">
                        <h2>"Failed to load summary"</h2>
                        <p class="subtitle">
                            {move || match load_state.get() {
                                SummaryLoadState::Failed(msg) => msg,
                                _ => String::new(),
                            }}
                        </p>
                        <button class="btn btn-primary" on:click=move |_| do_reload()>
                            "Try again"
                        </button>
                    </div>
                </Show>

                // ---------- Summary content (cached data on screen) ----------
                <Show when=move || data.get().is_some() fallback=|| view! { <div></div> }>
                    {move || {
                        let resp = data.get().unwrap_or_default();
                        let summary = resp.summary.clone();
                        view! {
                            <SummaryHeader
                                event_id=resp.event_id.clone()
                                frozen=resp.frozen
                                frozen_at=summary.frozen_at.clone()
                                frozen_by=summary.frozen_by.clone()
                            />
                            <FunnelTiles funnel=summary.funnel.clone() />
                            <Financials financials=summary.financials.clone() />
                            <FreezeSection
                                event_id=resp.event_id.clone()
                                frozen=resp.frozen
                                event_end_ms=summary.event_end_ms
                                set_toast=set_toast
                                set_refresh_counter=set_refresh_counter
                            />
                        }
                            .into_any()
                    }}
                </Show>
            </div>
        </div>
    }
}

/// Page header: event id + frozen/live badge.
#[component]
fn SummaryHeader(
    event_id: String,
    frozen: bool,
    frozen_at: Option<String>,
    frozen_by: String,
) -> impl IntoView {
    view! {
        <div class="flex-row-gap events-flex-wrap-center" style="width:100%;align-items:center;margin-bottom:1rem;">
            <h1 style="margin:0;">"Event Summary"</h1>
            {if frozen {
                let badge_text = match frozen_at.as_deref() {
                    Some(ts) => format!("❄ Frozen at {}", format_iso(ts)),
                    None => "❄ Frozen".to_string(),
                };
                let by_text = if frozen_by.is_empty() {
                    String::new()
                } else {
                    format!(" by {frozen_by}")
                };
                view! {
                    <span class="badge badge-success-xs">
                        {format!("{badge_text}{by_text}")}
                    </span>
                }
                    .into_any()
            } else {
                view! {
                    <span class="badge badge-warning-xs">"Live preview (not yet frozen)"</span>
                }
                    .into_any()
            }}
            <span class="badge badge-info-xs">{event_id}</span>
        </div>
    }
}

/// Funnel tiles: registered → deposited → checked-in → claimed, with
/// conversion percentages against the registered count (the funnel entry).
/// Also surfaces no-show count + rate and post-event-reg count.
#[component]
fn FunnelTiles(funnel: api::FunnelSnapshotData) -> impl IntoView {
    let registered = funnel.registered_count;
    let deposited = funnel.deposited_count;
    let checked_in = funnel.checked_in_count;
    let claimed = funnel.claimed_count;
    let no_show = funnel.no_show_count;
    let post_reg = funnel.post_event_reg_count;
    let refunded = funnel.refunded_count;

    // Conversion of each stage against the registered count (the funnel entry).
    let conv = |stage: u64| -> String {
        if registered == 0 {
            "—".to_string()
        } else {
            format!("{:.0}%", (stage as f64 / registered as f64) * 100.0)
        }
    };
    // No-show rate is checked_in-eligible no-shows over registered attendees.
    let no_show_rate = if registered == 0 {
        "—".to_string()
    } else {
        format!("{:.0}%", (no_show as f64 / registered as f64) * 100.0)
    };

    view! {
        <section class="dashboard-tiles">
            <Tile label="Registered" value=format!("{registered}") sub="total".to_string() emoji="📝" />
            <Tile label="Deposited" value=format!("{deposited}") sub=conv(deposited) emoji="💰" />
            <Tile label="Checked In" value=format!("{checked_in}") sub=conv(checked_in) emoji="✅" />
            <Tile label="Claimed" value=format!("{claimed}") sub=conv(claimed) emoji="🎖️" />
        </section>

        <section class="dashboard-tiles" style="margin-top:1rem;">
            <Tile label="No-show" value=format!("{no_show}") sub=no_show_rate emoji="🚫" />
            <Tile label="Post-event Reg" value=format!("{post_reg}") sub="walk-ins".to_string() emoji="🚪" />
            <Tile label="Refunded" value=format!("{refunded}") sub="returned".to_string() emoji="↩️" />
        </section>
    }
}

/// A single summary tile. Reuses the dashboard-tile classes so the visual
/// style matches the live big-screen view. `sub` is `String` because some
/// callers compute it dynamically (conversion percentages).
#[component]
fn Tile(label: &'static str, value: String, sub: String, emoji: &'static str) -> impl IntoView {
    view! {
        <div class="dashboard-tile">
            <div class="dashboard-tile-emoji">{emoji}</div>
            <div class="dashboard-tile-value">{value}</div>
            <div class="dashboard-tile-label">{label}</div>
            <div class="dashboard-tile-sub">{sub}</div>
        </div>
    }
}

/// Financial breakdown. USDC uses the shared `format_usdc` (atomic /1_000_000).
/// THB amounts are satang (/100). The USDC-refunded row is labeled honestly:
/// v1 always reports 0 because the backend doesn't yet sum USDC refunds.
#[component]
fn Financials(financials: api::FinancialSnapshotData) -> impl IntoView {
    let usdc_deposited = format_usdc(financials.usdc_deposited_total);
    // Always 0 in v1 — surface honestly rather than pretending it's tracked.
    let usdc_refunded_display = format_usdc(financials.usdc_refunded_total);
    let thb_deposited = format_thb(financials.thb_deposited_total);
    let thb_refunded = format_thb(financials.thb_refunded_total);

    view! {
        <section class="card" style="margin-top:1rem;width:100%;">
            <div class="card-header">
                <span class="card-title">"Financials"</span>
            </div>
            <div class="event-detail-grid">
                <div class="quiz-setting-item">
                    <span class="quiz-setting-label">"USDC Deposited"</span>
                    <span class="setting-value">{usdc_deposited} " USDC"</span>
                </div>
                <div class="quiz-setting-item">
                    <span class="quiz-setting-label">"USDC Refunded"</span>
                    <span class="setting-value">
                        {usdc_refunded_display} " USDC"
                        <span class="subtitle" style="display:block;font-size:0.8rem;">
                            "(v1: not yet tracked)"
                        </span>
                    </span>
                </div>
                <div class="quiz-setting-item">
                    <span class="quiz-setting-label">"THB Deposited"</span>
                    <span class="setting-value">{thb_deposited} " THB"</span>
                </div>
                <div class="quiz-setting-item">
                    <span class="quiz-setting-label">"THB Refunded"</span>
                    <span class="setting-value">{thb_refunded} " THB"</span>
                </div>
            </div>
        </section>
    }
}

/// Freeze button + warning. Renders only when the snapshot is not yet frozen
/// AND the event has ended (`now_ms >= event_end_ms`). The confirm dialog
/// reminds the organizer that freezing is permanent — later refunds will not
/// change the frozen numbers.
///
/// Takes `set_refresh_counter` (a `Copy` WriteSignal) rather than a reload
/// callback so all captures stay `Copy` — that keeps the click handler `Fn`
/// and avoids `Send`/`Sync` bounds that a generic closure would fail.
#[component]
fn FreezeSection(
    event_id: String,
    frozen: bool,
    event_end_ms: i64,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    set_refresh_counter: WriteSignal<u32>,
) -> impl IntoView {
    // Hide entirely once frozen, or while the event is still running. The
    // backend re-checks both, but hiding the button avoids a guaranteed 400.
    let now_ms = js_sys::Date::now() as i64;
    let can_freeze = !frozen && now_ms >= event_end_ms;

    // Use a plain `{if ...}` (evaluated once) rather than `<Show>` so the
    // owned `event_id` can be moved into the click handler's `spawn_local`
    // future. `<Show>`'s children must be `Fn` (re-callable), which would make
    // an owned-String capture `FnOnce` and fail to compile.
    if !can_freeze {
        return view! { <div></div> }.into_any();
    }

    // Clone into a local so the click closure moves the clone, mirroring the
    // `let did = dup_id.clone();` pattern in events_page.rs.
    let freeze_id = event_id.clone();
    let set_toast_for_click = set_toast;
    let set_counter_for_click = set_refresh_counter;

    view! {
        <section class="card" style="margin-top:1rem;width:100%;">
            <div class="flex-row-gap events-flex-wrap-center">
                <div>
                    <span class="card-title">"Freeze this snapshot"</span>
                    <p class="subtitle" style="margin:0.25rem 0 0;">
                        "Lock these numbers as the permanent post-event record."
                    </p>
                </div>
                <button
                    class="btn btn-primary"
                    on:click=move |_| {
                        let id = freeze_id.clone();
                        let set_toast = set_toast_for_click;
                        let set_counter = set_counter_for_click;
                        let confirm_msg = "This snapshot is permanent — later refunds will not change these numbers.";
                        if !web_sys::window().unwrap().confirm_with_message(confirm_msg).unwrap_or(false) {
                            return;
                        }
                        leptos::task::spawn_local(async move {
                            match api::freeze_event_summary(&id).await {
                                Ok(_) => {
                                    components::show_toast(
                                        &set_toast,
                                        "Summary frozen successfully",
                                        components::ToastType::Success,
                                    );
                                    set_counter.update(|n| *n += 1);
                                }
                                Err(e) => {
                                    log::error!("[event-summary] freeze failed: {e}");
                                    components::show_toast(
                                        &set_toast,
                                        &format!("Failed to freeze: {e}"),
                                        components::ToastType::Error,
                                    );
                                }
                            }
                        });
                    }
                >
                    "❄ Freeze Snapshot"
                </button>
            </div>
        </section>
    }
        .into_any()
}

// ---------------------------------------------------------------------------
// Local formatting helpers
// ---------------------------------------------------------------------------

/// Format an ISO 8601 timestamp (e.g. `"2026-06-24T10:00:00Z"`) for display.
///
/// Falls back to the raw string if the browser can't parse it. Mirrors the
/// `format_timestamp` pattern in `audit_panel.rs` (no `chrono` dependency).
fn format_iso(iso: &str) -> String {
    let parsed = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    if parsed.get_time().is_nan() {
        return iso.to_string();
    }
    let year = parsed.get_full_year();
    let month = parsed.get_month() + 1; // 0-indexed
    let day = parsed.get_date();
    let hours = parsed.get_hours();
    let minutes = parsed.get_minutes();
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}

/// Format a satang amount (1 THB = 100 satang) as a 2-decimal THB string.
fn format_thb(satang: u64) -> String {
    format!("{:.2}", satang as f64 / 100.0)
}
