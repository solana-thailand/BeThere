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
                <div class="admin-actions-row" style="margin-bottom:0.75rem">
                    <button class="btn btn-outline btn-sm" on:click=handle_refresh disabled=move || loading.get()>
                        {move || if loading.get() { "Loading..." } else { "Refresh Status" }}
                    </button>
                </div>

                // Loading state
                <Show when=move || loading.get() && cancel_status.get().is_none() fallback=|| view! { <div></div> }>
                    <div class="page-loading" style="padding:1rem">
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
                            <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:0.5rem;margin-bottom:0.75rem">
                                <div style="padding:0.5rem;border:1px solid var(--border);border-radius:6px;background:var(--bg-primary);text-align:center">
                                    <div style="font-size:1.2rem;font-weight:700;color:var(--text-primary)">{status.usdc_deposits}</div>
                                    <div style="font-size:0.65rem;color:var(--text-secondary)">"USDC Deposits"</div>
                                </div>
                                <div style="padding:0.5rem;border:1px solid var(--border);border-radius:6px;background:var(--bg-primary);text-align:center">
                                    <div style="font-size:1.2rem;font-weight:700;color:var(--text-primary)">{status.usdc_refundable}</div>
                                    <div style="font-size:0.65rem;color:var(--text-secondary)">"USDC Refundable"</div>
                                </div>
                                <div style="padding:0.5rem;border:1px solid var(--border);border-radius:6px;background:var(--bg-primary);text-align:center">
                                    <div style="font-size:1.2rem;font-weight:700;color:var(--text-primary)">{status.thb_deposits}</div>
                                    <div style="font-size:0.65rem;color:var(--text-secondary)">"THB Deposits"</div>
                                </div>
                                <div style="padding:0.5rem;border:1px solid var(--border);border-radius:6px;background:var(--bg-primary);text-align:center">
                                    <div style="font-size:1.2rem;font-weight:700;color:var(--success,green)">{status.thb_refunded}</div>
                                    <div style="font-size:0.65rem;color:var(--text-secondary)">"THB Refunded"</div>
                                </div>
                                <div style="padding:0.5rem;border:1px solid var(--border);border-radius:6px;background:var(--bg-primary);text-align:center">
                                    <div style="font-size:1.2rem;font-weight:700;color:var(--warning,orange)">{status.thb_pending_refund}</div>
                                    <div style="font-size:0.65rem;color:var(--text-secondary)">"THB Pending"</div>
                                </div>
                                <div style="padding:0.5rem;border:1px solid var(--border);border-radius:6px;background:var(--bg-primary);text-align:center">
                                    <div style="font-size:0.85rem">
                                        <span class=escrow_badge>{escrow_label}</span>
                                    </div>
                                    <div style="font-size:0.65rem;color:var(--text-secondary)">"Escrow Status"</div>
                                </div>
                            </div>
                        }.into_any()
                    }}

                    // ── Step 1: Deactivate Escrow (on-chain) ──
                    <Show when=move || can_cancel() && !is_cancelled() fallback=|| view! { <div></div> }>
                        <div style="margin-bottom:0.75rem;padding:0.75rem;border:1px solid var(--warning,orange);border-radius:8px;background:var(--bg-secondary)">
                            <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.5rem">
                                <Icon icon=IconName::AlertTriangle class="icon-sm" />
                                <span style="font-weight:600;font-size:0.85rem;color:var(--text-primary)">
                                    "Cancel Event"
                                </span>
                            </div>
                            <p style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.75rem">
                                "This will deactivate the on-chain escrow and prepare all deposits for refund. "
                                "USDC refunds require each attendee to sign a transaction — they cannot be force-refunded."
                            </p>
                            <div style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.5rem">
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
                                <div style="display:flex;gap:0.5rem;align-items:center">
                                    <a
                                        href="#/admin"
                                        class="btn btn-confirm-danger"
                                        style="font-size:0.8rem;padding:0.4rem 0.8rem;text-decoration:none"
                                        on:click={
                                            let set_cc = set_confirm_cancel;
                                            move |ev: web_sys::MouseEvent| {
                                                ev.prevent_default();
                                                set_cc.set(true);
                                                let reset = set_confirm_cancel;
                                                gloo::timers::callback::Timeout::new(5000, move || {
                                                    reset.set(false);
                                                }).forget();
                                            }
                                        }
                                    >
                                        "Cancel Event"
                                    </a>
                                    <span style="font-size:0.7rem;color:var(--text-secondary)">
                                        "Go to Escrow section → Deactivate first, then return here"
                                    </span>
                                </div>
                            </Show>
                            <Show
                                when=move || confirm_cancel.get()
                                fallback=|| view! { <div></div> }
                            >
                                <div style="display:flex;gap:0.5rem;align-items:center">
                                    <span style="font-size:0.8rem;color:var(--error,red);font-weight:600">
                                        "⚠ Are you sure?"
                                    </span>
                                    <span style="font-size:0.7rem;color:var(--text-secondary)">
                                        "Navigate to Escrow tab and click 'Deactivate' to proceed."
                                    </span>
                                    <button
                                        class="btn btn-outline btn-sm"
                                        style="font-size:0.7rem;padding:0.2rem 0.5rem"
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
                        <div style="margin-bottom:0.75rem;padding:0.75rem;border:1px solid var(--border);border-radius:8px;background:var(--bg-secondary)">
                            <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:0.5rem">
                                <div style="display:flex;align-items:center;gap:0.5rem">
                                    <span style="font-size:1rem">"💰"</span>
                                    <span style="font-weight:600;font-size:0.85rem;color:var(--text-primary)">
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
                            <p style="font-size:0.7rem;color:var(--text-secondary)">
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
                                <div style="margin-bottom:0.75rem;padding:0.75rem;border:1px solid var(--border);border-radius:8px;background:var(--bg-secondary)">
                                    <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.5rem">
                                        <Icon icon=IconName::Lock class="icon-sm" />
                                        <span style="font-weight:600;font-size:0.85rem;color:var(--text-primary)">
                                            "USDC Refund Queue"
                                        </span>
                                        <span class="badge badge-warning">{format!("{} pending", items_count)}</span>
                                    </div>
                                    <p style="font-size:0.7rem;color:var(--text-secondary);margin-bottom:0.5rem">
                                        "Each attendee must sign a refund transaction from their wallet. Share the refund link with depositors so they can claim their USDC back."
                                    </p>

                                    // Queue list
                                    <Show
                                        when=move || has_items
                                        fallback=|| view! {
                                            <div style="font-size:0.75rem;color:var(--success,green);padding:0.25rem 0">
                                                "No USDC deposits pending refund."
                                            </div>
                                        }
                                    >
                                        <div style="max-height:300px;overflow-y:auto">
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
                                                    <div style="display:flex;align-items:center;justify-content:space-between;padding:0.35rem 0.5rem;border-bottom:1px solid var(--border);font-size:0.75rem">
                                                        <div style="display:flex;flex-direction:column;gap:0.1rem">
                                                            <span style="color:var(--text-primary)">
                                                                {format!("Attendee: {}", &attendee_id[..attendee_id.len().min(12)])}
                                                            </span>
                                                            <span style="color:var(--text-secondary);font-size:0.65rem">
                                                                {format!("Wallet: {wallet_short}")}
                                                            </span>
                                                        </div>
                                                        <div style="display:flex;align-items:center;gap:0.5rem">
                                                            <span style="color:var(--text-primary);font-weight:600">
                                                                {format!("{} USDC", item.amount / 1_000_000)}
                                                            </span>
                                                            <span class="badge badge-warning" style="font-size:0.6rem">
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
                        <div style="padding:0.5rem 0.75rem;border:1px dashed var(--success,green);border-radius:6px;background:var(--bg-secondary);font-size:0.7rem;color:var(--text-secondary)">
                            <strong style="color:var(--success,green)">"Event Cancelled"</strong>
                            " — This event has been cancelled. Use the Escrow section to claim forfeited deposits and close the escrow account."
                        </div>
                    </Show>

                    // ── Info Note ──
                    <div style="margin-top:0.75rem;padding:0.5rem 0.75rem;border:1px dashed var(--border);border-radius:6px;background:var(--bg-secondary);font-size:0.7rem;color:var(--text-secondary)">
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
