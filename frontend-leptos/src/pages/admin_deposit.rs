//! Admin deposit and refund management — embedded in the admin dashboard.
//!
//! Two sub-tabs:
//! - **Deposits**: Shows THB payment slips pending admin verification.
//! - **Refund Queue**: Shows verified deposits awaiting refund processing.

use leptos::prelude::*;

use crate::api::{self, MarkRefundRequest, ThbDepositInfo, VerifySlipRequest};
use crate::components::{self, ToastType};
use crate::utils;

// ---------------------------------------------------------------------------
// Sub-tab enum
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum AdminDepositTab {
    Deposits,
    RefundQueue,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[component]
pub fn AdminDeposits(
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    active_event_id: ReadSignal<Option<String>>,
) -> impl IntoView {
    // Sub-tab state
    let (active_tab, set_active_tab) = signal(AdminDepositTab::Deposits);

    // Data state
    let (slips, set_slips) = signal(Vec::<ThbDepositInfo>::new());
    let (refunds, set_refunds) = signal(Vec::<ThbDepositInfo>::new());

    // UI state
    let (loading, set_loading) = signal(true);
    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let (action_pending, set_action_pending) = signal(None::<String>);

    // Load data on mount and when active_event_id / refresh_counter changes
    let tracked_event_id = active_event_id;
    Effect::new(move |_| {
        let _ = refresh_counter.get();
        let eid = tracked_event_id.get();

        // Skip when event hasn't been selected yet
        if eid.is_none() {
            set_loading.set(false);
            return;
        }

        set_loading.set(true);

        let set_slips = set_slips;
        let set_refunds = set_refunds;
        let set_loading = set_loading;
        let set_toast = set_toast;

        leptos::task::spawn_local(async move {
            let slips_result = api::get_pending_slips(eid.as_deref()).await;
            let refunds_result = api::get_refund_queue(eid.as_deref()).await;

            match slips_result {
                Ok(data) => set_slips.set(data.slips),
                Err(e) => {
                    log::warn!("[admin-deposit] failed to load pending slips: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to load pending slips: {e}"),
                        ToastType::Error,
                    );
                }
            }

            match refunds_result {
                Ok(data) => set_refunds.set(data.pending),
                Err(e) => {
                    log::warn!("[admin-deposit] failed to load refund queue: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to load refund queue: {e}"),
                        ToastType::Error,
                    );
                }
            }

            set_loading.set(false);
        });
    });

    // Helper to refresh data after an action
    let refresh_data = move || {
        set_refresh_counter.update(|c| *c += 1);
    };

    // Approve a slip
    let handle_approve = move |slip: ThbDepositInfo| {
        let attendee_id = slip.attendee_id.clone();
        let event_id = slip.event_id.clone();
        let key = format!("approve-{attendee_id}");
        set_action_pending.set(Some(key));

        leptos::task::spawn_local(async move {
            let body = VerifySlipRequest {
                event_id,
                attendee_id: attendee_id.clone(),
                approved: true,
            };
            match api::verify_thb_slip(&body).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        "Slip approved successfully",
                        ToastType::Success,
                    );
                    refresh_data();
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to approve slip: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_action_pending.set(None);
        });
    };

    // Reject a slip
    let handle_reject = move |slip: ThbDepositInfo| {
        let attendee_id = slip.attendee_id.clone();
        let event_id = slip.event_id.clone();
        let key = format!("reject-{attendee_id}");
        set_action_pending.set(Some(key));

        leptos::task::spawn_local(async move {
            let body = VerifySlipRequest {
                event_id,
                attendee_id: attendee_id.clone(),
                approved: false,
            };
            match api::verify_thb_slip(&body).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        "Slip rejected",
                        ToastType::Warning,
                    );
                    refresh_data();
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to reject slip: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_action_pending.set(None);
        });
    };

    // Mark as refunded
    let handle_mark_refunded = move |item: ThbDepositInfo| {
        let attendee_id = item.attendee_id.clone();
        let event_id = item.event_id.clone();
        let key = format!("refund-{attendee_id}");
        set_action_pending.set(Some(key));

        leptos::task::spawn_local(async move {
            let body = MarkRefundRequest { event_id };
            match api::mark_refund(&attendee_id, &body).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        "Refund marked as processed",
                        ToastType::Success,
                    );
                    refresh_data();
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to mark refund: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_action_pending.set(None);
        });
    };

    // Computed: pending count for display
    let pending_count = Memo::new(move |_| slips.get().len());
    let refund_count = Memo::new(move |_| refunds.get().len());

    view! {
        <div class="admin-deposits">
            // Sub-tab navigation
            <div class="tabs">
                <button
                    class="tab"
                    class:active=move || active_tab.get() == AdminDepositTab::Deposits
                    on:click=move |_| set_active_tab.set(AdminDepositTab::Deposits)
                >
                    "Deposits"
                    <Show when=move || pending_count.get() != 0 fallback=|| view! { <span></span> }>
                        <span class="badge badge-warning">
                            {move || pending_count.get()}
                        </span>
                    </Show>
                </button>
                <button
                    class="tab"
                    class:active=move || active_tab.get() == AdminDepositTab::RefundQueue
                    on:click=move |_| set_active_tab.set(AdminDepositTab::RefundQueue)
                >
                    "💸 Refund Queue"
                    <Show when=move || refund_count.get() != 0 fallback=|| view! { <span></span> }>
                        <span class="badge badge-warning">
                            {move || refund_count.get()}
                        </span>
                    </Show>
                </button>
            </div>

            // Loading state
            <Show when=move || loading.get() fallback=|| view! { <div></div> }>
                <div class="page-loading">
                    <span class="spinner"></span>
                    "Loading deposits..."
                </div>
            </Show>

            // Content (shown when not loading)
            <Show when=move || !loading.get() fallback=|| view! { <div></div> }>

                // ── Deposits Tab ──
                <Show
                    when=move || active_tab.get() == AdminDepositTab::Deposits
                    fallback=|| view! { <div></div> }
                >
                    <div class="admin-section-header">
                        <h3>{format!("{} pending slip{}", pending_count.get(), if pending_count.get() != 1 { "s" } else { "" })}</h3>
                    </div>

                    <Show
                        when=move || pending_count.get() == 0
                        fallback=|| view! { <div></div> }
                    >
                        <div class="admin-empty-state">
                            "No pending deposits to verify"
                        </div>
                    </Show>

                    {move || {
                        let current_action = action_pending.get();
                        slips.get().iter().map(|slip| {
                            let slip_id = slip.attendee_id.clone();
                            let approve_key = format!("approve-{slip_id}");
                            let reject_key = format!("reject-{slip_id}");
                            let approve_disabled = current_action.as_ref().map_or(false, |k| k == &approve_key || k == &reject_key);
                            let reject_disabled = approve_disabled;
                            let approve_loading = current_action.as_ref().map_or(false, |k| k == &approve_key);
                            let reject_loading = current_action.as_ref().map_or(false, |k| k == &reject_key);

                            let amount = format!("{} THB", slip.amount_thb);
                            let uploaded_ago = utils::time_ago(&slip.uploaded_at);
                            let uploaded_formatted = utils::format_timestamp(&slip.uploaded_at);
                            let slip_url = slip.slip_url.clone();
                            let has_slip_url = slip_url.is_some();

                            let slip_for_approve = slip.clone();
                            let slip_for_reject = slip.clone();

                            view! {
                                <div class="card">
                                    <div class="flex-row-wrap">
                                        <div>
                                            <div class="admin-attendee-name">
                                                {format!("Attendee: {}", utils::escape_html(&slip.attendee_id))}
                                            </div>
                                            <div class="admin-amount-line">
                                                {amount}
                                            </div>
                                            <div class="panel-hint">
                                                {"Uploaded: "}
                                                <span title={uploaded_formatted.clone()}>{uploaded_ago.clone()}</span>
                                            </div>
                                            <Show when=move || has_slip_url fallback=|| view! { <span></span> }>
                                                <div style="margin-top:0.25rem">
                                                    <a
                                                        href=slip_url.clone().unwrap_or_default()
                                                        target="_blank"
                                                        rel="noopener noreferrer"
                                                        class="link-accent"
                                                    >
                                                        "View Slip"
                                                    </a>
                                                </div>
                                            </Show>
                                        </div>
                                        <div class="flex-row-gap">
                                            <button
                                                class="btn btn-success btn-sm"
                                                disabled=approve_disabled
                                                on:click=move |_| handle_approve(slip_for_approve.clone())
                                            >
                                                {if approve_loading { "Approving..." } else { "✓ Approve" }}
                                            </button>
                                            <button
                                                class="btn btn-danger btn-sm"
                                                disabled=reject_disabled
                                                on:click=move |_| handle_reject(slip_for_reject.clone())
                                            >
                                                {if reject_loading { "Rejecting..." } else { "✗ Reject" }}
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    }}
                </Show>

                // ── Refund Queue Tab ──
                <Show
                    when=move || active_tab.get() == AdminDepositTab::RefundQueue
                    fallback=|| view! { <div></div> }
                >
                    <div class="admin-section-header">
                        <h3>{format!("💸 {} pending refund{}", refund_count.get(), if refund_count.get() != 1 { "s" } else { "" })}</h3>
                    </div>

                    <Show
                        when=move || refund_count.get() == 0
                        fallback=|| view! { <div></div> }
                    >
                        <div class="admin-empty-state">
                            "No pending refunds to process"
                        </div>
                    </Show>

                    {move || {
                        let current_action = action_pending.get();
                        refunds.get().iter().map(|item| {
                            let item_id = item.attendee_id.clone();
                            let refund_key = format!("refund-{item_id}");
                            let refund_disabled = current_action.as_ref().map_or(false, |k| k == &refund_key);
                            let refund_loading = current_action.as_ref().map_or(false, |k| k == &refund_key);

                            let amount = format!("{} THB", item.amount_thb);
                            let verified_by = item.verified_by.as_deref().unwrap_or("Unknown");
                            let verified_at = item.verified_at.as_deref().map(utils::format_timestamp).unwrap_or_else(|| "N/A".to_string());

                            let item_for_refund = item.clone();

                            view! {
                                <div class="card">
                                    <div class="flex-row-wrap">
                                        <div>
                                            <div class="admin-attendee-name">
                                                {format!("Attendee: {}", utils::escape_html(&item.attendee_id))}
                                            </div>
                                            <div class="admin-amount-line">
                                                {amount}
                                            </div>
                                            <div class="panel-hint">
                                                {format!("Verified by: {}", utils::escape_html(verified_by))}
                                            </div>
                                            <div class="panel-hint">
                                                {format!("Verified at: {verified_at}")}
                                            </div>
                                        </div>
                                        <div>
                                            <button
                                                class="btn btn-primary btn-sm"
                                                disabled=refund_disabled
                                                on:click=move |_| handle_mark_refunded(item_for_refund.clone())
                                            >
                                                {if refund_loading { "Processing..." } else { "Mark Refunded" }}
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    }}
                </Show>

            </Show>
        </div>
    }
}
