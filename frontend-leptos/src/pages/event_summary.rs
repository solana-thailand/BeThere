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
                            <RecapSection
                                event_id=resp.event_id.clone()
                                summary_frozen=resp.frozen
                                set_toast=set_toast
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
    let in_person_registered = funnel.in_person_registered_count;

    // Conversion of each stage against the registered count (the funnel entry).
    let conv = |stage: u64| -> String {
        if registered == 0 {
            "—".to_string()
        } else {
            format!("{:.0}%", (stage as f64 / registered as f64) * 100.0)
        }
    };
    // No-show rate is computed across the in-person slice only — online
    // registrants are excluded because their attendance isn't signaled by
    // check-in (quest completion is opt-in; joining the call isn't recorded).
    // For online-only events (no in-person attendees) the rate is N/A.
    let no_show_rate = if in_person_registered == 0 {
        "online".to_string()
    } else {
        format!(
            "{:.0}% of {} in-person",
            (no_show as f64 / in_person_registered as f64) * 100.0,
            in_person_registered
        )
    };
    // Online-only events show no no-show value (we have no attendance signal);
    // in-person / hybrid events show the in-person no-show count.
    let no_show_value = if in_person_registered == 0 {
        "—".to_string()
    } else {
        format!("{no_show}")
    };

    view! {
        <section class="dashboard-tiles">
            <Tile label="Registered" value=format!("{registered}") sub="total".to_string() emoji="📝" />
            <Tile label="Deposited" value=format!("{deposited}") sub=conv(deposited) emoji="💰" />
            <Tile label="Checked In" value=format!("{checked_in}") sub=conv(checked_in) emoji="✅" />
            <Tile label="Claimed" value=format!("{claimed}") sub=conv(claimed) emoji="🎖️" />
        </section>

        <section class="dashboard-tiles" style="margin-top:1rem;">
            <Tile label="No-show (in-person)" value=no_show_value sub=no_show_rate emoji="🚫" />
            <Tile label="Post-event Reg" value=format!("{post_reg}") sub="walk-ins".to_string() emoji="🚪" />
            <Tile label="Refunded" value=format!("{refunded}") sub="returned".to_string() emoji="↩️" />
        </section>

        <p class="subtitle" style="margin-top:0.75rem;font-size:0.85rem;">
            "Online registrants are excluded from no-show — their attendance isn't recorded via check-in."
        </p>
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
// Public Recap section (Plan 008 — Phase 2)
// ---------------------------------------------------------------------------

/// Organizer recap authoring section.
///
/// Renders a markdown editor + image URL field + preview, plus "Save Draft"
/// and "Publish" buttons. Disabled (with an explanatory banner) when no frozen
/// summary exists — the backend refuses to publish a recap without a freeze,
/// because "recaps without numbers are misleading" (Plan 008 §3.2.1).
///
/// On mount, fetches `GET /events/{id}/recap` to seed the editor. Save/Publish
/// call `PUT /events/{id}/recap` and refresh the local state from the response.
#[component]
fn RecapSection(
    event_id: String,
    summary_frozen: bool,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
) -> impl IntoView {
    let (recap_data, set_recap_data) = signal(Option::<api::EventRecapData>::None);
    let (load_state, set_load_state) = signal(SummaryLoadState::Loading);
    let (markdown, set_markdown) = signal(String::new());
    let (image_url, set_image_url) = signal(String::new());
    let (busy, set_busy) = signal(false);
    // Bumping `refresh` re-runs the fetch (used after a successful publish to
    // pick up the canonical server state, mirroring the page-level pattern).
    let (refresh, set_refresh) = signal(0u32);

    // Wrap `event_id` in a signal so the two click handlers below can read an
    // owned copy via `.get()` without capturing the String itself by `move`.
    // This is required because the view macro's children must be `Fn`
    // (re-callable); a `move` closure that captures an owned `String` makes
    // the outer view closure `FnOnce` and fails to compile. `ReadSignal` is
    // `Copy`, so each click handler captures the signal by value and the view
    // stays `Fn`. The Effect below also reads from this signal for the same
    // reason. Mirrors the inline-click-handler pattern in `FreezeSection`.
    let (event_id_sig, _) = signal(event_id);

    Effect::new(move |_| {
        let _ = refresh.get();
        let id = event_id_sig.get();
        if id.is_empty() {
            return;
        }
        set_load_state.set(SummaryLoadState::Loading);
        leptos::task::spawn_local(async move {
            match api::get_event_recap(&id).await {
                Ok(payload) => {
                    set_markdown.set(payload.recap.recap_markdown.clone());
                    set_image_url.set(payload.recap.recap_image_url.clone());
                    set_recap_data.set(Some(payload));
                    set_load_state.set(SummaryLoadState::Loaded);
                }
                Err(e) => {
                    log::error!("[event-recap] load failed: {e}");
                    set_load_state.set(SummaryLoadState::Failed(format!("{e}")));
                }
            }
        });
    });

    let is_published = move || {
        recap_data
            .get()
            .and_then(|d| d.recap.recap_published_at)
            .is_some()
    };

    let can_edit = move || summary_frozen && !busy.get();
    let can_publish = move || can_edit() && !markdown.get().trim().is_empty();

    view! {
        <section class="card" style="margin-top:1rem;width:100%;">
            <div class="flex-row-gap events-flex-wrap-center" style="margin-bottom:1rem;">
                <span class="card-title">"Public Recap"</span>
                <Show when=move || is_published() fallback=|| view! { <span></span> }>
                    <span class="badge badge-success-xs">"Published"</span>
                </Show>
                <Show
                    when=move || !is_published() && summary_frozen
                    fallback=|| view! { <span></span> }
                >
                    <span class="badge badge-warning-xs">"Draft"</span>
                </Show>
            </div>

            // Freeze gate — recap authoring requires a frozen summary.
            <Show when=move || !summary_frozen fallback=|| view! { <div></div> }>
                <div class="hint-info" style="margin-bottom:1rem;">
                    "Freeze the summary above before authoring the public recap. Recaps without numbers are misleading."
                </div>
            </Show>

            // Initial loading state (before any data is in hand).
            <Show
                when=move || {
                    matches!(load_state.get(), SummaryLoadState::Loading) && recap_data.get().is_none()
                }
                fallback=|| view! { <div></div> }
            >
                <div class="page-loading">
                    <span class="spinner spinner-sm"></span>
                    "Loading recap..."
                </div>
            </Show>

            // Editor — visible once we have any data OR the summary is already
            // frozen (so the organizer can start drafting on a fresh row).
            <Show
                when=move || recap_data.get().is_some() || summary_frozen
                fallback=|| view! { <div></div> }
            >
                <div class="form-group" style="margin-bottom:1rem;">
                    <label class="form-label">"Recap content (Markdown)"</label>
                    <textarea
                        class="form-input"
                        rows=12
                        placeholder="## Great event! We had 50 developers join us..."
                        prop:value=move || markdown.get()
                        on:input=move |ev| set_markdown.set(event_target_value(&ev))
                        disabled=move || !can_edit()
                        style="font-family:monospace;width:100%;"
                    ></textarea>
                    <div class="hint-info">
                        {move || format!("{} / 16384 bytes", markdown.get().len())}
                    </div>
                </div>

                <div class="form-group" style="margin-bottom:1rem;">
                    <label class="form-label">"Hero image URL (https:// only, optional)"</label>
                    <input
                        type="url"
                        class="form-input"
                        placeholder="https://cdn.example.com/hero.png"
                        prop:value=move || image_url.get()
                        on:input=move |ev| set_image_url.set(event_target_value(&ev))
                        disabled=move || !can_edit()
                    />
                </div>

                // Live image preview.
                <Show when=move || !image_url.get().is_empty() fallback=|| view! { <div></div> }>
                    <div class="form-group" style="margin-bottom:1rem;">
                        <label class="form-label">"Image preview"</label>
                        <img
                            src=move || image_url.get()
                            alt="Recap hero preview"
                            style="max-width:100%;max-height:240px;border-radius:8px;"
                        />
                    </div>
                </Show>

                // Action buttons.
                <div class="flex-row-gap">
                    <button
                        class="btn btn-outline"
                        disabled=move || !can_edit()
                        on:click=move |_| {
                            if !summary_frozen {
                                components::show_toast(
                                    &set_toast,
                                    "Freeze the summary before authoring a recap",
                                    components::ToastType::Error,
                                );
                                return;
                            }
                            spawn_recap_save(
                                event_id_sig.get(),
                                markdown.get(),
                                image_url.get(),
                                false,
                                set_markdown,
                                set_image_url,
                                set_recap_data,
                                set_busy,
                                set_refresh,
                                set_toast,
                            );
                        }
                    >
                        {move || if busy.get() { "Saving..." } else { "Save Draft" }}
                    </button>
                    <button
                        class="btn btn-primary"
                        disabled=move || !can_publish()
                        on:click=move |_| {
                            if !summary_frozen {
                                components::show_toast(
                                    &set_toast,
                                    "Freeze the summary before authoring a recap",
                                    components::ToastType::Error,
                                );
                                return;
                            }
                            if markdown.get().trim().is_empty() {
                                components::show_toast(
                                    &set_toast,
                                    "Cannot publish an empty recap",
                                    components::ToastType::Error,
                                );
                                return;
                            }
                            let confirm_msg = "Publish this recap now? It will be visible immediately at /events/{slug}/recap.";
                            if !web_sys::window().unwrap().confirm_with_message(confirm_msg).unwrap_or(false) {
                                return;
                            }
                            spawn_recap_save(
                                event_id_sig.get(),
                                markdown.get(),
                                image_url.get(),
                                true,
                                set_markdown,
                                set_image_url,
                                set_recap_data,
                                set_busy,
                                set_refresh,
                                set_toast,
                            );
                        }
                    >
                        {move || if busy.get() { "Publishing..." } else { "Publish" }}
                    </button>
                </div>

                // Published-at timestamp.
                <Show when=move || is_published() fallback=|| view! { <div></div> }>
                    <div class="hint-info" style="margin-top:1rem;">
                        {move || format!(
                            "Published at {}",
                            recap_data
                                .get()
                                .and_then(|d| d.recap.recap_published_at)
                                .map(|s| format_iso(&s))
                                .unwrap_or_default()
                        )}
                    </div>
                </Show>
            </Show>
        </section>
    }
}

/// Spawn the recap PUT request as a background task.
///
/// Free function (no captures) so each button's click handler can clone the
/// needed values and call this — avoids the `FnOnce` issue of a shared closure
/// that moves owned data. Mirrors the inline-click-handler pattern in
/// `FreezeSection`.
//
// `too_many_arguments` is intentional: grouping these into a context struct
// would add a one-shot type that exists only to be unpacked immediately. The
// 10 params split naturally into 3 inputs (event_id, markdown, image_url,
// publish) + 6 signal setters the closure mutates + 1 toast signal. Each is
// `Copy` (`WriteSignal` is `Copy`) so there's no ownership/aliasing hazard.
#[allow(clippy::too_many_arguments)]
fn spawn_recap_save(
    event_id: String,
    markdown: String,
    image_url: String,
    publish: bool,
    set_markdown: WriteSignal<String>,
    set_image_url: WriteSignal<String>,
    set_recap_data: WriteSignal<Option<api::EventRecapData>>,
    set_busy: WriteSignal<bool>,
    set_refresh: WriteSignal<u32>,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
) {
    set_busy.set(true);
    leptos::task::spawn_local(async move {
        match api::put_event_recap(&event_id, &markdown, &image_url, publish).await {
            Ok(payload) => {
                set_markdown.set(payload.recap.recap_markdown.clone());
                set_image_url.set(payload.recap.recap_image_url.clone());
                set_recap_data.set(Some(payload));
                let msg = if publish {
                    "Recap published — live on /past-events"
                } else {
                    "Draft saved"
                };
                components::show_toast(&set_toast, msg, components::ToastType::Success);
                set_refresh.update(|n| *n += 1);
            }
            Err(e) => {
                log::error!("[event-recap] save failed: {e}");
                components::show_toast(
                    &set_toast,
                    &format!("Failed to save recap: {e}"),
                    components::ToastType::Error,
                );
            }
        }
        set_busy.set(false);
    });
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
