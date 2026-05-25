//! Admin deposit and refund management — embedded in the admin dashboard.
//!
//! Two sub-tabs:
//! - **Deposits**: Shows THB payment slips pending admin verification.
//! - **Refund Queue**: Shows verified deposits awaiting refund processing.

use leptos::prelude::*;

use crate::api::{self, MarkRefundRequest, ThbDepositInfo, VerifySlipRequest};
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};
use crate::utils;

// ---------------------------------------------------------------------------
// Sub-tab enum
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum AdminDepositTab {
    Deposits,
    RefundQueue,
    Refunded,
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
    let (refunded_list, set_refunded_list) = signal(Vec::<ThbDepositInfo>::new());

    // UI state
    let (loading, set_loading) = signal(true);
    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let (action_pending, set_action_pending) = signal(None::<String>);
    let (confirm_reject_id, set_confirm_reject_id) = signal(None::<String>);
    // Refund proof: 2-step flow — first click shows input, second click confirms
    let (refund_proof_pending_id, set_refund_proof_pending_id) = signal(None::<String>);
    let (refund_proof_url_input, set_refund_proof_url_input) = signal(String::new());

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
            let refunded_result = api::get_refunded_list(eid.as_deref()).await;

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

            match refunded_result {
                Ok(data) => set_refunded_list.set(data.refunded),
                Err(e) => {
                    log::warn!("[admin-deposit] failed to load refunded list: {e}");
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

    // Reject a slip (two-step confirmation)
    let handle_reject = move |slip: ThbDepositInfo| {
        let attendee_id = slip.attendee_id.clone();

        // First click: enter confirm state
        if confirm_reject_id.get().as_deref() != Some(&attendee_id) {
            set_confirm_reject_id.set(Some(attendee_id.clone()));
            let set_confirm = set_confirm_reject_id;
            gloo::timers::callback::Timeout::new(3000, move || {
                set_confirm.set(None);
            }).forget();
            return;
        }

        // Second click: execute the actual reject
        let event_id = slip.event_id.clone();
        let key = format!("reject-{attendee_id}");
        set_confirm_reject_id.set(None);
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

    // Mark as refunded — takes the refund proof URL from the input signal
    let handle_mark_refunded = move |item: ThbDepositInfo| {
        let attendee_id = item.attendee_id.clone();
        let event_id = item.event_id.clone();
        let refund_proof_url = refund_proof_url_input.get();

        if refund_proof_url.trim().is_empty() {
            components::show_toast(
                &set_toast,
                "Refund proof URL is required — upload a bank transfer receipt",
                ToastType::Error,
            );
            return;
        }

        let key = format!("refund-{attendee_id}");
        set_action_pending.set(Some(key));
        set_refund_proof_pending_id.set(None);

        leptos::task::spawn_local(async move {
            let body = MarkRefundRequest { event_id, refund_proof_url };
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
    let refunded_count = Memo::new(move |_| refunded_list.get().len());

    let has_event = move || active_event_id.get().is_some();

    view! {
        <div class="admin-deposits">
            // No event selected
            <Show when=move || !has_event() fallback=|| view! { <div></div> }>
                <div class="admin-empty-state">
                    "Select an event to manage deposits and refunds."
                </div>
            </Show>

            // Event selected — show full content
            <Show when=move || has_event() fallback=|| view! { <div></div> }>
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
                    <Icon icon=IconName::MoneyWings class="icon-sm"/>" Refund Queue"
                    <Show when=move || refund_count.get() != 0 fallback=|| view! { <span></span> }>
                        <span class="badge badge-warning">
                            {move || refund_count.get()}
                        </span>
                    </Show>
                </button>
                <button
                    class="tab"
                    class:active=move || active_tab.get() == AdminDepositTab::Refunded
                    on:click=move |_| set_active_tab.set(AdminDepositTab::Refunded)
                >
                    <Icon icon=IconName::Check class="icon-sm"/>" Refunded"
                    <Show when=move || refunded_count.get() != 0 fallback=|| view! { <span></span> }>
                        <span class="badge badge-success">
                            {move || refunded_count.get()}
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
                            let display_name = slip.attendee_name.as_deref().unwrap_or(&slip.attendee_id);

                            let slip_for_approve = slip.clone();
                            let slip_for_reject = slip.clone();
                            let is_confirming = confirm_reject_id.get().as_deref() == Some(&slip_id);

                            view! {
                                <div class="card">
                                    <div class="flex-row-wrap">
                                        <div>
                                            <div class="admin-attendee-name">
                                                {format!("Attendee: {}", utils::escape_html(display_name))}
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
                                                class=if is_confirming { "btn btn-confirm-danger btn-sm" } else { "btn btn-danger btn-sm" }
                                                disabled=reject_disabled
                                                on:click=move |_| handle_reject(slip_for_reject.clone())
                                            >
                                                {if reject_loading { "Rejecting..." } else if is_confirming { "⚠ Confirm Reject?" } else { "✗ Reject" }}
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
                        <h3><Icon icon=IconName::MoneyWings class="icon-sm"/>{format!(" {} pending refund{}", refund_count.get(), if refund_count.get() != 1 { "s" } else { "" })}</h3>
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
                            let display_name = item.attendee_name.as_deref().unwrap_or(&item.attendee_id);
                            let slip_url = item.slip_url.clone();
                            let has_slip_url = slip_url.is_some();
                            let has_bank_info = item.bank_account.is_some() && item.bank_name.is_some() && item.account_name.is_some();
                            let display_bank_account = item.bank_account.clone();
                            let display_bank_name = item.bank_name.clone();
                            let display_account_name = item.account_name.clone();

                            let item_for_refund = item.clone();
                            let item_id_for_click = item_id.clone();
                            let item_id_for_style = item_id.clone();

                            view! {
                                <div class="card">
                                    <div class="flex-row-wrap">
                                        <div>
                                            <div class="admin-attendee-name">
                                                {format!("Attendee: {}", utils::escape_html(display_name))}
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

                                            // Slip image link
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

                                            // Bank info section
                                            <div style="margin-top:0.5rem;border-top:1px solid rgba(255,255,255,0.1);padding-top:0.5rem;">
                                                <div class="panel-hint" style="font-weight:600;margin-bottom:0.25rem;">"Refund Bank Info"</div>
                                                <Show when=move || has_bank_info fallback=|| view! { <span></span> }>
                                                    <div class="panel-hint">
                                                        {format!("Account: {}", utils::escape_html(display_bank_account.as_deref().unwrap_or("-")))}
                                                    </div>
                                                    <div class="panel-hint">
                                                        {format!("Bank: {}", utils::escape_html(display_bank_name.as_deref().unwrap_or("-")))}
                                                    </div>
                                                    <div class="panel-hint">
                                                        {format!("Name: {}", utils::escape_html(display_account_name.as_deref().unwrap_or("-")))}
                                                    </div>
                                                </Show>
                                                <Show when=move || !has_bank_info fallback=|| view! { <span></span> }>
                                                    <div class="badge badge-warning" style="margin-top:0.25rem;">
                                                        "⚠ No bank info — ask attendee"
                                                    </div>
                                                </Show>
                                            </div>
                                        </div>
                                        <div>
                                            <button
                                                class="btn btn-primary btn-sm"
                                                disabled=refund_disabled
                                                style=move || if refund_proof_pending_id.get().as_deref() == Some(&item_id_for_style) { "display:none" } else { "" }
                                                on:click=move |_| {
                                                    set_refund_proof_pending_id.set(Some(item_id_for_click.clone()));
                                                    set_refund_proof_url_input.set(String::new());
                                                }
                                            >
                                                {if refund_loading { "Processing..." } else { "Mark Refunded" }}
                                            </button>
                                            <div
                                                style=move || if refund_proof_pending_id.get().as_deref() != Some(&item_id) { "display:none" } else { "display:flex;flex-direction:column;gap:0.25rem" }
                                            >
                                                <input
                                                    type="text"
                                                    class="form-input dep-input"
                                                    placeholder="Paste refund proof URL (transfer receipt)"
                                                    prop:value=move || refund_proof_url_input.get()
                                                    on:input=move |ev| {
                                                        let val = event_target_value(&ev);
                                                        set_refund_proof_url_input.set(val);
                                                    }
                                                />
                                                <div style="display:flex;gap:0.25rem;">
                                                    <button
                                                        class="btn btn-success btn-sm"
                                                        disabled=refund_disabled
                                                        on:click=move |_| handle_mark_refunded(item_for_refund.clone())
                                                    >
                                                        {if refund_loading { "Processing..." } else { "✓ Confirm Refund" }}
                                                    </button>
                                                    <button
                                                        class="btn btn-danger btn-sm"
                                                        on:click=move |_| set_refund_proof_pending_id.set(None)
                                                    >
                                                        "Cancel"
                                                    </button>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    }}
                </Show>

                // ── Refunded Tab ──
                <Show
                    when=move || active_tab.get() == AdminDepositTab::Refunded
                    fallback=|| view! { <div></div> }
                >
                    <div class="admin-section-header">
                        <h3><Icon icon=IconName::Check class="icon-sm icon-success"/>{format!(" {} refund{} processed", refunded_count.get(), if refunded_count.get() != 1 { "s" } else { "" })}</h3>
                    </div>

                    <Show
                        when=move || refunded_count.get() == 0
                        fallback=|| view! { <div></div> }
                    >
                        <div class="admin-empty-state">
                            "No refunds processed yet"
                        </div>
                    </Show>

                    {move || {
                        let items: Vec<_> = refunded_list.get().iter().map(|item| {
                            let amount = format!("{} THB", item.amount_thb);
                            let verified_by = item.verified_by.as_deref().unwrap_or("Unknown").to_string();
                            let refunded_at = item.refunded_at.as_deref().map(utils::format_timestamp).unwrap_or_else(|| "N/A".to_string());
                            let display_name = item.attendee_name.as_deref().unwrap_or(&item.attendee_id).to_string();
                            let slip_url = item.slip_url.clone();
                            let has_slip_url = slip_url.is_some();
                            let has_bank_info = item.bank_account.is_some() && item.bank_name.is_some() && item.account_name.is_some();
                            let display_bank_account = item.bank_account.clone();
                            let display_bank_name = item.bank_name.clone();
                            let display_account_name = item.account_name.clone();
                            let refund_proof_url = item.refund_proof_url.clone();
                            let has_refund_proof = refund_proof_url.is_some();

                            (amount, verified_by, refunded_at, display_name, slip_url, has_slip_url, has_bank_info, display_bank_account, display_bank_name, display_account_name, refund_proof_url, has_refund_proof)
                        }).collect();

                        items.into_iter().map(|(amount, verified_by, refunded_at, display_name, slip_url, has_slip_url, has_bank_info, display_bank_account, display_bank_name, display_account_name, refund_proof_url, has_refund_proof)| {
                            view! {
                                <div class="card">
                                    <div class="flex-row-wrap">
                                        <div>
                                            <div class="admin-attendee-name">
                                                {format!("Attendee: {}", utils::escape_html(&display_name))}
                                            </div>
                                            <div class="admin-amount-line">
                                                {amount}
                                            </div>
                                            <div class="panel-hint">
                                                {format!("Verified by: {}", utils::escape_html(&verified_by))}
                                            </div>
                                            <div class="panel-hint">
                                                {format!("Refunded at: {refunded_at}")}
                                            </div>

                                            // Slip image link
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

                                            // Bank info section
                                            <div style="margin-top:0.5rem;border-top:1px solid rgba(255,255,255,0.1);padding-top:0.5rem;">
                                                <div class="panel-hint" style="font-weight:600;margin-bottom:0.25rem;">"Refund Bank Info"</div>
                                                <Show when=move || has_bank_info fallback=|| view! { <span></span> }>
                                                    <div class="panel-hint">
                                                        {format!("Account: {}", utils::escape_html(display_bank_account.as_deref().unwrap_or("-")))}
                                                    </div>
                                                    <div class="panel-hint">
                                                        {format!("Bank: {}", utils::escape_html(display_bank_name.as_deref().unwrap_or("-")))}
                                                    </div>
                                                    <div class="panel-hint">
                                                        {format!("Name: {}", utils::escape_html(display_account_name.as_deref().unwrap_or("-")))}
                                                    </div>
                                                </Show>
                                                <Show when=move || !has_bank_info fallback=|| view! { <span></span> }>
                                                    <div class="badge badge-warning" style="margin-top:0.25rem;">
                                                        "⚠ No bank info was provided"
                                                    </div>
                                                </Show>
                                            </div>

                                            // Refund proof link
                                            <Show when=move || has_refund_proof fallback=|| view! { <span></span> }>
                                                <div style="margin-top:0.25rem">
                                                    <a
                                                        href=refund_proof_url.clone().unwrap_or_default()
                                                        target="_blank"
                                                        rel="noopener noreferrer"
                                                        class="link-accent"
                                                    >
                                                        "View Refund Proof"
                                                    </a>
                                                </div>
                                            </Show>
                                        </div>
                                        <div>
                                            <span class="badge badge-success">"✓ Refunded"</span>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    }}
                </Show>

            </Show>
            </Show>
        </div>
    }
}
