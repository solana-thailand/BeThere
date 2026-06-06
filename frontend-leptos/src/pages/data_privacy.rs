use leptos::prelude::*;
use leptos_meta::Title;

use crate::api::{self, BlockedEvent};
use crate::icons::{Icon, IconName};

/// Format a blocked event's end_ms as a readable date.
fn format_available_date(end_ms: i64) -> String {
    let date = js_sys::Date::new(&(end_ms as f64).into());
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let day = date.get_date();
    let month = months[date.get_month() as usize % 12];
    let year = date.get_full_year();
    format!("{month} {day}, {year}")
}

#[derive(Clone)]
enum DeleteState {
    Result(api::DeleteRequestResponse),
    Error(String),
}

#[component]
pub fn DataPrivacy() -> impl IntoView {
    // Marketing unsubscribe state
    let (unsub_state, set_unsub_state) = signal::<Result<String, String>>(Ok(String::new()));
    let (unsub_loading, set_unsub_loading) = signal(false);

    // Data deletion state
    let (delete_state, set_delete_state) = signal::<Option<DeleteState>>(None);
    let (delete_loading, set_delete_loading) = signal(false);

    let on_unsubscribe = move || {
        set_unsub_loading.set(true);
        set_unsub_state.set(Ok(String::new()));
        leptos::task::spawn_local(async move {
            match api::unsubscribe_marketing().await {
                Ok(resp) => {
                    set_unsub_state.set(Ok(format!(
                        "Marketing preference updated. {} record(s) updated.",
                        resp.rows_updated
                    )));
                }
                Err(e) => {
                    set_unsub_state.set(Err(e.message));
                }
            }
            set_unsub_loading.set(false);
        });
    };

    let on_delete_request = move || {
        set_delete_loading.set(true);
        set_delete_state.set(None);
        leptos::task::spawn_local(async move {
            match api::request_data_deletion(None).await {
                Ok(resp) => {
                    set_delete_state.set(Some(DeleteState::Result(resp)));
                }
                Err(e) => {
                    set_delete_state.set(Some(DeleteState::Error(e.message)));
                }
            }
            set_delete_loading.set(false);
        });
    };

    view! {
        <Title text="Data & Privacy — BeThere" />
        <div class="center-page">
            <div class="container" style="max-width: 720px;">

                // Header
                <div class="pe-card">
                    <h1 class="pe-section-title" style="margin-bottom: 0.5rem;">
                        <Icon icon=IconName::Lock class="icon-md" />
                        " Data & Privacy"
                    </h1>
                    <p class="pe-detail-secondary">
                        "Manage your personal data preferences under Thailand's PDPA."
                    </p>
                </div>

                // Marketing Consent Section
                <div class="pe-card">
                    <h2 class="pe-section-title" style="font-size: 1.1rem;">
                        <Icon icon=IconName::Sound class="icon-sm" />
                        " Marketing Communications"
                    </h2>
                    <p class="pe-detail-secondary" style="margin-bottom: 0.75rem;">
                        "You can unsubscribe from marketing communications at any time. You'll still receive event-related notifications (registration confirmation, check-in info, etc.)."
                    </p>
                    <button
                        class="btn btn-outline btn-block"
                        disabled=move || unsub_loading.get()
                        on:click=move |_| on_unsubscribe()
                    >
                        {move || if unsub_loading.get() {
                            "Processing...".to_string()
                        } else {
                            "Unsubscribe from Marketing".to_string()
                        }}
                    </button>
                    {move || match &unsub_state.get() {
                        Ok(msg) if !msg.is_empty() => {
                            let m = msg.clone();
                            view! {
                                <div class="pe-success-box" style="margin-top: 0.5rem;">
                                    {m}
                                </div>
                            }.into_any()
                        }
                        Err(err) => {
                            let e = err.clone();
                            view! {
                                <div class="pe-error-box" style="margin-top: 0.5rem;">
                                    {e}
                                </div>
                            }.into_any()
                        }
                        _ => view! { <div></div> }.into_any(),
                    }}
                </div>

                // Data Deletion Section
                <div class="pe-card">
                    <h2 class="pe-section-title" style="font-size: 1.1rem;">
                        <Icon icon=IconName::Recycle class="icon-sm" />
                        " Request Data Deletion"
                    </h2>
                    <p class="pe-detail-secondary" style="margin-bottom: 0.75rem;">
                        "Request erasure of your personal data (PDPA Section 29). Data for upcoming or active events cannot be deleted until the event concludes (PDPA Section 38 — contract performance exemption)."
                    </p>
                    <button
                        class="btn btn-outline btn-block"
                        disabled=move || delete_loading.get()
                        on:click=move |_| on_delete_request()
                    >
                        {move || if delete_loading.get() {
                            "Processing...".to_string()
                        } else {
                            "Request Data Deletion".to_string()
                        }}
                    </button>

                    // Delete result
                    {move || match &delete_state.get() {
                        Some(DeleteState::Result(resp)) => {
                            let status = resp.status.clone();
                            let is_completed = status == "completed";
                            let is_blocked = status == "blocked";
                            let is_partial = status == "partial";

                            let blocked = resp.blocked_events.clone();
                            let affected = resp.events_affected;

                            view! {
                                <div class=if is_completed { "pe-success-box" } else { "pe-error-box" } style="margin-top: 0.75rem;">
                                    {if is_completed {
                                        view! {
                                            <div>
                                                <strong>"Deletion Completed"</strong>
                                                <p style="margin-top: 0.25rem;">
                                                    {format!("Personal data cleared from {} event(s).", affected)}
                                                </p>
                                            </div>
                                        }.into_any()
                                    } else if is_blocked {
                                        view! {
                                            <div>
                                                <strong>"Deletion Blocked"</strong>
                                                <p style="margin-top: 0.25rem;">
                                                    "Your data cannot be deleted yet because you have active/upcoming events."
                                                </p>
                                            </div>
                                        }.into_any()
                                    } else if is_partial {
                                        view! {
                                            <div>
                                                <strong>"Partial Deletion"</strong>
                                                <p style="margin-top: 0.25rem;">
                                                    {format!("Data deleted from {} event(s). Some events are still active.", affected)}
                                                </p>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}
                                </div>

                                // Show blocked events with dates
                                {if !blocked.is_empty() {
                                    let blocked_clone = blocked.clone();
                                    view! {
                                        <div style="margin-top: 0.75rem;">
                                            <p class="pe-detail-secondary" style="font-weight: 600; margin-bottom: 0.5rem;">
                                                "Blocked Events:"
                                            </p>
                                            {blocked_clone.into_iter().map(|ev: BlockedEvent| {
                                                let name = ev.event_name.clone();
                                                let available = format_available_date(ev.event_end_ms);
                                                view! {
                                                    <div class="ticket-action-card ticket-action-card--pending" style="margin-bottom: 0.5rem;">
                                                        <div class="ticket-action-icon">
                                                            <Icon icon=IconName::Clock class="icon-sm" />
                                                        </div>
                                                        <div>
                                                            <div class="ticket-action-title">{name}</div>
                                                            <div class="ticket-action-desc">
                                                                {format!("Available after {available}")}
                                                            </div>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}

                                // On-chain note
                                {if is_completed || is_partial {
                                    view! {
                                        <p class="pe-detail-secondary" style="margin-top: 0.5rem; font-size: 0.8rem;">
                                            "Note: On-chain data (wallet addresses, transaction signatures) is immutable and cannot be deleted. This is a technical limitation of blockchain technology."
                                        </p>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}
                            }.into_any()
                        }
                        Some(DeleteState::Error(err)) => {
                            let e = err.clone();
                            view! {
                                <div class="pe-error-box" style="margin-top: 0.75rem;">
                                    {e}
                                </div>
                            }.into_any()
                        }
                        None => view! { <div></div> }.into_any(),
                    }}
                </div>

                // Privacy Policy Link
                <div class="pe-card">
                    <p class="pe-detail-secondary">
                        "For full details on how we handle your data, see our "
                        <a href="/privacy" class="pe-ext-link">"Privacy Policy"</a>"."
                    </p>
                </div>

                // Back link
                <div style="text-align: center; margin-top: 0.5rem;">
                    <a href="/" class="btn btn-outline">"← Back to Home"</a>
                </div>
            </div>
        </div>
    }
}
