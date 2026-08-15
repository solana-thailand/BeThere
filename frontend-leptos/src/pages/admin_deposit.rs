//! Admin deposit and refund management — embedded in the admin dashboard.
//!
//! Three sub-tabs:
//! - **Deposits**: Shows THB payment slips pending admin verification.
//! - **Refund Queue**: Shows verified deposits awaiting refund processing.
//!   Includes a "Hold as Credit" action for attendees who confirmed hold verbally.
//! - **Held as Credit**: Shows deposits held as rolling credit for the next event.
//!   Includes a "Credit Refund Requested" badge + sub-list when any contact has
//!   requested return of their held credit (Issue #061 Phase 3 — exit path).

use std::collections::HashMap;

use leptos::prelude::*;

use crate::api::{
    self, AdminHoldRequest, ClearCreditRefundRequest, CreditLiability, CreditRefundRequest,
    MarkRefundRequest, ThbDepositInfo, VerifySlipRequest,
};
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};
use crate::pages::admin_deposit_record_slip::AdminRecordSlipModal;
use crate::utils;

// ---------------------------------------------------------------------------
// Sub-tab enum
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum AdminDepositTab {
    Deposits,
    RefundQueue,
    Refunded,
    Held,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[component]
pub fn AdminDeposits(
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    active_event_id: ReadSignal<Option<String>>,
    /// Deep-link trigger for the Record-Slip modal — when an Attendees-list
    /// row button sets this to `Some(id)`, the modal opens pre-filled.
    /// Owned by the `Admin` parent (so the Attendees section can write it
    /// even while `AdminDeposits` is unmounted); forwarded to the modal.
    pending_attendee_id: ReadSignal<Option<String>>,
    set_pending_attendee_id: WriteSignal<Option<String>>,
) -> impl IntoView {
    // Sub-tab state
    let (active_tab, set_active_tab) = signal(AdminDepositTab::Deposits);

    // Data state
    let (slips, set_slips) = signal(Vec::<ThbDepositInfo>::new());
    let (refunds, set_refunds) = signal(Vec::<ThbDepositInfo>::new());
    let (refunded_list, set_refunded_list) = signal(Vec::<ThbDepositInfo>::new());
    let (held_list, set_held_list) = signal(Vec::<ThbDepositInfo>::new());
    // Cross-event credit liability — organizer's total cash held as rolling
    // deposit credit across ALL contacts (backs the header chip). Global, not
    // per-event; loaded alongside the per-event lists for one refresh cycle.
    let (liability, set_liability) = signal(CreditLiability::default());
    // Per-event Cash/Credit/Comp source summary (GOAT reconciliation chip).
    let (source_summary, set_source_summary) = signal(api::DepositSourceSummary::default());
    // Cross-event credit-refund-request queue — contacts who requested return
    // of their held credit (Issue #061 Phase 3 exit path). Backs the badge on
    // the Held-as-Credit tab. Global, not per-event; same refresh cycle as the
    // other reads. Empty when D1 is unreachable (admin view still renders).
    let (credit_refund_requests, set_credit_refund_requests) =
        signal(Vec::<CreditRefundRequest>::new());
    // Tracks which email is currently being cleared (organizer "✓ Clear" click)
    // — disables the row's clear button while the POST is in flight and keys
    // per-row pending state. Idempotent clear, so a re-click is a safe retry.
    let (clear_pending_email, set_clear_pending_email) = signal(None::<String>);
    // Record-slip-on-behalf modal visibility — opens when admin clicks the
    // "Record Slip" button in the Deposits tab header. Backed by the new
    // `POST /api/deposit/thb/admin-upload` endpoint (skips the VULN-012
    // email-match gate; staff-authed + audited server-side).
    let (show_record_slip_modal, set_show_record_slip_modal) = signal(false);

    // UI state
    let (loading, set_loading) = signal(true);
    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let (action_pending, set_action_pending) = signal(None::<String>);
    let (confirm_reject_id, set_confirm_reject_id) = signal(None::<String>);
    // Refund proof: 2-step flow — first click shows input, second click confirms.
    // Per-row state: each refund queue item has its own proof URL value,
    // keyed by attendee_id. A single shared signal would cause typing in row A
    // to leak into rows B/C/D — a real bug observed in production testing.
    let (refund_proof_pending_id, set_refund_proof_pending_id) = signal(None::<String>);
    let (refund_proof_urls, set_refund_proof_urls) = signal(HashMap::<String, String>::new());

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
        let set_held_list = set_held_list;
        let set_liability = set_liability;
        let set_source_summary = set_source_summary;
        let set_credit_refund_requests = set_credit_refund_requests;
        let set_loading = set_loading;
        let set_toast = set_toast;

        leptos::task::spawn_local(async move {
            let slips_result = api::get_pending_slips(eid.as_deref()).await;
            let refunds_result = api::get_refund_queue(eid.as_deref()).await;
            let refunded_result = api::get_refunded_list(eid.as_deref()).await;
            let held_result = api::get_held_list(eid.as_deref()).await;
            let liability_result = api::get_credit_liability().await;
            let source_result = api::get_credit_used(eid.as_deref()).await;
            let refund_requests_result = api::get_credit_refund_requests().await;

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

            match held_result {
                Ok(data) => set_held_list.set(data.held),
                Err(e) => {
                    log::warn!("[admin-deposit] failed to load held list: {e}");
                }
            }

            // Liability is non-fatal — the deposits view must still render with
            // a zero chip if D1 is unreachable (backend already degrades to 0).
            match liability_result {
                Ok(data) => set_liability.set(data),
                Err(e) => {
                    log::warn!("[admin-deposit] failed to load credit liability: {e}");
                }
            }

            // Per-event Cash/Credit/Comp summary — non-fatal.
            match source_result {
                Ok(data) => set_source_summary.set(data.summary),
                Err(e) => {
                    log::warn!("[admin-deposit] failed to load source summary: {e}");
                }
            }

            // Credit refund requests is non-fatal — the Held-as-Credit tab must
            // still render with an empty badge if D1 is unreachable (backend
            // already degrades to empty list). Cross-event (global).
            match refund_requests_result {
                Ok(data) => set_credit_refund_requests.set(data.requests),
                Err(e) => {
                    log::warn!("[admin-deposit] failed to load credit refund requests: {e}");
                }
            }

            set_loading.set(false);
        });
    });

    // Helper to refresh data after an action
    let refresh_data = move || {
        set_refresh_counter.update(|c| *c += 1);
    };

    // Clear a credit-refund-request flag (Issue #061 Phase 3 — exit path).
    // Organizer clicks "✓ Clear" after processing the payout through the
    // existing refund tooling. Idempotent server-side, so a re-click is a safe
    // retry; the row's clear button stays disabled while its POST is in flight.
    let handle_clear_credit_refund_request = move |email: String| {
        let set_toast = set_toast;
        let set_clear_pending = set_clear_pending_email;
        let refresh = refresh_data;
        set_clear_pending.set(Some(email.clone()));

        leptos::task::spawn_local(async move {
            let body = ClearCreditRefundRequest { email: email.clone() };
            match api::clear_credit_refund_request(&body).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        "Cleared credit refund request.",
                        ToastType::Success,
                    );
                    refresh();
                }
                Err(e) => {
                    log::warn!("[admin-deposit] failed to clear credit refund request: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to clear request: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_clear_pending.set(None);
        });
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
            gloo_timers::callback::Timeout::new(3000, move || {
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

    // Mark as refunded — reads this row's refund proof URL from the per-row map
    let handle_mark_refunded = move |item: ThbDepositInfo| {
        let attendee_id = item.attendee_id.clone();
        let event_id = item.event_id.clone();
        let refund_proof_url = refund_proof_urls
            .get()
            .get(&attendee_id)
            .cloned()
            .unwrap_or_default();

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
        // Clear this row's input on success; leave others untouched
        set_refund_proof_urls.update(|m| {
            m.remove(&attendee_id);
        });

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

    // Mark a deposit as held-as-rolling-credit on behalf of the attendee
    // (attendee confirmed verbally / over chat but didn't tap the button
    // themselves). Mirrors handle_mark_refund's shape; the backend preserves
    // all financial invariants (settle-before-increment, idempotency guards).
    let handle_admin_hold = move |item: ThbDepositInfo| {
        let attendee_id = item.attendee_id.clone();
        let event_id = item.event_id.clone();
        let key = format!("hold-{attendee_id}");
        set_action_pending.set(Some(key));

        leptos::task::spawn_local(async move {
            let body = AdminHoldRequest { event_id };
            match api::admin_hold_deposit(&attendee_id, &body).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        "Deposit held as rolling credit",
                        ToastType::Success,
                    );
                    refresh_data();
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to hold deposit: {e}"),
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
    let held_count = Memo::new(move |_| held_list.get().len());
    // Cross-event count of contacts who requested return of held credit
    // (Issue #061 Phase 3). Backs the badge on the Held-as-Credit tab.
    let refund_request_count = Memo::new(move |_| credit_refund_requests.get().len());

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
            // Record-slip-on-behalf modal — admin records a THB slip for an
            // attendee who cannot upload themselves (JWT expired, browser bug,
            // slip sent via LINE/email). Mounted at the top of the
            // event-selected view; visibility is controlled by a signal.
            // Position:fixed inside the modal makes DOM placement irrelevant.
            <AdminRecordSlipModal
                show=show_record_slip_modal
                set_show=set_show_record_slip_modal
                event_id=active_event_id
                set_toast=set_toast
                on_success=refresh_data
                pending_attendee_id=pending_attendee_id
                set_pending_attendee_id=set_pending_attendee_id
            />
            // Credit liability header chip — organizer's total cash held as
            // rolling deposit credit across all contacts (Issue #061 Phase 2
            // option a2). Cross-event (global); only renders when there's
            // actual liability to surface (no clutter when balance is zero).
            <Show when=move || { let l = liability.get(); l.total_thb > 0 || l.total_usdc > 0 } fallback=|| view! { <div></div> }>
                <div
                    class="admin-dep-liability-chip"
                    title="Your total cash liability from rolling deposit credit — attendees who chose credit over refund. Auto-applies to their next event registration."
                >
                    <Icon icon=IconName::MoneyWings class="icon-sm"/>
                    <span>
                        {move || {
                            let l = liability.get();
                            let mut parts: Vec<String> = Vec::new();
                            if l.total_thb > 0 {
                                parts.push(format!("{} THB", l.total_thb));
                            }
                            if l.total_usdc > 0 {
                                parts.push(format!("{} USDC", l.total_usdc));
                            }
                            format!(
                                "Total credit held: {} across {} contacts",
                                parts.join(" + "),
                                l.contact_count
                            )
                        }}
                    </span>
                </div>
            </Show>
            // Per-event Cash/Credit/Comp summary chip (GOAT reconciliation): how
            // attendees got in for THIS event — paid cash, spent rolling credit,
            // or staff comp. Complements the per-attendee "Credit ✓" roster badge.
            <Show when=move || { let s = source_summary.get(); s.cash_count + s.credit_count + s.comp_count > 0 } fallback=|| view! { <div></div> }>
                <div
                    class="admin-dep-liability-chip"
                    title="How attendees got in for this event: paid cash vs spent rolling credit vs staff comp."
                >
                    <Icon icon=IconName::MoneyWings class="icon-sm"/>
                    <span>
                        {move || {
                            let s = source_summary.get();
                            format!(
                                "This event \u{2014} Cash: {} (\u{0e3f}{}) \u{00b7} Credit: {} (\u{0e3f}{}) \u{00b7} Comp: {}",
                                s.cash_count, s.cash_thb, s.credit_count, s.credit_thb, s.comp_count
                            )
                        }}
                    </span>
                </div>
            </Show>
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
                <button
                    class="tab"
                    class:active=move || active_tab.get() == AdminDepositTab::Held
                    on:click=move |_| set_active_tab.set(AdminDepositTab::Held)
                >
                    <Icon icon=IconName::MoneyWings class="icon-sm"/>" Held as Credit"
                    <Show when=move || held_count.get() != 0 fallback=|| view! { <span></span> }>
                        <span class="badge badge-success">
                            {move || held_count.get()}
                        </span>
                    </Show>
                    // Phase 3 exit path — attendees who requested return of held
                    // credit (Issue #061 §D3). Warning badge, separate from the
                    // success held-count badge so the organizer can see at a
                    // glance that action is requested.
                    <Show when=move || refund_request_count.get() != 0 fallback=|| view! { <span></span> }>
                        <span class="badge badge-warning" title="Attendees who requested return of their held credit">
                            {move || refund_request_count.get()}
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
                    <div class="admin-section-header" style="display:flex;justify-content:space-between;align-items:center;gap:0.5rem;flex-wrap:wrap;">
                        <h3 style="margin:0;">{format!("{} pending slip{}", pending_count.get(), if pending_count.get() != 1 { "s" } else { "" })}</h3>
                        <button
                            class="btn btn-secondary btn-sm"
                            title="Record a slip on behalf of an attendee who cannot upload themselves (slip sent via LINE/email, JWT expired, browser bug, etc.). Staff-authed + audited."
                            on:click=move |_| set_show_record_slip_modal.set(true)
                        >
                            <Icon icon=IconName::Ticket class="icon-sm" />
                            " Record Slip for Attendee"
                        </button>
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
                            let approve_disabled = current_action.as_ref().is_some_and(|k| k == &approve_key || k == &reject_key);
                            let reject_disabled = approve_disabled;
                            let approve_loading = current_action.as_ref() == Some(&approve_key);
                            let reject_loading = current_action.as_ref() == Some(&reject_key);

                            let amount = format!("{} THB", slip.amount_thb);
                            let uploaded_ago = utils::time_ago(&slip.uploaded_at);
                            let uploaded_formatted = utils::format_timestamp(&slip.uploaded_at);
                            let slip_url = slip.slip_url.clone();
                            // Only real serving paths are viewable slips. Credit-covered /
                            // staff-comp deposits store a sentinel (ROLLING_CREDIT_AUTO_APPLIED /
                            // STAFF_COMP_WAIVED) in slip_url — not a URL — so suppress the link.
                            let has_slip_url = slip_url
                                .as_deref()
                                .is_some_and(|u| u.starts_with("/api/") || u.starts_with("http"));
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
                                                <div class="admin-dep-slip-link-row">
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
                        <p class="admin-dep-flow-hint">
                            "Two steps per attendee: click "
                            <strong>"Enter Refund Proof"</strong>
                            " → paste the bank transfer receipt URL → click "
                            <strong>"Confirm Refund"</strong>
                            ". Each row keeps its own URL — filling one does not affect others."
                        </p>
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
                            let refund_disabled = current_action.as_ref() == Some(&refund_key);
                            let refund_loading = current_action.as_ref() == Some(&refund_key);
                            let hold_key = format!("hold-{item_id}");
                            let hold_disabled = current_action.as_ref() == Some(&hold_key);
                            let hold_loading = hold_disabled;

                            let amount = format!("{} THB", item.amount_thb);
                            let verified_by = item.verified_by.as_deref().unwrap_or("Unknown");
                            let verified_at = item.verified_at.as_deref().map(utils::format_timestamp).unwrap_or_else(|| "N/A".to_string());
                            let display_name = item.attendee_name.as_deref().unwrap_or(&item.attendee_id);
                            let slip_url = item.slip_url.clone();
                            // Only real serving paths are viewable slips. Credit-covered /
                            // staff-comp deposits store a sentinel (ROLLING_CREDIT_AUTO_APPLIED /
                            // STAFF_COMP_WAIVED) in slip_url — not a URL — so suppress the link.
                            let has_slip_url = slip_url
                                .as_deref()
                                .is_some_and(|u| u.starts_with("/api/") || u.starts_with("http"));
                            let has_bank_info = item.bank_account.is_some() && item.bank_name.is_some() && item.account_name.is_some();
                            let display_bank_account = item.bank_account.clone();
                            let display_bank_name = item.bank_name.clone();
                            let display_account_name = item.account_name.clone();

                            let item_for_refund = item.clone();
                            let item_for_hold = item.clone();
                            let item_id_for_click = item_id.clone();
                            let item_id_for_style = item_id.clone();
                            // Dedicated clones for the input's reactive closures.
                            // Each closure moves its own copy — without these,
                            // `item_id` would be moved twice (prop:value + on:input).
                            let item_id_for_value = item_id.clone();
                            let item_id_for_input = item_id.clone();

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
                                                <div class="admin-dep-slip-link-row">
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
                                            <div class="admin-dep-bank-section">
                                                <div class="panel-hint admin-dep-bank-label">"Refund Bank Info"</div>
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
                                                    <div class="badge badge-warning admin-dep-badge-row">
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
                                                title="Open a refund proof URL input for this attendee"
                                                on:click=move |_| {
                                                    set_refund_proof_pending_id.set(Some(item_id_for_click.clone()));
                                                    // Reset only this row's input — other rows are unaffected
                                                    set_refund_proof_urls.update(|m| {
                                                        m.insert(item_id_for_click.clone(), String::new());
                                                    });
                                                }
                                            >
                                                {if refund_loading { "Processing..." } else { "Enter Refund Proof" }}
                                            </button>
                                            <div
                                                style=move || if refund_proof_pending_id.get().as_deref() != Some(&item_id) { "display:none" } else { "display:flex;flex-direction:column;gap:0.25rem" }
                                            >
                                                <input
                                                    type="text"
                                                    class="form-input dep-input"
                                                    placeholder="Paste refund proof URL (transfer receipt)"
                                                    prop:value=move || refund_proof_urls.get().get(&item_id_for_value).cloned().unwrap_or_default()
                                                    on:input=move |ev| {
                                                        let val = event_target_value(&ev);
                                                        set_refund_proof_urls.update(|m| {
                                                            m.insert(item_id_for_input.clone(), val);
                                                        });
                                                    }
                                                />
                                                <div class="admin-dep-confirm-row">
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
                                            // Hold-as-credit action — use when the attendee
                                            // confirmed hold verbally but didn't tap their own
                                            // button. Credits the attendee's contact row.
                                            <div class="admin-dep-hold-row">
                                                <button
                                                    class="btn btn-secondary btn-sm"
                                                    disabled=hold_disabled
                                                    title="Hold this deposit as rolling credit for the attendee's next event (use when the attendee confirmed hold verbally)"
                                                    on:click=move |_| handle_admin_hold(item_for_hold.clone())
                                                >
                                                    {if hold_loading { "Holding..." } else { "↻ Hold as Credit" }}
                                                </button>
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
                            // Only real serving paths are viewable slips. Credit-covered /
                            // staff-comp deposits store a sentinel (ROLLING_CREDIT_AUTO_APPLIED /
                            // STAFF_COMP_WAIVED) in slip_url — not a URL — so suppress the link.
                            let has_slip_url = slip_url
                                .as_deref()
                                .is_some_and(|u| u.starts_with("/api/") || u.starts_with("http"));
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
                                                <div class="admin-dep-slip-link-row">
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
                                            <div class="admin-dep-bank-section">
                                                <div class="panel-hint admin-dep-bank-label">"Refund Bank Info"</div>
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
                                                    <div class="badge badge-warning admin-dep-badge-row">
                                                        "⚠ No bank info was provided"
                                                    </div>
                                                </Show>
                                            </div>

                                            // Refund proof link
                                            <Show when=move || has_refund_proof fallback=|| view! { <span></span> }>
                                                <div class="admin-dep-slip-link-row">
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

                // ── Held as Credit Tab ──
                <Show
                    when=move || active_tab.get() == AdminDepositTab::Held
                    fallback=|| view! { <div></div> }
                >
                    <div class="admin-section-header">
                        <h3><Icon icon=IconName::MoneyWings class="icon-sm icon-success"/>{format!(" {} deposit{} held as credit", held_count.get(), if held_count.get() != 1 { "s" } else { "" })}</h3>
                        <p class="admin-dep-flow-hint">
                            "These attendees' deposits are kept as rolling credit for their next event registration. They are excluded from the refund queue. Use the " <strong>"Hold as Credit"</strong> " action in the Refund Queue when an attendee confirms hold verbally."
                        </p>
                    </div>

                    // Phase 3 exit path — "Credit Refund Requested" sub-list
                    // (Issue #061 §D3). Cross-event: contacts who clicked
                    // "Request Return" on their ticket page. Rendered at the
                    // top of the Held tab (actionable items first). The
                    // organizer processes the actual payout through the existing
                    // refund tooling, then clicks "✓ Clear" to dismiss the
                    // request. Renders only when there's at least one open
                    // request (no clutter otherwise).
                    <Show
                        when=move || refund_request_count.get() != 0
                        fallback=|| view! { <div></div> }
                    >
                        <div class="admin-dep-credit-refund-requests">
                            <div class="admin-dep-flow-hint">
                                <Icon icon=IconName::Warning class="icon-sm"/>
                                {format!(
                                    "{} contact{} requested return of held credit. Process the payout through your refund channel, then clear the request.",
                                    refund_request_count.get(),
                                    if refund_request_count.get() != 1 { "s" } else { "" }
                                )}
                            </div>
                            {move || {
                                let items: Vec<_> = credit_refund_requests.get().iter().map(|req| {
                                    let email = req.email.clone();
                                    let name = req.name.clone();
                                    let credit_thb = req.credit_thb;
                                    let credit_usdc = req.credit_usdc;
                                    let requested_at = req.requested_at.clone();
                                    (email, name, credit_thb, credit_usdc, requested_at)
                                }).collect();

                                items.into_iter().map(|(email, name, credit_thb, credit_usdc, requested_at)| {
                                    let pending = clear_pending_email.get();
                                    let is_pending = pending.as_deref() == Some(email.as_str());
                                    let display_name = if name.is_empty() { email.clone() } else { name.clone() };
                                    let credit_str = if credit_thb > 0 && credit_usdc > 0 {
                                        format!("{} THB + {} USDC", credit_thb, credit_usdc)
                                    } else if credit_thb > 0 {
                                        format!("{} THB", credit_thb)
                                    } else if credit_usdc > 0 {
                                        format!("{} USDC", credit_usdc)
                                    } else {
                                        "0".to_string()
                                    };
                                    let requested_display = if requested_at.is_empty() {
                                        "N/A".to_string()
                                    } else {
                                        utils::format_timestamp(&requested_at)
                                    };
                                    let click_email = email.clone();
                                    view! {
                                        <div class="card admin-dep-credit-refund-row">
                                            <div class="flex-row-wrap">
                                                <div>
                                                    <div class="admin-attendee-name">
                                                        {utils::escape_html(&display_name)}
                                                    </div>
                                                    <div class="admin-amount-line">
                                                        {format!("Held credit: {}", credit_str)}
                                                    </div>
                                                    <div class="panel-hint">
                                                        {format!("Requested: {}", requested_display)}
                                                    </div>
                                                </div>
                                                <div>
                                                    <span class="badge badge-warning">"Refund Requested"</span>
                                                    <button
                                                        class="btn btn-success btn-xs admin-dep-clear-btn"
                                                        disabled=move || is_pending
                                                        on:click=move |_| {
                                                            handle_clear_credit_refund_request(click_email.clone());
                                                        }
                                                    >
                                                        {move || if is_pending {
                                                            view! { <span>"Clearing..."</span> }.into_any()
                                                        } else {
                                                            view! {
                                                                <Icon icon=IconName::Check class="icon-sm"/>
                                                                " ✓ Clear"
                                                            }.into_any()
                                                        }}
                                                    </button>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }).collect_view()
                            }}
                        </div>
                    </Show>

                    <Show
                        when=move || held_count.get() == 0
                        fallback=|| view! { <div></div> }
                    >
                        <div class="admin-empty-state">
                            "No deposits held as credit"
                        </div>
                    </Show>

                    {move || {
                        let items: Vec<_> = held_list.get().iter().map(|item| {
                            let amount = format!("{} THB", item.amount_thb);
                            let verified_by = item.verified_by.as_deref().unwrap_or("Unknown").to_string();
                            let held_at = item.held_as_credit_at.as_deref().map(utils::format_timestamp).unwrap_or_else(|| "N/A".to_string());
                            let display_name = item.attendee_name.as_deref().unwrap_or(&item.attendee_id).to_string();
                            let slip_url = item.slip_url.clone();
                            // Only real serving paths are viewable slips. Credit-covered /
                            // staff-comp deposits store a sentinel (ROLLING_CREDIT_AUTO_APPLIED /
                            // STAFF_COMP_WAIVED) in slip_url — not a URL — so suppress the link.
                            let has_slip_url = slip_url
                                .as_deref()
                                .is_some_and(|u| u.starts_with("/api/") || u.starts_with("http"));

                            (amount, verified_by, held_at, display_name, slip_url, has_slip_url)
                        }).collect();

                        items.into_iter().map(|(amount, verified_by, held_at, display_name, slip_url, has_slip_url)| {
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
                                                {format!("Held at: {held_at}")}
                                            </div>

                                            // Slip image link
                                            <Show when=move || has_slip_url fallback=|| view! { <span></span> }>
                                                <div class="admin-dep-slip-link-row">
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
                                        <div>
                                            <span class="badge badge-success">"✓ Held as Credit"</span>
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
