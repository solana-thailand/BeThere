//! Admin event selector — searchable, sortable dropdown combobox.
//!
//! Replaces the native `<select>` with a richer UI that:
//! - Filters events by name/slug as you type
//! - Sorts by smart priority so the most relevant event is on top:
//!     0. Active + upcoming   (soonest start first — ASC by event_start_ms)
//!     1. Active + past       (most recently started first — DESC)
//!     2. Draft               (soonest start first — ASC)
//!     3. Completed           (most recently completed first — DESC)
//!   Archived events are hidden entirely.
//! - Shows a relative date hint per event ("in 3d", "2d ago", "today")
//! - Closes on outside click (fixed-position backdrop) or Escape
//! - Enter selects the top visible match (keyboard-friendly)
//!
//! The dropdown panel is `position: absolute` under the trigger. The parent
//! `.admin-event-selector` container MUST be `position: relative` and MUST NOT
//! clip overflow (no `overflow: hidden/auto` on any ancestor while open), or
//! the panel will be cut off. See the `.admin-evt-*` rules in `style.css`.

use leptos::prelude::*;

use crate::api::{EventMeta, EventStatus};

/// Relative-date hint shown next to each event in the dropdown.
///
/// Returns short strings like "in 3d", "2d ago", "today", or a short
/// absolute date for far-out events. `now_ms` is passed in so callers can
/// snapshot it once per render pass (Date::now is not reactive).
fn date_hint(start_ms: i64, now_ms: i64) -> String {
    if start_ms <= 0 {
        return "—".to_string();
    }
    let diff_ms = start_ms - now_ms;
    let diff_days = diff_ms / 86_400_000;
    if diff_days == 0 {
        "today".to_string()
    } else if diff_days == 1 {
        "tomorrow".to_string()
    } else if diff_days == -1 {
        "yesterday".to_string()
    } else if (2..=7).contains(&diff_days) {
        format!("in {diff_days}d")
    } else if (-7..=-2).contains(&diff_days) {
        format!("{}d ago", -diff_days)
    } else {
        // Absolute short date: "Mar 15" (same year) or "Mar 15, 2025"
        let date = js_sys::Date::new(&(start_ms as f64).into());
        let months = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let m = months.get(date.get_month() as usize).copied().unwrap_or("?");
        let day = date.get_date();
        let year = date.get_full_year();
        let cur_year = js_sys::Date::new(&(now_ms as f64).into()).get_full_year();
        if year == cur_year {
            format!("{m} {day}")
        } else {
            format!("{m} {day}, {year}")
        }
    }
}

/// Status badge glyph + color class. Used by both the trigger and list items.
fn status_badge(status: &EventStatus) -> (&'static str, &'static str) {
    match status {
        EventStatus::Active => ("●", "admin-evt-status-active"),
        EventStatus::Draft => ("○", "admin-evt-status-draft"),
        EventStatus::Completed => ("✓", "admin-evt-status-done"),
        EventStatus::Archived => ("—", "admin-evt-status-archived"),
    }
}

/// Sort key tuple: `(group, secondary)`. Lower tuple = earlier in the list.
///
/// Groups (see module docs):
///   0 = Active + upcoming → secondary is `start_ms` (ASC: soonest first)
///   1 = Active + past     → secondary is `-start_ms` (DESC: most recent first)
///   2 = Draft             → secondary is `start_ms` (ASC)
///   3 = Completed         → secondary is `-start_ms` (DESC)
///   4 = Archived          → unreachable (filtered before sort)
fn sort_key(e: &EventMeta, now_ms: i64) -> (u8, i64) {
    let upcoming = e.event_start_ms >= now_ms;
    match e.status {
        EventStatus::Active if upcoming => (0, e.event_start_ms),
        EventStatus::Active => (1, -e.event_start_ms),
        EventStatus::Draft => (2, e.event_start_ms),
        EventStatus::Completed => (3, -e.event_start_ms),
        EventStatus::Archived => (4, 0),
    }
}

/// Searchable, sortable event selector for the admin sidebar.
///
/// Drop-in replacement for the native `<select>`. Pass the same signals the
/// old select consumed — this component manages its own open/close + filter
/// state internally and calls `set_active_event_id` + bumps
/// `set_refresh_counter` on selection, exactly like the old `on:change`.
#[component]
pub fn AdminEventSelector(
    events_list: ReadSignal<Vec<EventMeta>>,
    active_event_id: ReadSignal<Option<String>>,
    set_active_event_id: WriteSignal<Option<String>>,
    set_refresh_counter: WriteSignal<u32>,
) -> impl IntoView {
    let (open, set_open) = signal(false);
    let (query, set_query) = signal(String::new());
    let search_ref = NodeRef::<leptos::html::Input>::new();

    // Sorted + filtered view of the events list.
    // Recomputes only when events_list or query changes (memoized).
    let visible_events = Memo::new(move |_| {
        let now_ms = js_sys::Date::now() as i64;
        let q = query.get().to_ascii_lowercase();
        let mut events: Vec<EventMeta> = events_list
            .get()
            .into_iter()
            .filter(|e| e.status != EventStatus::Archived)
            .filter(|e| {
                if q.is_empty() {
                    return true;
                }
                e.name.to_ascii_lowercase().contains(&q)
                    || e.slug.to_ascii_lowercase().contains(&q)
            })
            .collect();
        events.sort_by(|a, b| sort_key(a, now_ms).cmp(&sort_key(b, now_ms)));
        events
    });

    // Currently selected event (drives the trigger button label).
    let selected = Memo::new(move |_| {
        let id = active_event_id.get()?;
        events_list.get().iter().find(|e| e.id == id).cloned()
    });

    // Focus the search input shortly after the dropdown opens.
    // The short delay lets the <Show> render the input into the DOM before
    // we try to focus it — otherwise NodeRef::get returns None on the same
    // tick the open signal flips.
    Effect::new(move |_| {
        if open.get() {
            let search_ref = search_ref.clone();
            leptos::task::spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(20).await;
                if let Some(input) = search_ref.get() {
                    let _ = input.focus();
                }
            });
        }
    });

    // Shared selection handler — used by both item clicks and Enter key.
    let select_event = move |id: String| {
        set_active_event_id.set(Some(id));
        set_refresh_counter.update(|c| *c += 1);
        set_open.set(false);
        set_query.set(String::new());
    };

    view! {
        <div class="admin-evt-selector" class:open=move || open.get()>
            // ── Trigger button ──
            // stop_propagation prevents the backdrop's click handler from
            // immediately closing the dropdown when the trigger is toggled.
            <button
                type="button"
                class="admin-evt-trigger"
                on:click=move |ev| {
                    ev.stop_propagation();
                    set_open.update(|o| *o = !*o);
                    if !open.get() {
                        set_query.set(String::new());
                    }
                }
            >
                {move || match selected.get() {
                    Some(e) => {
                        let (badge, cls) = status_badge(&e.status);
                        let hint = date_hint(e.event_start_ms, js_sys::Date::now() as i64);
                        let full_name = e.name.clone();
                        view! {
                            <span class=format!("admin-evt-badge {cls}")>{badge}</span>
                            <span class="admin-evt-label" title=full_name.clone()>
                                <span class="admin-evt-name">{e.name.clone()}</span>
                                <span class="admin-evt-hint">{hint}</span>
                            </span>
                        }.into_any()
                    }
                    None => view! {
                        <span class="admin-evt-placeholder">"Select an event…"</span>
                    }.into_any(),
                }}
                <svg
                    class="admin-evt-chevron"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                >
                    <polyline points="6 9 12 15 18 9"></polyline>
                </svg>
            </button>

            // ── Dropdown (open state) ──
            <Show when=move || open.get() fallback=|| view! { <div></div> }>
                // Full-viewport backdrop: catches outside clicks to close.
                // Fixed + high z so it sits above page chrome (incl. the
                // admin sidebar's overflow container).
                <div class="admin-evt-backdrop" on:click=move |_| set_open.set(false)></div>

                <div class="admin-evt-panel">
                    // Search input
                    <div class="admin-evt-search">
                        <svg
                            class="admin-evt-search-icon"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        >
                            <circle cx="11" cy="11" r="8"></circle>
                            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                        </svg>
                        <input
                            node_ref=search_ref
                            type="text"
                            class="admin-evt-search-input"
                            placeholder="Filter events…"
                            prop:value=move || query.get()
                            on:input=move |ev| set_query.set(event_target_value(&ev))
                            on:click=move |ev| ev.stop_propagation()
                            on:keydown=move |ev| {
                                let key = ev.key();
                                if key == "Escape" {
                                    set_open.set(false);
                                    set_query.set(String::new());
                                } else if key == "Enter" {
                                    if let Some(first) = visible_events.get().first() {
                                        select_event(first.id.clone());
                                    }
                                }
                            }
                        />
                    </div>

                    // Event list
                    <div class="admin-evt-list">
                        {move || {
                            let now = js_sys::Date::now() as i64;
                            let events = visible_events.get();
                            if events.is_empty() {
                                view! {
                                    <div class="admin-evt-empty">"No matching events"</div>
                                }.into_any()
                            } else {
                                events
                                    .iter()
                                    .map(|e| {
                                        let id = e.id.clone();
                                        let id_for_selected = id.clone();
                                        let id_for_click = id.clone();
                                        let (badge, cls) = status_badge(&e.status);
                                        let hint = date_hint(e.event_start_ms, now);
                                        let name = e.name.clone();
                                        let name_for_title = name.clone();
                                        view! {
                                            <button
                                                type="button"
                                                class="admin-evt-item"
                                                title=name_for_title.clone()
                                                class:selected=move || {
                                                    active_event_id.get().as_deref() == Some(&id_for_selected)
                                                }
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    select_event(id_for_click.clone());
                                                }
                                            >
                                                <span class=format!("admin-evt-badge {cls}")>{badge}</span>
                                                <span class="admin-evt-item-label">
                                                    <span class="admin-evt-item-name">{name}</span>
                                                    <span class="admin-evt-item-hint">{hint}</span>
                                                </span>
                                                <Show
                                                    when=move || {
                                                        active_event_id.get().as_deref() == Some(&id)
                                                    }
                                                    fallback=|| view! { <div></div> }
                                                >
                                                    <svg
                                                        class="admin-evt-check"
                                                        viewBox="0 0 24 24"
                                                        fill="none"
                                                        stroke="currentColor"
                                                        stroke-width="2"
                                                        stroke-linecap="round"
                                                        stroke-linejoin="round"
                                                    >
                                                        <polyline points="20 6 9 17 4 12"></polyline>
                                                    </svg>
                                                </Show>
                                            </button>
                                        }.into_any()
                                    })
                                    .collect::<Vec<_>>()
                                    .into_any()
                            }
                        }}
                    </div>
                </div>
            </Show>
        </div>
    }
}
