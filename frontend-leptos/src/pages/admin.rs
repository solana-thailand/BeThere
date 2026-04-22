//! Admin dashboard page — stats, attendee list, QR generation.
//!
//! Features:
//! - Check-in statistics with progress bar
//! - In-Person / Online breakdown stat cards
//! - Attendee list with search, participation badges, check-in status
//! - QR code generation with force-regenerate option
//! - Recent check-in history
//!
//! Requires being wrapped in `<ProtectedRoute>` to provide
//! `ReadSignal<String>` (user email) via context.

use std::collections::HashMap;

use leptos::prelude::*;

use crate::api::{self, AttendeeResponse, GenerateQrData, StatsResponse};
use crate::auth;
use crate::components::{self, ToastType};
use crate::utils;

// ===== Admin Component =====

/// Admin dashboard page component.
#[component]
pub fn Admin() -> impl IntoView {
    // Get user email from ProtectedRoute context
    let user_email = use_context::<ReadSignal<String>>().unwrap_or_else(|| {
        log::error!(
            "[admin] no user_email in context — route not wrapped in \
                 ProtectedRoute?"
        );
        signal(String::new()).0
    });

    // Data state
    let (attendees, set_attendees) = signal(Vec::<AttendeeResponse>::new());
    let (stats, set_stats) = signal(None::<StatsResponse>);
    let (search_query, set_search_query) = signal(String::new());
    let (is_loading, set_is_loading) = signal(true);
    let (qr_generating, set_qr_generating) = signal(false);
    let (qr_result, set_qr_result) = signal(None::<GenerateQrData>);
    let (toast, set_toast) = signal(None::<components::ToastMessage>);

    // Refresh counter — increment to trigger data reload
    let (refresh_counter, set_refresh_counter) = signal(0u32);

    // Filtered attendees: derived from search query
    let filtered_attendees = Memo::new(move |_| {
        let query = search_query.get().to_lowercase();
        let list = attendees.get();
        let mut filtered: Vec<AttendeeResponse> = if query.is_empty() {
            list.to_vec()
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

    // Data loading effect — triggered by refresh_counter changes
    Effect::new(move |_| {
        let _ = refresh_counter.get(); // track refresh counter
        set_is_loading.set(true);

        leptos::task::spawn_local(async move {
            match api::get_attendees().await {
                Ok(data) => {
                    set_attendees.set(data.attendees);
                    set_stats.set(Some(data.stats));
                }
                Err(err) => {
                    log::error!("[admin] failed to load dashboard: {err}");
                    components::show_toast(
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
    let handle_refresh = move |_: web_sys::MouseEvent| {
        set_refresh_counter.update(|c| *c += 1);
    };

    // Handle QR code generation (normal)
    let handle_generate_qrs = move |_: web_sys::MouseEvent| {
        if qr_generating.get() {
            return;
        }
        spawn_qr_generation(
            false,
            set_qr_generating,
            set_qr_result,
            set_toast,
            set_refresh_counter,
        );
    };

    // Handle QR code generation (force)
    let handle_force_generate_qrs = move |_: web_sys::MouseEvent| {
        if qr_generating.get() {
            return;
        }
        spawn_qr_generation(
            true,
            set_qr_generating,
            set_qr_result,
            set_toast,
            set_refresh_counter,
        );
    };

    // Handle sign out
    let handle_sign_out = move |_: web_sys::MouseEvent| {
        auth::logout();
    };

    // Compute show_loading (once, used in view)
    let show_loading = move || is_loading.get() && attendees.get().is_empty();
    let show_content = move || !is_loading.get() || !attendees.get().is_empty();

    view! {
        <div>
            <components::AppHeader
                title="📊 Admin Dashboard"
                user_email=user_email
                on_sign_out=handle_sign_out
            />

            <div class="page-container">
                // Loading state
                <Show when=show_loading fallback=|| view! { <div></div> }>
                    <div class="page-loading">
                        <span class="spinner spinner-lg"></span>
                        "Loading dashboard..."
                    </div>
                </Show>

                // Dashboard content
                <Show when=show_content fallback=|| view! { <div></div> }>
                    // Action buttons row
                    <div style="display:flex;gap:0.5rem;margin-bottom:1rem;flex-wrap:wrap;">
                        <button class="btn btn-outline btn-sm" on:click=handle_refresh>
                            "🔄 Refresh"
                        </button>
                        <a href="/staff" class="btn btn-outline btn-sm" style="text-decoration:none;">
                            "🎫 Scanner"
                        </a>
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
                        {move || render_qr_result(&qr_result.get())}
                        // Force regenerate button (shown after any generation)
                        <div style="margin-top:0.5rem;display:flex;align-items:center;gap:0.5rem;">
                            <button class="btn btn-outline btn-sm" on:click=handle_force_generate_qrs>
                                "🔄 Force Regenerate All"
                            </button>
                            <span style="font-size:0.75rem;color:var(--text-muted);">
                                "Overwrites existing QR URLs"
                            </span>
                        </div>
                    </Show>

                    // Stats cards
                    {move || render_stats(&stats.get(), &attendees.get())}

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
                        {move || render_attendee_list(&filtered_attendees.get())}
                    </div>

                    // Recent check-ins
                    {move || render_recent_check_ins(&stats.get(), &attendees.get())}

                    // Footer
                    <div class="footer">"Built with 🦀 Rust (Leptos + Axum)"</div>
                </Show>
            </div>

            <components::Toast toast_signal=toast />
        </div>
    }
}

// ===== QR Generation Logic =====

/// Spawn a QR generation request (normal or forced).
fn spawn_qr_generation(
    force: bool,
    set_qr_generating: WriteSignal<bool>,
    set_qr_result: WriteSignal<Option<GenerateQrData>>,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    set_refresh_counter: WriteSignal<u32>,
) {
    set_qr_generating.set(true);
    set_qr_result.set(None);

    leptos::task::spawn_local(async move {
        match api::generate_qrs(force).await {
            Ok(result) => {
                log::info!(
                    "[admin] QR generation complete (force={force}): {} \
                     generated, {} skipped",
                    result.generated,
                    result.skipped
                );

                if result.generated > 0 {
                    components::show_toast(
                        &set_toast,
                        &format!(
                            "Generated {} QR codes ({} skipped)",
                            result.generated, result.skipped
                        ),
                        ToastType::Success,
                    );
                } else {
                    components::show_toast(
                        &set_toast,
                        &format!(
                            "All {} approved attendees already have QR \
                             codes. Use \"Force Regenerate\" to overwrite.",
                            result.skipped
                        ),
                        ToastType::Warning,
                    );
                }

                set_qr_result.set(Some(result));
                // Refresh data to reflect new QR codes
                set_refresh_counter.update(|c| *c += 1);
            }
            Err(err) => {
                log::error!("[admin] QR generation failed: {err}");
                components::show_toast(
                    &set_toast,
                    &format!("QR generation failed: {err}"),
                    ToastType::Error,
                );
            }
        }
        set_qr_generating.set(false);
    });
}

// ===== Render Helpers =====

/// Render stats cards including In-Person/Online breakdown.
fn render_stats(stats: &Option<StatsResponse>, attendees: &[AttendeeResponse]) -> AnyView {
    match stats {
        Some(s) => {
            let total = s.total_approved;
            let checked_in = s.total_checked_in;
            let remaining = s.total_remaining;
            let percentage = s.check_in_percentage;

            // Compute participation type counts from attendees
            let in_person_count = attendees
                .iter()
                .filter(|a| utils::is_in_person(&a.participation_type))
                .count();
            let online_count = attendees.len().saturating_sub(in_person_count);

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

                // Participation type breakdown
                <div class="stats-grid" style="margin-top:0.5rem;">
                    <div class="stat-card" style="background:rgba(59,130,246,0.1);border-color:rgba(59,130,246,0.3);">
                        <div class="stat-value" style="color:#3b82f6;">{in_person_count}</div>
                        <div class="stat-label">"In-Person"</div>
                    </div>
                    <div class="stat-card" style="background:rgba(245,158,11,0.1);border-color:rgba(245,158,11,0.3);">
                        <div class="stat-value" style="color:#f59e0b;">{online_count}</div>
                        <div class="stat-label">"Online"</div>
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
            }
                .into_any()
        }
        None => view! { <div></div> }.into_any(),
    }
}

/// Render the QR generation result panel with force regenerate option.
fn render_qr_result(result: &Option<GenerateQrData>) -> AnyView {
    match result {
        Some(r) => {
            let generated_details: Vec<_> = r
                .details
                .iter()
                .filter(|d| d.status == "generated")
                .cloned()
                .collect();
            let skipped_details: Vec<_> = r
                .details
                .iter()
                .filter(|d| d.status == "skipped")
                .cloned()
                .collect();
            let skipped_count = skipped_details.len();
            let has_generated = !generated_details.is_empty();
            let show_skipped_list = skipped_count > 0 && skipped_count <= 20;
            let show_skipped_summary = skipped_count > 20;

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

                    // Generated attendee details
                    <Show when=move || has_generated fallback=|| view! { <div></div> }>
                        <div style="margin-top:1rem;max-height:200px;overflow-y:auto;">
                            {generated_details.iter().map(|d| {
                                let name = d.name.clone();
                                let api_id = d.api_id.clone();
                                view! {
                                    <div class="attendee-item" style="padding:0.5rem 0.75rem;">
                                        <div class="attendee-info">
                                            <div class="attendee-name" style="font-size:0.85rem;">
                                                {utils::escape_html(&name)}
                                            </div>
                                            <div class="attendee-email" style="font-size:0.7rem;">
                                                {utils::escape_html(&api_id)}
                                            </div>
                                        </div>
                                        <span class="badge badge-success" style="font-size:0.7rem;">"Generated"</span>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    </Show>

                    // Skipped attendee details (show up to 20)
                    <Show when=move || show_skipped_list fallback=|| view! { <div></div> }>
                        <div style="margin-top:0.75rem;font-size:0.8rem;color:var(--text-muted);">
                            "Skipped (already have QR URLs):"
                        </div>
                        <div style="max-height:150px;overflow-y:auto;">
                            {skipped_details.iter().map(|d| {
                                let name = d.name.clone();
                                view! {
                                    <div class="attendee-item" style="padding:0.4rem;opacity:0.7;">
                                        <div class="attendee-info">
                                            <div class="attendee-name" style="font-size:0.8rem;">
                                                {utils::escape_html(&name)}
                                            </div>
                                        </div>
                                        <span class="badge badge-warning" style="font-size:0.65rem;">"Skipped"</span>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    </Show>

                    // Large skip count summary
                    <Show when=move || show_skipped_summary fallback=|| view! { <div></div> }>
                        <div style="font-size:0.8rem;color:var(--text-muted);margin-top:0.5rem;">
                            {format!("{skipped_count} attendees skipped (already have QR URLs)")}
                        </div>
                    </Show>
                </div>
            }
                .into_any()
        }
        None => view! { <div></div> }.into_any(),
    }
}

/// Render the attendee list with participation badges and check-in status.
fn render_attendee_list(filtered: &[AttendeeResponse]) -> AnyView {
    filtered
        .iter()
        .map(|attendee| {
            let is_checked_in = attendee.checked_in_at.is_some();
            let badge_class = if is_checked_in {
                "badge badge-success"
            } else {
                "badge badge-warning"
            };
            let badge_text = if is_checked_in {
                "✓ Checked In"
            } else {
                "⏳ Pending"
            };

            let participation =
                utils::get_participation_badge(&attendee.participation_type);
            let p_class = participation.css_class.to_string();
            let p_label = participation.label;

            let name = attendee.name.clone();
            let email = attendee.email.clone();
            let ticket = attendee.ticket_name.clone();
            let has_ticket = !ticket.is_empty();
            let time_ago_str = attendee
                .checked_in_at
                .as_deref()
                .map(utils::time_ago)
                .unwrap_or_default();
            let has_time_ago = is_checked_in && !time_ago_str.is_empty();

            view! {
                <div class="attendee-item">
                    <div class="attendee-info">
                        <div class="attendee-name">{utils::escape_html(&name)}</div>
                        <div class="attendee-email">{utils::escape_html(&email)}</div>
                        <Show
                            when=move || has_ticket
                            fallback=|| view! { <div></div> }
                        >
                            <div style="font-size:0.75rem;color:var(--text-muted);margin-top:2px;">
                                {utils::escape_html(&ticket)}
                            </div>
                        </Show>
                    </div>
                    <div class="attendee-status">
                        <span class=p_class.clone()>{p_label.clone()}</span>
                        <span class=badge_class>{badge_text}</span>
                        <Show
                            when=move || has_time_ago
                            fallback=|| view! { <div></div> }
                        >
                            <div style="font-size:0.7rem;color:var(--text-muted);margin-top:4px;text-align:right;">
                                {time_ago_str.clone()}
                            </div>
                        </Show>
                    </div>
                </div>
            }
        })
        .collect_view()
        .into_any()
}

/// Render the recent check-ins panel with participation badges.
fn render_recent_check_ins(
    stats: &Option<StatsResponse>,
    attendees: &[AttendeeResponse],
) -> AnyView {
    match stats {
        Some(s) if !s.recent_check_ins.is_empty() => {
            let recent: Vec<_> = {
                let mut r = s.recent_check_ins.clone();
                r.sort_by(|a, b| {
                    let a_time = js_sys::Date::parse(&a.checked_in_at);
                    let b_time = js_sys::Date::parse(&b.checked_in_at);
                    b_time
                        .partial_cmp(&a_time)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                r.into_iter().take(10).collect()
            };

            // Build a lookup map for participation type by api_id
            let participation_map: HashMap<String, String> = attendees
                .iter()
                .map(|a| (a.api_id.clone(), a.participation_type.clone()))
                .collect();

            view! {
                <div class="card mt-3">
                    <h3 style="margin-bottom:0.75rem;">"Recent Check-Ins"</h3>
                    <div class="attendee-list">
                        {recent.iter().map(|check_in| {
                            let name = check_in.name.clone();
                            let api_id = check_in.api_id.clone();
                            let at = check_in.checked_in_at.clone();
                            let formatted = utils::format_timestamp(&at);

                            let p_type = participation_map
                                .get(&api_id)
                                .cloned()
                                .unwrap_or_default();
                            let participation = utils::get_participation_badge(&p_type);
                            let p_class = participation.css_class.to_string();
                            let p_label = participation.label;

                            view! {
                                <div class="attendee-item">
                                    <div class="attendee-info">
                                        <div class="attendee-name">{utils::escape_html(&name)}</div>
                                        <div class="attendee-email" style="font-size:0.8rem;">
                                            {utils::escape_html(&api_id)}
                                        </div>
                                    </div>
                                    <div class="attendee-status text-right">
                                        <span class=p_class.clone() style="font-size:0.7rem;margin-bottom:4px;display:inline-block;">
                                            {p_label.clone()}
                                        </span>
                                        <div style="font-size:0.8rem;color:var(--text-secondary);">
                                            {formatted}
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                </div>
            }
                .into_any()
        }
        _ => view! { <div></div> }.into_any(),
    }
}
