//! Collapsible audit trail panel — loaded on demand when expanded.
//!
//! Shown in the event detail context bar during Edit view.
//! Fetches audit entries from the backend only when the user opens the panel.

use leptos::prelude::*;

use crate::api;

/// Collapsible panel that displays the audit trail for a specific event.
///
/// Loads audit entries lazily — only fetches from the API the first time the
/// panel is opened. Subsequent open/close toggles use the cached data.
#[component]
pub fn AuditPanel(event_id: String) -> impl IntoView {
    let (open, set_open) = signal(false);
    let (entries, set_entries) = signal(Vec::<api::AuditEntry>::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let load_audit = move || {
        let eid = event_id.clone();
        leptos::task::spawn_local(async move {
            set_loading.set(true);
            set_error.set(None);
            match api::get_event_audit(&eid).await {
                Ok(data) => {
                    set_entries.set(data.entries);
                }
                Err(e) => {
                    set_error.set(Some(e.to_string()));
                }
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="form-section" style="margin-top: 1rem">
            <div class="form-section-header" on:click=move |_| {
                let was_open = open.get();
                set_open.set(!was_open);
                if !was_open && entries.get().is_empty() {
                    load_audit();
                }
            }>
                <span class="form-section-icon" style="background: #6b7280">"📋"</span>
                <span class="form-section-title">"Audit Trail"</span>
                <span class="form-section-badge" style="background: #f3f4f6; color: #374151">
                    {move || {
                        let count = entries.get().len();
                        if count > 0 { format!("{count} entries") } else { "Click to load".to_string() }
                    }}
                </span>
                <span class="form-section-toggle" class:form-section-toggle-open=move || open.get()>"▼"</span>
            </div>
            <div class="form-section-body" class:form-section-body-hidden=move || !open.get()>
                <Show when=move || loading.get() fallback=|| view! { <div></div> }>
                    <div style="text-align: center; padding: 1rem; color: #6b7280">"Loading audit trail..."</div>
                </Show>
                <Show when=move || error.get().is_some() && !loading.get() fallback=|| view! { <div></div> }>
                    <div style="color: #dc2626; padding: 0.5rem">
                        {move || error.get().unwrap_or_default()}
                    </div>
                </Show>
                <Show when=move || !loading.get() && error.get().is_none() && entries.get().is_empty() fallback=|| view! { <div></div> }>
                    <div style="text-align: center; padding: 1rem; color: #6b7280">"No audit entries found"</div>
                </Show>
                <Show when=move || !entries.get().is_empty() && !loading.get() fallback=|| view! { <div></div> }>
                    <div class="audit-timeline" style="max-height: 400px; overflow-y: auto">
                        {move || entries.get().into_iter().map(|e| {
                            let ts = format_timestamp(&e.timestamp);
                            let action_class = action_badge_class(&e.action);
                            view! {
                                <div class="audit-entry" style="display: flex; gap: 0.75rem; padding: 0.5rem 0; border-bottom: 1px solid #f3f4f6; font-size: 0.85rem">
                                    <div style="flex-shrink: 0; width: 130px; color: #6b7280">{ts}</div>
                                    <div style="flex-shrink: 0">
                                        <span class=action_class style="padding: 2px 6px; border-radius: 4px; font-size: 0.75rem; white-space: nowrap">
                                            {format_action(&e.action)}
                                        </span>
                                    </div>
                                    <div style="flex: 1; color: #374151">{e.description.clone()}</div>
                                    <div style="flex-shrink: 0; color: #9ca3af; font-size: 0.75rem">{e.actor.clone()}</div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                </Show>
            </div>
        </div>
    }
}

/// Format an ISO 8601 / RFC 3339 timestamp into a human-readable string.
///
/// Uses `js_sys::Date` for parsing (no `chrono` dependency in the frontend).
/// Example: `"2025-01-15T10:30:00+00:00"` → `"Jan 15, 10:30"`
fn format_timestamp(ts: &str) -> String {
    let parsed = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(ts));
    if parsed.get_time().is_nan() {
        // Fallback: show first 16 chars of the raw string
        return ts.chars().take(16).collect();
    }

    let month = match parsed.get_month() as u8 {
        0 => "Jan",
        1 => "Feb",
        2 => "Mar",
        3 => "Apr",
        4 => "May",
        5 => "Jun",
        6 => "Jul",
        7 => "Aug",
        8 => "Sep",
        9 => "Oct",
        10 => "Nov",
        11 => "Dec",
        _ => "???",
    };
    let day = parsed.get_date();
    let hours = parsed.get_hours();
    let minutes = parsed.get_minutes();

    format!("{month} {day:02}, {hours:02}:{minutes:02}")
}

/// Convert a `snake_case` action string to Title Case for display.
fn format_action(action: &str) -> String {
    action
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Return a CSS badge class based on the action category.
fn action_badge_class(action: &str) -> String {
    if action.starts_with("event_") {
        "badge badge-info-xs".to_string()
    } else if action.starts_with("escrow_") || action.starts_with("deposit_") {
        "badge badge-warning-xs".to_string()
    } else if action.starts_with("attendee_") || action == "walkin_registered" {
        "badge badge-success-xs".to_string()
    } else if action.contains("delete") || action.contains("reject") {
        "badge badge-error-xs".to_string()
    } else {
        "badge".to_string()
    }
}
