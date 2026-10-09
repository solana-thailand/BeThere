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
use wasm_bindgen::JsValue;

use crate::api::{
    self, AdminHoldRequest, CompDepositRequest, CreditLiability, CreditRefundRequest,
    MarkRefundRequest, ThbDepositInfo, VerifySlipRequest,
};
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};
use crate::pages::admin_deposit_bank_info::{
    load_refund_note, refund_bank_info, refund_copy_buttons, refund_note_editor,
};
use crate::pages::admin_deposit_credit_requests::{CreditPayoutCandidates, CreditRefundRequests};
use crate::pages::admin_deposit_queue_comp::QueueCompAction;
use crate::pages::admin_deposit_record_slip::AdminRecordSlipModal;
use crate::pages::admin_deposit_settled::{SettledDepositList, SettledKind};
use crate::pages::admin_deposit_slip_link::slip_link;
use crate::pages::admin_deposit_slip_proposal::SlipProposalLine;
use crate::pages::admin_deposit_summary_chips::DepositSummaryChips;
use crate::pages::admin_linked_emails::AdminLinkedEmails;
use crate::pages::admin_refund_queue_filter::{RefundQueueFilter, RefundQueueFilterBar};
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

/// The Worker's cap on a data-URL proof (`validate_slip_url`: 5 MiB encoded,
/// about a 3 MB image).
const MAX_PROOF_DATA_URL_LEN: usize = 5 * 1024 * 1024;

#[component]
pub fn AdminDeposits(
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    active_event_id: ReadSignal<Option<String>>,
    /// The selected event's name, used in the refund transfer note.
    #[prop(into)]
    event_name: Signal<String>,
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
    // Slip fingerprints that more than one attendee in this event submitted —
    // the same payment image, sent by two different people. The organizer used
    // to catch these by recognising the person at refund time (`.issues/129`).
    let (duplicate_slip_hashes, set_duplicate_slip_hashes) = signal(Vec::<String>::new());
    // The slip agent's advisory proposal per attendee id (`.plans/033` W1).
    let (slip_proposals, set_slip_proposals) = signal(HashMap::<
        String,
        event_checkin_domain::slip_proposal::SlipProposal,
    >::new());
    let (refunds, set_refunds) = signal(Vec::<ThbDepositInfo>::new());
    let (refund_context, set_refund_context) = signal(HashMap::<
        String,
        event_checkin_domain::models::deposit::RefundQueueContext,
    >::new());
    let (refund_filter, set_refund_filter) = signal(RefundQueueFilter::All);
    let (refunded_list, set_refunded_list) = signal(Vec::<ThbDepositInfo>::new());
    let (held_list, set_held_list) = signal(Vec::<ThbDepositInfo>::new());
    // Cross-event credit liability — organizer's total cash held as rolling
    // deposit credit across ALL contacts (backs the header chip). Global, not
    // per-event; loaded alongside the per-event lists for one refresh cycle.
    let (liability, set_liability) = signal(CreditLiability::default());
    // Per-event Cash/Credit/Comp source summary (GOAT reconciliation chip).
    let (source_summary, set_source_summary) = signal(api::DepositSourceSummary::default());
    // The attendees who got in by spending rolling credit (for the summary list).
    let (credit_used_list, set_credit_used_list) = signal(Vec::<ThbDepositInfo>::new());
    // Cross-event credit-refund-request queue — contacts who requested return
    // of their held credit (Issue #061 Phase 3 exit path). Backs the badge on
    // the Held-as-Credit tab. Global, not per-event; same refresh cycle as the
    // other reads. Empty when D1 is unreachable (admin view still renders).
    let (credit_refund_requests, set_credit_refund_requests) =
        signal(Vec::<CreditRefundRequest>::new());
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
    // Mirrors confirm_reject_id: the comp button also needs a deliberate
    // second click, because it writes off money and cannot be undone here.
    let (confirm_comp_id, set_confirm_comp_id) = signal(None::<String>);
    // 2-step confirm for the irreversible "Hold as Credit" money action.
    let (confirm_hold_id, set_confirm_hold_id) = signal(None::<String>);
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
        let set_duplicate_slip_hashes = set_duplicate_slip_hashes;
        let set_slip_proposals = set_slip_proposals;
        let set_refunds = set_refunds;
        let set_held_list = set_held_list;
        let set_liability = set_liability;
        let set_source_summary = set_source_summary;
        let set_credit_used_list = set_credit_used_list;
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
                Ok(data) => {
                    set_slips.set(data.slips);
                    set_duplicate_slip_hashes.set(data.duplicate_slip_hashes);
                    set_slip_proposals.set(data.slip_proposals);
                }
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
                Ok(data) => {
                    set_refunds.set(data.pending);
                    set_refund_context.set(data.context);
                }
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

            // Per-event Cash/Credit/Comp summary + credit-used list — non-fatal.
            match source_result {
                Ok(data) => {
                    set_source_summary.set(data.summary);
                    set_credit_used_list.set(data.credit_used);
                }
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

    // Admit the attendee without owing them a refund (.issues/129 Gap 1).
    //
    // Two-step confirmation, same as reject: this writes off real money the
    // attendee says they sent, and it is not undoable from this screen.
    let handle_comp = move |slip: ThbDepositInfo| {
        let attendee_id = slip.attendee_id.clone();

        if confirm_comp_id.get().as_deref() != Some(&attendee_id) {
            set_confirm_comp_id.set(Some(attendee_id.clone()));
            let set_confirm = set_confirm_comp_id;
            gloo_timers::callback::Timeout::new(3000, move || {
                set_confirm.set(None);
            })
            .forget();
            return;
        }

        let event_id = slip.event_id.clone();
        let key = format!("comp-{attendee_id}");
        set_confirm_comp_id.set(None);
        set_action_pending.set(Some(key));

        leptos::task::spawn_local(async move {
            let body = CompDepositRequest {
                event_id,
                attendee_id: attendee_id.clone(),
                reason: None,
            };
            match api::comp_thb_deposit(&body).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        "Attendee admitted — no refund owed",
                        ToastType::Success,
                    );
                    refresh_data();
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to admit without deposit: {e}"),
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
            })
            .forget();
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
                    components::show_toast(&set_toast, "Slip rejected", ToastType::Warning);
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
            let body = MarkRefundRequest {
                event_id,
                refund_proof_url,
            };
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

        // First click arms confirm — Hold irreversibly converts a refundable cash
        // deposit into rolling credit, so require a 2-step (like reject/delete).
        if confirm_hold_id.get().as_deref() != Some(&attendee_id) {
            set_confirm_hold_id.set(Some(attendee_id.clone()));
            let set_confirm = set_confirm_hold_id;
            gloo_timers::callback::Timeout::new(3000, move || {
                set_confirm.set(None);
            })
            .forget();
            return;
        }
        set_confirm_hold_id.set(None);

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

    // One transfer note per event, shared by every row's Copy note button:
    // the organizer's saved edit, else a default from the event name.
    let (refund_note_text, set_refund_note_text) = signal(String::new());
    Effect::new(move |_| {
        let name = event_name.get();
        let note = match active_event_id.get() {
            Some(id) => load_refund_note(&id, &name),
            None => String::new(),
        };
        set_refund_note_text.set(note);
    });

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
            <DepositSummaryChips
                liability=liability
                source_summary=source_summary
                credit_used_list=credit_used_list
            />
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
                            class="btn btn-outline btn-sm"
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
                        let duplicates = duplicate_slip_hashes.get();
                        let proposals = slip_proposals.get();
                        slips.get().iter().map(|slip| {
                            // A row is flagged only when its OWN fingerprint is
                            // in the set. A slip with no fingerprint (anything
                            // uploaded before 2026-09-22) is never flagged:
                            // unknown is not the same as clean, and saying
                            // otherwise would make the badge worthless.
                            let is_duplicate = slip
                                .slip_blake3
                                .as_deref()
                                .is_some_and(|h| !h.is_empty() && duplicates.iter().any(|d| d == h));
                            let slip_id = slip.attendee_id.clone();
                            let proposal = proposals.get(&slip_id).cloned();
                            let approve_key = format!("approve-{slip_id}");
                            let reject_key = format!("reject-{slip_id}");
                            let approve_disabled = current_action.as_ref().is_some_and(|k| k == &approve_key || k == &reject_key);
                            let reject_disabled = approve_disabled;
                            let approve_loading = current_action.as_ref() == Some(&approve_key);
                            let reject_loading = current_action.as_ref() == Some(&reject_key);

                            let amount = format!("{} THB", slip.amount_thb);
                            let uploaded_ago = utils::time_ago(&slip.uploaded_at);
                            let uploaded_formatted = utils::format_timestamp(&slip.uploaded_at);
                            let display_name = slip.attendee_name.as_deref().unwrap_or(&slip.attendee_id);

                            let slip_for_approve = slip.clone();
                            let slip_for_reject = slip.clone();
                            let slip_for_comp = slip.clone();
                            let is_confirming = confirm_reject_id.get().as_deref() == Some(&slip_id);
                            let is_confirming_comp = confirm_comp_id.get().as_deref() == Some(&slip_id);
                            let comp_key = format!("comp-{slip_id}");
                            let comp_disabled = current_action.as_ref().is_some_and(|k| k == &approve_key || k == &reject_key || k == &comp_key);
                            let comp_loading = current_action.as_ref() == Some(&comp_key);

                            view! {
                                <div class="card">
                                    <div class="flex-row-wrap">
                                        <div>
                                            <div class="admin-attendee-name">
                                                {format!("Attendee: {display_name}")}
                                            </div>
                                            <div class="admin-amount-line">
                                                {amount}
                                            </div>
                                            <Show when=move || is_duplicate fallback=|| view! { <span></span> }>
                                                <div class="admin-dep-duplicate-warning">
                                                    "⚠ Same image as another attendee's slip in this event — check before approving. Approving is what promises the refund."
                                                </div>
                                            </Show>
                                            <SlipProposalLine proposal=proposal />
                                            <div class="panel-hint">
                                                {"Uploaded: "}
                                                <span title={uploaded_formatted.clone()}>{uploaded_ago.clone()}</span>
                                            </div>
                                            {slip_link(slip.slip_url.clone())}
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
                                            // Admit without owing a refund. Sits
                                            // between Approve and Reject because
                                            // that is where it belongs: the
                                            // organizer wants to let them in AND
                                            // not pay them back, and until now
                                            // they had to pick one.
                                            <button
                                                class=if is_confirming_comp { "btn btn-confirm-danger btn-sm" } else { "btn btn-outline btn-sm" }
                                                disabled=comp_disabled
                                                title="Give this attendee their ticket, but record the deposit as a comp — no refund will be owed. Use when you know the payment did not arrive."
                                                on:click=move |_| handle_comp(slip_for_comp.clone())
                                            >
                                                {if comp_loading { "Admitting..." } else if is_confirming_comp { "⚠ Confirm: no refund owed?" } else { "Admit, no refund owed" }}
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
                            "Per attendee: copy the account and amount into your bank app and transfer, then click "
                            <strong>"Enter Refund Proof"</strong>
                            " → attach the slip image (or paste a receipt link) → click "
                            <strong>"Confirm Refund"</strong>
                            ". Each row keeps its own proof — filling one does not affect others."
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

                    {refund_note_editor(refund_note_text, set_refund_note_text, active_event_id.into())}

                    <RefundQueueFilterBar
                        refunds=refunds
                        context=refund_context
                        filter=refund_filter
                        set_filter=set_refund_filter
                    />

                    {move || {
                        let current_action = action_pending.get();
                        let filter = refund_filter.get();
                        let context = refund_context.get();
                        refunds.get().iter().filter(|item| filter.matches(context.get(&item.attendee_id))).map(|item| {
                            let item_id = item.attendee_id.clone();
                            let refund_key = format!("refund-{item_id}");
                            let refund_disabled = current_action.as_ref() == Some(&refund_key);
                            let refund_loading = current_action.as_ref() == Some(&refund_key);
                            let hold_key = format!("hold-{item_id}");
                            let hold_disabled = current_action.as_ref() == Some(&hold_key);
                            let hold_loading = hold_disabled;
                            let is_confirming_hold = confirm_hold_id.get().as_deref() == Some(&item_id);

                            let amount = format!("{} THB", item.amount_thb);
                            let verified_by = item.verified_by.as_deref().unwrap_or("Unknown");
                            let verified_at = item.verified_at.as_deref().map(utils::format_timestamp).unwrap_or_else(|| "N/A".to_string());
                            let display_name = item.attendee_name.as_deref().unwrap_or(&item.attendee_id);
                            let bank_info = refund_bank_info(
                                item.bank_account.clone(),
                                item.bank_name.clone(),
                                item.account_name.clone(),
                                "⚠ No bank info — ask attendee",
                            );

                            let item_for_refund = item.clone();
                            let item_for_hold = item.clone();
                            let comp_event_id = item.event_id.clone();
                            let comp_attendee_id = item.attendee_id.clone();
                            let item_id_for_click = item_id.clone();
                            let item_id_for_style = item_id.clone();
                            // Dedicated clones for the input's reactive closures.
                            // Each closure moves its own copy — without these,
                            // `item_id` would be moved twice (prop:value + on:input).
                            let item_id_for_value = item_id.clone();
                            let item_id_for_input = item_id.clone();
                            let item_id_for_file = item_id.clone();
                            let item_id_for_attached = item_id.clone();
                            let copy_buttons = refund_copy_buttons(
                                item.bank_account.clone(),
                                item.amount_thb,
                                refund_note_text.into(),
                                set_toast,
                            );
                            // Came to the event: refund these first.
                            let checked_in =
                                RefundQueueFilter::row_checked_in(context.get(&item.attendee_id));

                            view! {
                                <div class="card">
                                    <div class="flex-row-wrap">
                                        <div>
                                            <div class="admin-attendee-name">
                                                {format!("Attendee: {display_name}")}
                                                {checked_in.then(|| view! {
                                                    " " <span class="badge badge-success">"Checked in"</span>
                                                })}
                                            </div>
                                            <div class="admin-amount-line">
                                                {amount}
                                            </div>
                                            <div class="panel-hint">
                                                {format!("Verified by: {verified_by}")}
                                            </div>
                                            <div class="panel-hint">
                                                {format!("Verified at: {verified_at}")}
                                            </div>

                                            {slip_link(item.slip_url.clone())}

                                            {bank_info}
                                            {copy_buttons}
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
                                                // The slip straight from the phone's gallery; the
                                                // Worker stores a data-URL proof in R2 (refund.rs).
                                                <label class="form-label">"Attach slip image (JPEG, PNG, WebP, max 3MB)"</label>
                                                <input
                                                    type="file"
                                                    accept="image/jpeg,image/png,image/webp"
                                                    class="file-input-styled"
                                                    on:change=move |ev| {
                                                        let id = item_id_for_file.clone();
                                                        let target: JsValue = event_target::<web_sys::HtmlInputElement>(&ev).into();
                                                        leptos::task::spawn_local(async move {
                                                            match crate::pages::deposit::js_interop::read_file_as_data_url(&target).await {
                                                                Some(data_url) if data_url.len() <= MAX_PROOF_DATA_URL_LEN => {
                                                                    set_refund_proof_urls.update(|m| { m.insert(id, data_url); });
                                                                }
                                                                Some(_) => components::show_toast(&set_toast, "Slip image is over 3MB — take a screenshot of it instead", ToastType::Error),
                                                                None => components::show_toast(&set_toast, "Could not read that image", ToastType::Error),
                                                            }
                                                        });
                                                    }
                                                />
                                                {move || refund_proof_urls.with(|m| m.get(&item_id_for_attached).is_some_and(|v| v.starts_with("data:image/"))).then(|| view! {
                                                    <span class="badge badge-success" style="align-self:flex-start">"Slip attached"</span>
                                                })}
                                                <input
                                                    type="text"
                                                    class="form-input dep-input"
                                                    placeholder="…or paste a receipt link"
                                                    prop:value=move || refund_proof_urls.get().get(&item_id_for_value).cloned().filter(|v| !v.starts_with("data:")).unwrap_or_default()
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
                                                    class="btn btn-outline btn-sm"
                                                    disabled=hold_disabled
                                                    title="Hold this deposit as rolling credit for the attendee's next event (use when the attendee confirmed hold verbally)"
                                                    on:click=move |_| handle_admin_hold(item_for_hold.clone())
                                                >
                                                    {if hold_loading { "Holding..." } else if is_confirming_hold { "Confirm hold?" } else { "↻ Hold as Credit" }}
                                                </button>
                                            </div>
                                            <QueueCompAction
                                                event_id=comp_event_id
                                                attendee_id=comp_attendee_id
                                                set_toast=set_toast
                                                set_refresh_counter=set_refresh_counter
                                            />
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

                    <SettledDepositList items=refunded_list kind=SettledKind::Refunded/>
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

                    // Super admins link a returner's emails so they share credit
                    // (plan 025 §7.4, `.issues/122`). Renders nothing for others.
                    <AdminLinkedEmails set_toast=set_toast/>

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
                        <CreditRefundRequests
                            requests=credit_refund_requests
                            set_toast=set_toast
                            set_refresh_counter=set_refresh_counter
                        />
                    </Show>

                    // .issues/192 — holders the organizer can pay unasked.
                    <CreditPayoutCandidates
                        refresh_counter=refresh_counter
                        set_toast=set_toast
                        set_refresh_counter=set_refresh_counter
                    />

                    <Show
                        when=move || held_count.get() == 0
                        fallback=|| view! { <div></div> }
                    >
                        <div class="admin-empty-state">
                            // The held list is per event; the payout candidates above
                            // are cross-event, so say which list is empty.
                            "No deposits held as credit for this event"
                        </div>
                    </Show>

                    <SettledDepositList items=held_list kind=SettledKind::Held/>
                </Show>

            </Show>
            </Show>
        </div>
    }
}
