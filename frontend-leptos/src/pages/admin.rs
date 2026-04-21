use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::api::{self, AttendeeResponse, GenerateQrData, StatsResponse};
use crate::auth::{self, handle_token_from_url, is_authenticated};

// ===== Toast Notification =====

/// Toast notification type.
#[derive(Clone)]
struct ToastMessage {
    text: String,
    toast_type: ToastType,
}

#[derive(Clone, Copy)]
enum ToastType {
    Success,
    Error,
    Warning,
    Info,
}

/// Show a toast notification that auto-dismisses after 4 seconds.
fn show_toast(
    set_toast: &WriteSignal<Option<ToastMessage>>,
    text: &str,
    toast_type: ToastType,
) {
    set_toast.set(Some(ToastMessage {
        text: text.to_string(),
        toast_type,
    }));

    let set_toast = *set_toast;
    set_timeout(
        move || {
            set_toast.set(None);
        },
        std::time::Duration::from_secs(4),
    );
}

// ===== Helpers =====

/// Format an ISO timestamp to a readable string.
fn format_timestamp(iso: &str) -> String {
    if iso.is_empty() {
        return "N/A".to_string();
    }
    let js_date = js_sys::Date::new_with_year_month_day_hr_min_sec(0, 0, 0, 0, 0, 0.0);
    js_date.set_time(js_sys::Date::parse(iso));
    if js_date.get_time().is_nan() {
        return iso.to_string();
    }
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("year"),
        &wasm_bindgen::JsValue::from_str("numeric"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("month"),
        &wasm_bindgen::JsValue::from_str("short"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("day"),
        &wasm_bindgen::JsValue::from_str("numeric"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("hour"),
        &wasm_bindgen::JsValue::from_str("2-digit"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("minute"),
        &wasm_bindgen::JsValue::from_str("2-digit"),
    );
    js_date
        .to_locale_string("en-US", &opts)
        .as_string()
        .unwrap_or_else(|| iso.to_string())
}

/// Format a relative time string (e.g., "5 minutes ago").
fn time_ago(iso: &str) -> String {
    if iso.is_empty() {
        return String::new();
    }
    let js_date = js_sys::Date::new_with_year_month_day_hr_min_sec(0, 0, 0, 0, 0, 0.0);
    js_date.set_time(js_sys::Date::parse(iso));
    if js_date.get_time().is_nan() {
        return String::new();
    }
    let now_ms = js_sys::Date::now();
    let date_ms = js_date.get_time();
    let seconds = ((now_ms - date_ms) / 1000.0) as i64;

    if seconds < 60 {
        return "just now".to_string();
    }
    if seconds < 3600 {
        return format!("{}m ago", seconds / 60);
    }
    if seconds < 86400 {
        return format!("{}h ago", seconds / 3600);
    }
    format!("{}d ago", seconds / 86400)
}

/// Escape HTML special characters to prevent XSS.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ===== Admin Component =====

/// Admin dashboard page component.
///
/// Displays check-in statistics, attendee list with search,
/// QR code generation, and recent check-in history.
#[component]
pub fn Admin() -> impl IntoView {
    let navigate = use_navigate();

    // Auth state
    let (user_email, set_user_email) = signal(String::new());

    // Data state
    let (attendees, set_attendees) = signal(Vec::<AttendeeResponse>::new());
    let (stats, set_stats) = signal(None::<StatsResponse>);
    let (search_query, set_search_query) = signal(String::new());
    let (is_loading, set_is_loading) = signal(true);
    let (qr_generating, set_qr_generating) = signal(false);
    let (qr_result, set_qr_result) = signal(None::<GenerateQrData>);
    let (toast, set_toast) = signal(None::<ToastMessage>);

    // Refresh counter — increment to trigger data reload
    let (refresh_counter, set_refresh_counter) = signal(0u32);

    // Filtered attendees: derived from search query
    let filtered_attendees = Memo::new(move |_| {
        let query = search_query.get().to_lowercase();
        let list = attendees.get();
        let mut filtered: Vec<AttendeeResponse> = if query.is_empty() {
            list.iter().cloned().collect()
        } else {
            list.iter()
                .filter(|a| {
                    let name = a.name.to_lowercase();
                    let email = a.email.to_lowercase();
                    let api_id = a.api_id.to_lowercase();
                    let ticket = a.ticket_name.to_lowercase();
                    name.contains(&query)
                        || email.contains(&query)
                        || api_id.contains(&query)
                        || ticket.contains(&query)
                })
                .cloned()
                .collect()
        };

        // Sort: not checked in first, then by name
        filtered.sort_by(|a, b| {
            let a_checked = a.checked_in_at.is_some();
            let b_checked = b.checked_in_at.is_some();
            match (a_checked, b_checked) {
                (false, true) => std::cmp::Ordering::Less,
                (true, false) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        filtered
    });

    // On mount: handle token from URL, check auth, load user info
    Effect::new(move |_| {
        // Handle token from OAuth callback
        handle_token_from_url();

        // Require authentication
        if !is_authenticated() {
            auth::clear_token();
            navigate("/", Default::default());
            return;
        }

        // Load user info
        spawn_local(async move {
            match api::get_me().await {
                Ok(me) => {
                    set_user_email.set(me.email);
                }
                Err(_) => {
                    log::warn!("[admin] failed to load user info");
                }
            }
        });
    });

    // Data loading effect — triggered by refresh_counter changes
    Effect::new(move |_| {
        let _ = refresh_counter.get(); // track refresh counter
        set_is_loading.set(true);

        spawn_local(async move {
            match api::get_attendees().await {
                Ok(data) => {
                    set_attendees.set(data.attendees);
                    set_stats.set(Some(data.stats));
                }
                Err(err) => {
                    log::error!("[admin] failed to load dashboard: {err}");
                    show_toast(
                        &set_toast,
                        &format!("Failed to load dashboard: {err}"),
                        ToastType::Error,
                    );
                }
            }
            set_is_loading.set(false);
        });
    });

    // Handle refresh button click
    let handle_refresh = move |_| {
        set_refresh_counter.update(|c| *c += 1);
    };

    // Handle QR code generation
    let handle_generate_qrs = move |_| {
        if qr_generating.get() {
            return;
        }
        set_qr_generating.set(true);
        set_qr_result.set(None);

        spawn_local(async move {
            match api::generate_qrs().await {
                Ok(result) => {
                    log::info!(
                        "[admin] QR generation complete: {} generated, {} skipped",
                        result.generated,
                        result.skipped
                    );
                    show_toast(
                        &set_toast,
                        &format!(
                            "Generated {} QR codes ({} skipped)",
                            result.generated, result.skipped
                        ),
                        ToastType::Success,
                    );
                    set_qr_result.set(Some(result));
                    // Refresh data to reflect new QR codes
                    set_refresh_counter.update(|c| *c += 1);
                }
                Err(err) => {
                    log::error!("[admin] QR generation failed: {err}");
                    show_toast(
                        &set_toast,
                        &format!("QR generation failed: {err}"),
                        ToastType::Error,
                    );
                }
            }
            set_qr_generating.set(false);
        });
    };

    // Handle sign out
    let handle_sign_out = move |_| {
        auth::logout(&|path: &str| navigate(path, Default::default()));
    };

    view! {
        <div>
            // Header
            <header class="header">
                <div class="header-inner">
                    <span class="header-title">"📊 Admin Dashboard"</span>
                    <div style="display:flex;align-items:center;gap:0.75rem;">
                        <span class="header-user">{move || user_email.get()}</span>
                        <button class="btn btn-outline btn-sm" on:click=handle_sign_out>
                            "Sign Out"
                        </button>
                    </div>
                </div>
            </header>

            <div class="page-container">
                // Loading state
                <Show
                    when=move || is_loading.get() && attendees.get().is_empty()
                    fallback=|| view! { <div></div> }
                >
                    <div class="page-loading">
                        <span class="spinner spinner-lg"></span>
                        "Loading dashboard..."
                    </div>
                </Show>

                // Dashboard content
                <Show
                    when=move || !is_loading.get() || !attendees.get().is_empty()
                    fallback=|| view! { <div></div> }
                >
                    // Action buttons row
                    <div style="display:flex;gap:0.5rem;margin-bottom:1rem;flex-wrap:wrap;">
                        <button class="btn btn-outline btn-sm" on:click=handle_refresh>
                            "🔄 Refresh"
                        </button>
                        <button
                            class="btn btn-primary btn-sm"
                            on:click=handle_generate_qrs
                            disabled=move || qr_generating.get()
                        >
                            {move || {
                                if qr_generating.get() {
                                    "⏳ Generating...".to_string()
                                } else {
                                    "🏷️ Generate QR Codes".to_string()
                                }
                            }}
                        </button>
                    </div>

                    // QR generation result
                    <Show
                        when=move || qr_result.get().is_some()
                        fallback=|| view! { <div></div> }
                    >
                        {move || {
                            let result = qr_result.get();
                            match result {
                                Some(r) => {
                                    let generated_details: Vec<_> = r.details.iter()
                                        .filter(|d| d.status == "generated")
                                        .collect();
                                    view! {
                                        <div class="card mb-2">
                                            <h3>"QR Generation Complete"</h3>
                                            <div class="stats-grid" style="margin-top:0.75rem;">
                                                <div class="stat-card info">
                                                    <div class="stat-value">{r.total}</div>
                                                    <div class="stat-label">"Total"</div>
                                                </div>
                                                <div class="stat-card success">
                                                    <div class="stat-value">{r.generated}</div>
                                                    <div class="stat-label">"Generated"</div>
                                                </div>
                                                <div class="stat-card warning">
                                                    <div class="stat-value">{r.skipped}</div>
                                                    <div class="stat-label">"Skipped"</div>
                                                </div>
                                            </div>
                                            <Show
                                                when=move || !generated_details.is_empty()
                                                fallback=|| view! { <div></div> }
                                            >
                                                <div style="margin-top:1rem;max-height:200px;overflow-y:auto;">
                                                    {generated_details.iter().map(|d| {
                                                        view! {
                                                            <div class="attendee-item" style="padding:0.5rem 0.75rem;">
                                                                <div class="attendee-info">
                                                                    <div class="attendee-name" style="font-size:0.85rem;">
                                                                        {escape_html(&d.name)}
                                                                    </div>
                                                                    <div class="attendee-email" style="font-size:0.7rem;">
                                                                        {escape_html(&d.api_id)}
                                                                    </div>
                                                                </div>
                                                                <span class="badge badge-success" style="font-size:0.7rem;">"Generated"</span>
                                                            </div>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </Show>
                                        </div>
                                    }.into_any()
                                }
                                None => view! { <div></div> }.into_any(),
                            }
                        }}
                    </Show>

                    // Stats cards
                    {move || {
                        let s = stats.get();
                        match s {
                            Some(s) => {
                                let total = s.total_approved;
                                let checked_in = s.total_checked_in;
                                let remaining = s.total_remaining;
                                let percentage = s.check_in_percentage;
                                view! {
                                    <div class="stats-grid">
                                        <div class="stat-card info">
                                            <div class="stat-value">{total}</div>
                                            <div class="stat-label">"Total"</div>
                                        </div>
                                        <div class="stat-card success">
                                            <div class="stat-value">{checked_in}</div>
                                            <div class="stat-label">"Checked In"</div>
                                        </div>
                                        <div class="stat-card warning">
                                            <div class="stat-value">{remaining}</div>
                                            <div class="stat-label">"Remaining"</div>
                                        </div>
                                    </div>

                                    // Progress bar
                                    <div class="card mb-2">
                                        <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:0.5rem;">
                                            <span style="font-size:0.85rem;font-weight:600;color:var(--text-primary);">
                                                "Check-In Progress"
                                            </span>
                                            <span style="font-size:0.85rem;color:var(--text-secondary);">
                                                {format!("{percentage:.1}% ({checked_in} / {total})")}
                                            </span>
                                        </div>
                                        <div class="progress-bar">
                                            <div
                                                class="progress-fill"
                                                style=move || format!("width: {percentage}%")
                                            ></div>
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            None => view! { <div></div> }.into_any(),
                        }
                    }}

                    // Search box
                    <div class="search-box">
                        <span class="search-icon">"🔍"</span>
                        <input
                            type="text"
                            placeholder="Search by name, email, ID, or ticket..."
                            prop:value=move || search_query.get()
                            on:input=move |ev| {
                                let val = event_target_value(&ev);
                                set_search_query.set(val);
                            }
                        />
                    </div>

                    // Attendee count
                    <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:0.75rem;">
                        <span style="font-size:0.85rem;color:var(--text-secondary);">
                            {move || {
                                let count = filtered_attendees.get().len();
                                format!("{count} attendee{}", if count != 1 { "s" } else { "" })
                            }}
                        </span>
                    </div>

                    // Attendee list
                    <div class="attendee-list">
                        <Show
                            when=move || filtered_attendees.get().is_empty()
                            fallback=|| view! { <div></div> }
                        >
                            <div style="text-align:center;padding:2rem;color:var(--text-muted);">
                                "No attendees found"
                            </div>
                        </Show>
                        {move || {
                            filtered_attendees.get()
                                .iter()
                                .map(|attendee| {
                                    let is_checked_in = attendee.checked_in_at.is_some();
                                    let badge_class = if is_checked_in { "badge badge-success" } else { "badge badge-warning" };
                                    let badge_text = if is_checked_in { "✓ Checked In" } else { "⏳ Pending" };
                                    let name = attendee.name.clone();
                                    let email = attendee.email.clone();
                                    let ticket = attendee.ticket_name.clone();
                                    let checked_at = attendee.checked_in_at.clone();
                                    let time_ago_str = checked_at.as_deref().map(time_ago).unwrap_or_default();

                                    view! {
                                        <div class="attendee-item">
                                            <div class="attendee-info">
                                                <div class="attendee-name">{escape_html(&name)}</div>
                                                <div class="attendee-email">{escape_html(&email)}</div>
                                                <Show
                                                    when=move || !ticket.is_empty()
                                                    fallback=|| view! { <div></div> }
                                                >
                                                    <div style="font-size:0.75rem;color:var(--text-muted);margin-top:2px;">
                                                        {escape_html(&ticket)}
                                                    </div>
                                                </Show>
                                            </div>
                                            <div class="attendee-status">
                                                <span class=badge_class>{badge_text}</span>
                                                <Show
                                                    when=move || is_checked_in && !time_ago_str.is_empty()
                                                    fallback=|| view! { <div></div> }
                                                >
                                                    <div style="font-size:0.7rem;color:var(--text-muted);margin-top:4px;text-align:right;">
                                                        {time_ago_str}
                                                    </div>
                                                </Show>
                                            </div>
                                        </div>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>

                    // Recent check-ins
                    {move || {
                        let s = stats.get();
                        match s {
                            Some(s) if !s.recent_check_ins.is_empty() => {
                                let recent: Vec<_> = {
                                    let mut r = s.recent_check_ins.clone();
                                    r.sort_by(|a, b| {
                                        let a_time = js_sys::Date::parse(&a.checked_in_at);
                                        let b_time = js_sys::Date::parse(&b.checked_in_at);
                                        b_time.partial_cmp(&a_time).unwrap_or(std::cmp::Ordering::Equal)
                                    });
                                    r.into_iter().take(10).collect()
                                };
                                view! {
                                    <div class="card mt-3">
                                        <h3 style="margin-bottom:0.75rem;">"Recent Check-Ins"</h3>
                                        <div class="attendee-list">
                                            {recent.iter().map(|check_in| {
                                                let name = check_in.name.clone();
                                                let api_id = check_in.api_id.clone();
                                                let at = check_in.checked_in_at.clone();
                                                let formatted = format_timestamp(&at);
                                                let ago = time_ago(&at);
                                                view! {
                                                    <div class="attendee-item">
                                                        <div class="attendee-info">
                                                            <div class="attendee-name">{escape_html(&name)}</div>
                                                            <div class="attendee-email" style="font-size:0.8rem;">
                                                                {escape_html(&api_id)}
                                                            </div>
                                                        </div>
                                                        <div class="attendee-status text-right">
                                                            <div style="font-size:0.8rem;color:var(--text-secondary);">
                                                                {formatted}
                                                            </div>
                                                            <div style="font-size:0.7rem;color:var(--text-muted);">
                                                                {ago}
                                                            </div>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            _ => view! { <div></div> }.into_any(),
                        }
                    }}

                    // Footer
                    <div class="footer">"Built with 🦀 Rust + Axum"</div>
                </Show>
            </div>

            // Toast notification
            <AdminToast toast_signal=toast />
        </div>
    }
}

// ===== Toast Component =====

/// Toast notification component for the admin page.
#[component]
fn AdminToast(toast_signal: ReadSignal<Option<ToastMessage>>) -> impl IntoView {
    view! {
        <Show
            when=move || toast_signal.get().is_some()
            fallback=|| view! { <div></div> }
        >
            {move || {
                let msg = toast_signal.get();
                match msg {
                    Some(m) => {
                        let style = match m.toast_type {
                            ToastType::Success => "background:rgba(34,197,94,0.15);border:1px solid rgba(34,197,94,0.4);color:#22c55e;",
                            ToastType::Error => "background:rgba(239,68,68,0.15);border:1px solid rgba(239,68,68,0.4);color:#ef4444;",
                            ToastType::Warning => "background:rgba(245,158,11,0.15);border:1px solid rgba(245,158,11,0.4);color:#f59e0b;",
                            ToastType::Info => "background:rgba(59,130,246,0.15);border:1px solid rgba(59,130,246,0.4);color:#3b82f6;",
                        };
                        view! {
                            <div
                                style=move || format!(
                                    "position:fixed;top:1rem;right:1rem;padding:0.85rem 1.25rem;border-radius:8px;font-size:0.9rem;font-weight:500;z-index:9999;max-width:360px;{}",
                                    style,
                                )
                            >
                                {m.text}
                            </div>
                        }.into_any()
                    }
                    None => view! { <div></div> }.into_any(),
                }
            }}
        </Show>
    }
}
