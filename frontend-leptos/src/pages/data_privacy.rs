use leptos::prelude::*;
use leptos_meta::Title;

use crate::api::{self, BlockedEvent};
use crate::i18n::{t, t_string, use_i18n};
use crate::icons::{Icon, IconName};

/// Outcome of the marketing unsubscribe. A count, not a sentence, so the
/// confirmation renders in the reader's language.
#[derive(Clone)]
enum UnsubState {
    Idle,
    Updated(usize),
    /// Server message, passed through as sent.
    Failed(String),
}

#[derive(Clone)]
enum DeleteState {
    Result(api::DeleteRequestResponse),
    Error(String),
}

#[component]
pub fn DataPrivacy() -> impl IntoView {
    let i18n = use_i18n();

    // Marketing unsubscribe state
    let (unsub_state, set_unsub_state) = signal(UnsubState::Idle);
    let (unsub_loading, set_unsub_loading) = signal(false);

    // Data deletion state
    let (delete_state, set_delete_state) = signal::<Option<DeleteState>>(None);
    let (delete_loading, set_delete_loading) = signal(false);

    let on_unsubscribe = move || {
        set_unsub_loading.set(true);
        set_unsub_state.set(UnsubState::Idle);
        leptos::task::spawn_local(async move {
            match api::unsubscribe_marketing().await {
                Ok(resp) => {
                    set_unsub_state.set(UnsubState::Updated(resp.rows_updated));
                }
                Err(e) => {
                    set_unsub_state.set(UnsubState::Failed(e.message));
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
        <Title text=crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.page_title)) />
        <div class="center-page">
            <div class="container" style="max-width: 720px;">

                // Header
                <div class="pe-card">
                    <h1 class="pe-section-title" style="margin-bottom: 0.5rem;">
                        <Icon icon=IconName::Lock class="icon-md" />
                        " "{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.title))}
                    </h1>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.intro))}
                    </p>
                </div>

                // Marketing Consent Section
                <div class="pe-card">
                    <h2 class="pe-section-title" style="font-size: 1.1rem;">
                        <Icon icon=IconName::Sound class="icon-sm" />
                        " "{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.marketing_title))}
                    </h2>
                    <p class="pe-detail-secondary" style="margin-bottom: 0.75rem;">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.marketing_body))}
                    </p>
                    <button
                        class="btn btn-outline btn-block"
                        disabled=move || unsub_loading.get()
                        on:click=move |_| on_unsubscribe()
                    >
                        {move || match unsub_loading.get() {
                            true => t_string!(i18n, privacy.data.processing),
                            false => t_string!(i18n, privacy.data.unsubscribe),
                        }}
                    </button>
                    {move || match unsub_state.get() {
                        UnsubState::Updated(count) => view! {
                            <div class="pe-success-box" style="margin-top: 0.5rem;">
                                {t!(i18n, privacy.data.unsubscribed, count)}
                            </div>
                        }.into_any(),
                        UnsubState::Failed(e) => view! {
                            <div class="pe-error-box" style="margin-top: 0.5rem;">
                                {e}
                            </div>
                        }.into_any(),
                        UnsubState::Idle => view! { <div></div> }.into_any(),
                    }}
                </div>

                // Data Deletion Section
                <div class="pe-card">
                    <h2 class="pe-section-title" style="font-size: 1.1rem;">
                        <Icon icon=IconName::Recycle class="icon-sm" />
                        " "{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.deletion_title))}
                    </h2>
                    <p class="pe-detail-secondary" style="margin-bottom: 0.75rem;">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.deletion_body))}
                    </p>
                    <button
                        class="btn btn-outline btn-block"
                        disabled=move || delete_loading.get()
                        on:click=move |_| on_delete_request()
                    >
                        {move || match delete_loading.get() {
                            true => t_string!(i18n, privacy.data.processing),
                            false => t_string!(i18n, privacy.data.request_deletion),
                        }}
                    </button>

                    // Delete result
                    {move || match &delete_state.get() {
                        Some(DeleteState::Result(resp)) => {
                            // `status` is a server code: matched on, never shown.
                            let status = resp.status.clone();
                            let is_completed = status == "completed";
                            let is_partial = status == "partial";

                            let blocked = resp.blocked_events.clone();
                            let affected = resp.events_affected;
                            let had_failures = !resp.failures.is_empty();

                            view! {
                                <div class=if is_completed { "pe-success-box" } else { "pe-error-box" } style="margin-top: 0.75rem;">
                                    {match status.as_str() {
                                        "completed" => view! {
                                            <div>
                                                <strong>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.completed_title))}</strong>
                                                <p style="margin-top: 0.25rem;">
                                                    {t!(i18n, privacy.data.completed_body, count = affected)}
                                                </p>
                                            </div>
                                        }.into_any(),
                                        "blocked" => view! {
                                            <div>
                                                <strong>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.blocked_title))}</strong>
                                                <p style="margin-top: 0.25rem;">
                                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.blocked_body))}
                                                </p>
                                            </div>
                                        }.into_any(),
                                        "partial" => view! {
                                            <div>
                                                <strong>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.partial_title))}</strong>
                                                <p style="margin-top: 0.25rem;">
                                                    {match had_failures {
                                                        true => t!(i18n, privacy.data.partial_failures, count = affected).into_any(),
                                                        false => t!(i18n, privacy.data.partial_active, count = affected).into_any(),
                                                    }}
                                                </p>
                                            </div>
                                        }.into_any(),
                                        "failed" => view! {
                                            <div>
                                                <strong>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.failed_title))}</strong>
                                                <p style="margin-top: 0.25rem;">
                                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.failed_body))}
                                                </p>
                                            </div>
                                        }.into_any(),
                                        _ => view! { <div></div> }.into_any(),
                                    }}
                                </div>

                                // Show blocked events with dates
                                {if !blocked.is_empty() {
                                    let blocked_clone = blocked.clone();
                                    view! {
                                        <div style="margin-top: 0.75rem;">
                                            <p class="pe-detail-secondary" style="font-weight: 600; margin-bottom: 0.5rem;">
                                                {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.blocked_events))}
                                            </p>
                                            {blocked_clone.into_iter().map(|ev: BlockedEvent| {
                                                let name = ev.event_name.clone();
                                                // Reactive, so the date follows a
                                                // language switch.
                                                let end_ms = ev.event_end_ms;
                                                let available = move || crate::utils::format_event_day(end_ms);
                                                view! {
                                                    <div class="ticket-action-card ticket-action-card--pending" style="margin-bottom: 0.5rem;">
                                                        <div class="ticket-action-icon">
                                                            <Icon icon=IconName::Clock class="icon-sm" />
                                                        </div>
                                                        <div>
                                                            <div class="ticket-action-title">{name}</div>
                                                            <div class="ticket-action-desc">
                                                                {t!(i18n, privacy.data.available_after, date = available)}
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
                                            {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.onchain_note))}
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
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.policy_prefix))}
                        <a href="/privacy" class="pe-ext-link">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.policy_link))}</a>
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.data.policy_suffix))}
                    </p>
                </div>

                // Back link
                <div style="text-align: center; margin-top: 0.5rem;">
                    <a href="/" class="btn btn-outline">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.back_home))}</a>
                </div>
            </div>
        </div>
    }
}
