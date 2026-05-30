//! Admin event cancellation workflow — embedded in the admin dashboard.
//!
//! Provides:
//! - Impact summary (USDC + THB deposit counts)
//! - One-click THB batch refund
//! - USDC refund queue listing (requires attendee signature)
//! - Step-by-step cancellation flow

use leptos::prelude::*;

use crate::api::{self, CancelStatusResponse, UsdcRefundQueueResponse};
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[component]
pub fn AdminCancel(
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    active_event_id: ReadSignal<Option<String>>,
) -> impl IntoView {
    // Cancellation status data
    let (cancel_status, set_cancel_status) = signal(None::<CancelStatusResponse>);
    let (usdc_queue, set_usdc_queue) = signal(None::<UsdcRefundQueueResponse>);

    // UI state
    let (loading, set_loading) = signal(false);
    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let (batching_thb, set_batching_thb) = signal(false);
    let (confirm_cancel, set_confirm_cancel) = signal(false);

    // Load cancel status when event changes or on refresh
    let tracked_event_id = active_event_id;
    Effect::new(move |_| {
        let _ = refresh_counter.get();
        let eid = tracked_event_id.get();
        if eid.is_none() {
            set_cancel_status.set(None);
            set_usdc_queue.set(None);
            return;
        }
        let eid_val = eid.unwrap();
        set_loading.set(true);

        leptos::task::spawn_local(async move {
            // Load cancel status
            match api::get_cancel_status(&eid_val).await {
                Ok(status) => set_cancel_status.set(Some(status)),
                Err(e) => {
                    log::warn!("[admin-cancel] failed to load cancel status: {e}");
                    set_cancel_status.set(None);
                }
            }

            // Load USDC refund queue
            match api::get_usdc_refund_queue(&eid_val).await {
                Ok(queue) => set_usdc_queue.set(Some(queue)),
                Err(e) => {
                    log::warn!("[admin-cancel] failed to load USDC queue: {e}");
                    set_usdc_queue.set(None);
                }
            }

            set_loading.set(false);
        });
    });

    // Handle refresh
    let handle_refresh = move |_: web_sys::MouseEvent| {
        set_refresh_counter.update(|c| *c += 1);
    };

    // Handle THB batch refund
    let handle_batch_thb = move |_: web_sys::MouseEvent| {
        if batching_thb.get() {
            return;
        }
        let eid = active_event_id.get().unwrap_or_default();
        if eid.is_empty() {
            return;
        }

        set_batching_thb.set(true);
        let set_t = set_toast;
        let set_refresh = set_refresh_counter;
        let set_busy = set_batching_thb;

        leptos::task::spawn_local(async move {
            match api::batch_thb_refund(&eid).await {
                Ok(result) => {
                    components::show_toast(
                        &set_t,
                        &format!(
                            "THB batch refund: {} refunded, {} skipped",
                            result.refunded, result.skipped
                        ),
                        ToastType::Success,
                    );
                    set_refresh.update(|c| *c += 1);
                }
                Err(e) => {
                    components::show_toast(
                        &set_t,
                        &format!("Batch THB refund failed: {}", e.message),
                        ToastType::Error,
                    );
                }
            }
            set_busy.set(false);
        });
    };

    let has_event = move || active_event_id.get().is_some();
    let escrow_status_str = move || {
        cancel_status
            .get()
            .as_ref()
            .map(|s| s.escrow_status.clone())
            .unwrap_or_default()
    };

    // Can cancel if escrow is initialized or deactivated
    let can_cancel = move || {
        let status = escrow_status_str();
        status == "initialized" || status == "deactivated"
    };

    // Is cancelled already
    let is_cancelled = move || escrow_status_str() == "cancelled";

    view! {
        <div class="admin-escrow">
            <div class="admin-section-header">
                <h3>"Event Cancellation"</h3>
                <p class="admin-section-subtitle">
                    "Cancel an event and manage refunds for all depositors. THB deposits can be batch-refunded by admin. USDC deposits require attendee-signed transactions."
                </p>
            </div>

            // No event selected
            <Show when=move || !has_event() fallback=|| view! { <div></div> }>
                <div class="admin-empty-state">
                    "Select an event with deposits enabled to manage cancellation."
                </div>
            </Show>

            // Event selected
            <Show when=move || has_event() fallback=|| view! { <div></div> }>

                // Refresh button
                <div class="admin-actions-row admin-cancel-actions-row">
                    <button class="btn btn-outline btn-sm" on:click=handle_refresh disabled=move || loading.get()>
                        {move || if loading.get() { "Loading..." } else { "Refresh Status" }}
                    </button>
                </div>

                // Loading state
                <Show when=move || loading.get() && cancel_status.get().is_none() fallback=|| view! { <div></div> }>
                    <div class="page-loading admin-cancel-loading">
                        <span class="spinner spinner-sm"></span>
                        "Loading cancellation status..."
                    </div>
                </Show>

                // Status loaded
                <Show when=move || cancel_status.get().is_some() fallback=|| view! { <div></div> }>
                    {move || {
                        let status = cancel_status.get().unwrap_or_default();
                        let escrow_label = match status.escrow_status.as_str() {
                            "none" => "No Escrow",
                            "initialized" => "Active",
                            "deactivated" => "Deactivated",
                            "closed" => "Closed",
                            "cancelled" => "Cancelled",
                            _ => &status.escrow_status,
                        };
                        let escrow_badge = match status.escrow_status.as_str() {
                            "none" => "badge badge-neutral",
                            "initialized" => "badge badge-success",
                            "deactivated" => "badge badge-warning",
                            "closed" => "badge badge-neutral",
                            "cancelled" => "badge badge-error",
                            _ => "badge badge-neutral",
                        };

                        view! {
                            // ── Status Overview ──
                            <div class="admin-cancel-stats-grid">
                                <div class="admin-cancel-stat-card">
                                    <div class="admin-cancel-stat-value">{status.usdc_deposits}</div>
                                    <div class="admin-cancel-stat-label">"USDC Deposits"</div>
                                </div>
                                <div class="admin-cancel-stat-card">
                                    <div class="admin-cancel-stat-value">{status.usdc_refundable}</div>
                                    <div class="admin-cancel-stat-label">"USDC Refundable"</div>
                                </div>
                                <div class="admin-cancel-stat-card">
                                    <div class="admin-cancel-stat-value">{status.thb_deposits}</div>
                                    <div class="admin-cancel-stat-label">"THB Deposits"</div>
                                </div>
                                <div class="admin-cancel-stat-card">
                                    <div class="admin-cancel-stat-value-success">{status.thb_refunded}</div>
                                    <div class="admin-cancel-stat-label">"THB Refunded"</div>
                                </div>
                                <div class="admin-cancel-stat-card">
                                    <div class="admin-cancel-stat-value-warning">{status.thb_pending_refund}</div>
                                    <div class="admin-cancel-stat-label">"THB Pending"</div>
                                </div>
                                <div class="admin-cancel-stat-card">
                                    <div class="admin-cancel-stat-escrow">
                                        <span class=escrow_badge>{escrow_label}</span>
                                    </div>
                                    <div class="admin-cancel-stat-label">"Escrow Status"</div>
                                </div>
                            </div>
                        }.into_any()
                    }}

                    // ── Step 1: Deactivate Escrow (on-chain) ──
                    <Show when=move || can_cancel() && !is_cancelled() fallback=|| view! { <div></div> }>
                        <div class="admin-cancel-warning-card">
                            <div class="admin-cancel-section-row">
                                <Icon icon=IconName::AlertTriangle class="icon-sm" />
                                <span class="admin-cancel-section-title">
                                    "Cancel Event"
                                </span>
                            </div>
                            <p class="admin-cancel-desc">
                                "This will deactivate the on-chain escrow and prepare all deposits for refund. "
                                "USDC refunds require each attendee to sign a transaction — they cannot be force-refunded."
                            </p>
                            <div class="admin-cancel-info-text">
                                {move || {
                                    let status = cancel_status.get().unwrap_or_default();
                                    format!(
                                        "Impact: {} USDC deposits (refundable), {} THB deposits to batch-refund",
                                        status.usdc_refundable,
                                        status.thb_pending_refund,
                                    )
                                }}
                            </div>

                            // Confirm + Cancel button
                            <Show
                                when=move || !confirm_cancel.get()
                                fallback=|| view! { <div></div> }
                            >
                                <div class="admin-cancel-btn-row">
                                    <a
                                        href="#/admin"
                                        class="btn btn-confirm-danger admin-cancel-btn-cancel"
                                        on:click={
                                            let set_cc = set_confirm_cancel;
                                            move |ev: web_sys::MouseEvent| {
                                                ev.prevent_default();
                                                set_cc.set(true);
                                                let reset = set_confirm_cancel;
                                                gloo_timers::callback::Timeout::new(5000, move || {
                                                    reset.set(false);
                                                }).forget();
                                            }
                                        }
                                    >
                                        "Cancel Event"
                                    </a>
                                    <span class="admin-cancel-hint">
                                        "Go to Escrow section → Deactivate first, then return here"
                                    </span>
                                </div>
                            </Show>
                            <Show
                                when=move || confirm_cancel.get()
                                fallback=|| view! { <div></div> }
                            >
                                <div class="admin-cancel-btn-row">
                                    <span class="admin-cancel-danger-text">
                                        "⚠ Are you sure?"
                                    </span>
                                    <span class="admin-cancel-hint">
                                        "Navigate to Escrow tab and click 'Deactivate' to proceed."
                                    </span>
                                    <button
                                        class="btn btn-outline btn-sm admin-cancel-btn-dismiss"
                                        on:click=move |_| set_confirm_cancel.set(false)
                                    >
                                        "Dismiss"
                                    </button>
                                </div>
                            </Show>
                        </div>
                    </Show>

                    // ── Step 2: Batch THB Refund ──
                    <Show when=move || is_cancelled() || can_cancel() fallback=|| view! { <div></div> }>
                        <div class="admin-cancel-panel">
                            <div class="admin-cancel-panel-header">
                                <div class="admin-cancel-icon-group">
                                    <span class="admin-cancel-emoji">"💰"</span>
                                    <span class="admin-cancel-section-title">
                                        "THB Batch Refund"
                                    </span>
                                </div>
                                <button
                                    class="btn btn-outline btn-sm"
                                    disabled=move || batching_thb.get()
                                    on:click=handle_batch_thb
                                >
                                    {move || {
                                        if batching_thb.get() {
                                            "Refunding...".to_string()
                                        } else {
                                            "Batch Refund All THB".to_string()
                                        }
                                    }}
                                </button>
                            </div>
                            <p class="admin-cancel-hint">
                                "Marks all verified THB deposits as refunded. No on-chain transaction required — this is a pure database operation."
                            </p>
                        </div>
                    </Show>

                    // ── Step 3: USDC Refund Queue ──
                    <Show when=move || usdc_queue.get().is_some() fallback=|| view! { <div></div> }>
                        {move || {
                            let queue = usdc_queue.get().unwrap_or_default();
                            let items = queue.queue.clone();
                            let items_count = items.len();
                            let has_items = !items.is_empty();

                            view! {
                                <div class="admin-cancel-panel">
                                    <div class="admin-cancel-section-row">
                                        <Icon icon=IconName::Lock class="icon-sm" />
                                        <span class="admin-cancel-section-title">
                                            "USDC Refund Queue"
                                        </span>
                                        <span class="badge badge-warning">{format!("{} pending", items_count)}</span>
                                    </div>
                                    <p class="admin-cancel-info-text">
                                        "Each attendee must sign a refund transaction from their wallet. Share the refund link with depositors so they can claim their USDC back."
                                    </p>

                                    // Queue list
                                    <Show
                                        when=move || has_items
                                        fallback=|| view! {
                                            <div class="admin-cancel-success-text">
                                                "No USDC deposits pending refund."
                                            </div>
                                        }
                                    >
                                        <div class="admin-cancel-scroll-list">
                                            {items.iter().map(|item| {
                                                let wallet_short = item.wallet_address.as_ref()
                                                    .map(|w| {
                                                        if w.len() > 12 {
                                                            format!("{}...{}", &w[..4], &w[w.len()-4..])
                                                        } else {
                                                            w.clone()
                                                        }
                                                    })
                                                    .unwrap_or_default();
                                                let attendee_id = item.attendee_id.clone();
                                                view! {
                                                    <div class="admin-cancel-queue-item">
                                                        <div class="admin-cancel-queue-left">
                                                            <span class="admin-cancel-queue-name">
                                                                {format!("Attendee: {}", &attendee_id[..attendee_id.len().min(12)])}
                                                            </span>
                                                            <span class="admin-cancel-queue-wallet">
                                                                {format!("Wallet: {wallet_short}")}
                                                            </span>
                                                        </div>
                                                        <div class="admin-cancel-queue-right">
                                                            <span class="admin-cancel-queue-amount">
                                                                {format!("{} USDC", item.amount / 1_000_000)}
                                                            </span>
                                                            <span class="badge badge-warning admin-cancel-badge-xs">
                                                                "Needs Signature"
                                                            </span>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                </div>
                            }.into_any()
                        }}
                    </Show>

                    // ── Cancelled Status ──
                    <Show when=move || is_cancelled() fallback=|| view! { <div></div> }>
                        <div class="admin-cancel-cancelled-banner">
                            <strong>"Event Cancelled"</strong>
                            " — This event has been cancelled. Use the Escrow section to claim forfeited deposits and close the escrow account."
                        </div>
                    </Show>

                    // ── Info Note ──
                    <div class="admin-cancel-info-note">
                        <strong>"Cancellation Flow:"</strong>
                        " 1) Go to Escrow → Deactivate event. "
                        " 2) Return here → Batch refund THB deposits. "
                        " 3) Share refund links with USDC depositors so they can self-refund. "
                        " 4) Go to Escrow → Claim forfeited → Close event."
                    </div>
                </Show>
            </Show>
        </div>
    }
}
