//! Modal component for admins to record a THB slip on behalf of an attendee.
//!
//! Use case: attendee sent the slip via off-platform channel (LINE/email) and
//! cannot upload themselves (JWT expired, browser bug, etc.).
//!
//! Mounted inside `AdminDeposits` (`pages/admin_deposit.rs`). Renders as a
//! fixed-position overlay with a backdrop. Mirrors the field shape of the
//! attendee THB upload form: attendee_id, slip image, bank info, plus an
//! `auto_verify` toggle (defaults to `true` — admin recording a confirmed
//! payment typically also verifies it in the same call).
//!
//! ## Deep-link trigger
//!
//! The modal watches a `pending_attendee_id` signal. When a parent sets it
//! to `Some(id)` (e.g. the Attendees list "Record slip" button on an attendee
//! row), this modal opens itself, pre-fills the attendee_id field, and the
//! parent clears the signal back to `None`. This avoids prop-drilling the
//! modal's own visibility state up to a grandparent and works from any source
//! (button click, URL param, etc.).
//!
//! ## Styling note
//!
//! Inline styles are used for the overlay positioning because the codebase
//! has no existing modal pattern. The card body reuses the existing `card`
//! class for consistency with the rest of the admin UI. New modal-specific
//! classes (`admin-modal-*`) are introduced for future CSS targeting — they
//! degrade gracefully (display: block) without explicit styles.
//!
//! ## Why no bank-name autocomplete?
//!
//! The attendee form (`pages/deposit/thb_payment.rs`) ships a `THAI_BANKS`
//! autocomplete dropdown because end users often don't know bank codes.
//! Admins recording slips on behalf of attendees can type bank names directly;
//! the backend doesn't normalize, so a plain string is fine. Skipping the
//! dropdown also avoids duplicating the bank list across modules.

use leptos::prelude::*;
use wasm_bindgen::JsValue;

use crate::api::{self, AdminSlipUploadRequest};
use crate::components::{self, LightboxImage, ToastType};
use crate::icons::{Icon, IconName};
// Reuses the attendee-side file→data-URL reader. Lives under the deposit page
// module; if its visibility ever changes to private, expose a pub re-export
// there and update this path.
use crate::pages::deposit::js_interop;

#[component]
pub fn AdminRecordSlipModal(
    /// Controls visibility — `true` renders the overlay.
    show: ReadSignal<bool>,
    set_show: WriteSignal<bool>,
    /// Currently selected event ID (required — admin must have picked an
    /// event in the parent's event selector before recording a slip).
    event_id: ReadSignal<Option<String>>,
    /// Toast sink — same signal the parent page uses (passed in so toasts
    /// appear in the parent's toast container, not a nested one).
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    /// Refresh callback — fired after a successful upload so the parent's
    /// pending list / refund queue updates immediately.
    on_success: impl Fn() + Clone + Send + Sync + 'static,
    /// Deep-link trigger — when a parent sets this to `Some(id)`, the modal
    /// opens itself and pre-fills the attendee_id field. The parent must
    /// clear it back to `None` after the Effect fires (otherwise re-opening
    /// would re-trigger). Pattern: parent owns the signal, modal only reads.
    pending_attendee_id: ReadSignal<Option<String>>,
    set_pending_attendee_id: WriteSignal<Option<String>>,
) -> impl IntoView {
    // ── Form state ──────────────────────────────────────────────────────
    let (attendee_id, set_attendee_id) = signal(String::new());
    let (slip_preview, set_slip_preview) = signal(None::<String>);
    let (bank_account, set_bank_account) = signal(String::new());
    let (bank_name, set_bank_name) = signal(String::new());
    let (account_name, set_account_name) = signal(String::new());
    let (auto_verify, set_auto_verify) = signal(true);
    let (submitting, set_submitting) = signal(false);
    let file_input_ref: NodeRef<leptos::html::Input> = NodeRef::new();

    // Deep-link watcher: when a parent sets `pending_attendee_id` to Some(id),
    // open the modal and pre-fill the attendee_id field, then clear the
    // trigger signal so it can fire again later. Runs once per Some value.
    // Consumes the value (clears back to None) so the parent doesn't have to
    // track whether the modal saw it.
    Effect::new(move |_| {
        if let Some(id) = pending_attendee_id.get() {
            set_attendee_id.set(id);
            set_show.set(true);
            set_pending_attendee_id.set(None);
        }
    });

    // Reset form whenever the modal closes (clean slate for next open).
    Effect::new(move |_| {
        if !show.get() {
            set_attendee_id.set(String::new());
            set_slip_preview.set(None);
            set_bank_account.set(String::new());
            set_bank_name.set(String::new());
            set_account_name.set(String::new());
            set_auto_verify.set(true);
            set_submitting.set(false);
            if let Some(el) = file_input_ref.get() {
                el.set_value("");
            }
        }
    });

    let close = move || set_show.set(false);

    // `on_success` is `Fn() + Clone` but not `Copy`. Wrapping it in a
    // `StoredValue` (which IS `Copy`) lets `handle_submit` capture only Copy
    // types, so the closure itself becomes Copy/Fn — required because the
    // Leptos view! body re-runs as Fn and can't move a FnOnce handler.
    let on_success_stored = StoredValue::new(on_success);

    let handle_submit = move |_| {
        let attendee = attendee_id.get().trim().to_string();
        let event = event_id.get();
        let slip = slip_preview.get();
        let bank_acc = bank_account.get().trim().to_string();
        let bank_nm = bank_name.get().trim().to_string();
        let acc_name = account_name.get().trim().to_string();
        let verify = auto_verify.get();
        let refresh = on_success_stored.with_value(|f| f.clone());

        // ── Client-side validation (mirrors server-side checks) ──────────
        if attendee.is_empty() {
            components::show_toast(&set_toast, "Attendee ID is required", ToastType::Error);
            return;
        }
        let Some(event_id) = event else {
            components::show_toast(&set_toast, "Select an event first", ToastType::Error);
            return;
        };
        if slip.as_ref().is_none_or(|s| s.is_empty()) {
            components::show_toast(&set_toast, "Slip image is required", ToastType::Error);
            return;
        }
        if bank_acc.is_empty() || bank_nm.is_empty() || acc_name.is_empty() {
            components::show_toast(
                &set_toast,
                "Bank account, bank name, and account holder are all required",
                ToastType::Error,
            );
            return;
        }

        set_submitting.set(true);
        let body = AdminSlipUploadRequest {
            event_id,
            attendee_id: attendee,
            slip_url: slip.unwrap_or_default(),
            bank_account: Some(bank_acc),
            bank_name: Some(bank_nm),
            account_name: Some(acc_name),
            auto_verify: verify,
        };

        leptos::task::spawn_local(async move {
            match api::admin_upload_thb_slip(&body).await {
                Ok(_) => {
                    let msg = if body.auto_verify {
                        "Slip recorded and verified"
                    } else {
                        "Slip recorded (pending verification)"
                    };
                    components::show_toast(&set_toast, msg, ToastType::Success);
                    set_show.set(false);
                    refresh();
                }
                Err(e) => {
                    log::warn!("[admin-deposit] failed to record slip on behalf: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to record slip: {e}"),
                        ToastType::Error,
                    );
                    set_submitting.set(false);
                }
            }
        });
    };

    // ── Render ──────────────────────────────────────────────────────────
    view! {
        <Show when=move || show.get() fallback=|| view! { <div></div> }>
            <div
                class="admin-modal-backdrop"
                style="position:fixed;inset:0;background:rgba(0,0,0,0.6);z-index:1000;display:flex;align-items:flex-start;justify-content:center;padding:2rem 1rem;overflow-y:auto;"
                role="presentation"
                on:click=move |_| close()
            >
                <div
                    class="card admin-modal-card"
                    style="max-width:600px;width:100%;background:var(--bg-card, #fff);box-shadow:0 8px 32px rgba(0,0,0,0.2);"
                    role="dialog"
                    aria-modal="true"
                    aria-label="Record slip for attendee"
                    on:click=move |ev| ev.stop_propagation()
                >
                    // ── Header ─────────────────────────────────────────────
                    <div
                        class="admin-modal-header"
                        style="display:flex;justify-content:space-between;align-items:center;padding-bottom:0.75rem;border-bottom:1px solid var(--border-muted, #e5e7eb);"
                    >
                        <h3 style="margin:0;display:flex;align-items:center;gap:0.4rem;">
                            <Icon icon=IconName::Ticket class="icon-sm" />
                            " Record Slip for Attendee"
                        </h3>
                        <button
                            class="admin-modal-close"
                            style="background:none;border:none;font-size:1.5rem;line-height:1;cursor:pointer;color:var(--text-muted, #6b7280);padding:0.25rem 0.5rem;"
                            aria-label="Close"
                            on:click=move |_| close()
                            disabled=move || submitting.get()
                        >
                            "×"
                        </button>
                    </div>

                    // ── Body ──────────────────────────────────────────────
                    <div class="admin-modal-body" style="padding-top:0.75rem;">
                        <Show
                            when=move || event_id.get().is_some()
                            fallback=|| view! {
                                <div class="admin-empty-state">
                                    "Select an event in the dropdown above before recording a slip."
                                </div>
                            }
                        >
                            <p class="hint-desc" style="margin-bottom:0.75rem;">
                                "Use when an attendee sent their slip via LINE/email and cannot upload themselves (session expired, browser bug, etc.). All actions are audited with your email."
                            </p>

                            // Attendee ID
                            <label class="form-label">"Attendee ID *"</label>
                            <input
                                type="text"
                                class="form-input"
                                placeholder="e.g. abc123def456 (from the Attendees list)"
                                prop:value=move || attendee_id.get()
                                on:input=move |ev| set_attendee_id.set(event_target_value(&ev))
                                disabled=move || submitting.get()
                            />

                            // Slip upload
                            <label class="form-label" style="margin-top:0.75rem;">
                                "Payment Slip *"
                            </label>
                            <p class="hint-muted" style="margin:0 0 0.25rem 0;font-size:0.85rem;">
                                "JPEG, PNG, or WebP. Max 3MB."
                            </p>
                            <input
                                type="file"
                                accept="image/jpeg,image/png,image/webp"
                                node_ref=file_input_ref
                                class="file-input-styled"
                                on:change=move |_| {
                                    let file_ref = file_input_ref;
                                    leptos::task::spawn_local(async move {
                                        if let Some(el) = file_ref.get() {
                                            let js_val: JsValue = el.into();
                                            let preview =
                                                js_interop::read_file_as_data_url(&js_val).await;
                                            set_slip_preview.set(preview);
                                        }
                                    });
                                }
                                disabled=move || submitting.get()
                            />

                            <Show
                                when=move || slip_preview.get().is_some()
                                fallback=|| view! { <div></div> }
                            >
                                <div
                                    class="slip-preview-container"
                                    style="margin-top:0.5rem;display:flex;align-items:flex-start;gap:0.5rem;"
                                >
                                    <LightboxImage
                                        src=slip_preview.get().unwrap_or_default()
                                        alt="Slip preview"
                                        thumb_class="slip-preview-img"
                                        hint="Tap the image or backdrop to close".to_string()
                                    />
                                    <button
                                        class="slip-preview-remove btn btn-secondary btn-sm"
                                        on:click=move |_| {
                                            set_slip_preview.set(None);
                                            if let Some(el) = file_input_ref.get() {
                                                el.set_value("");
                                            }
                                        }
                                        disabled=move || submitting.get()
                                    >
                                        "Remove"
                                    </button>
                                </div>
                            </Show>

                            // Bank info
                            <label class="form-label" style="margin-top:0.75rem;">
                                "Bank Account Number *"
                            </label>
                            <input
                                type="text"
                                class="form-input"
                                placeholder="e.g. 123-4-56789-0"
                                prop:value=move || bank_account.get()
                                on:input=move |ev| set_bank_account.set(event_target_value(&ev))
                                disabled=move || submitting.get()
                            />

                            <label class="form-label" style="margin-top:0.75rem;">
                                "Bank Name *"
                            </label>
                            <input
                                type="text"
                                class="form-input"
                                placeholder="e.g. KBank, SCB, BBL"
                                prop:value=move || bank_name.get()
                                on:input=move |ev| set_bank_name.set(event_target_value(&ev))
                                disabled=move || submitting.get()
                            />

                            <label class="form-label" style="margin-top:0.75rem;">
                                "Account Holder Name *"
                            </label>
                            <input
                                type="text"
                                class="form-input"
                                placeholder="As shown on the bank book"
                                prop:value=move || account_name.get()
                                on:input=move |ev| set_account_name.set(event_target_value(&ev))
                                disabled=move || submitting.get()
                            />

                            // Auto-verify toggle
                            <label
                                class="admin-modal-checkbox-row"
                                style="display:flex;align-items:flex-start;gap:0.5rem;margin-top:0.75rem;cursor:pointer;"
                            >
                                <input
                                    type="checkbox"
                                    prop:checked=move || auto_verify.get()
                                    on:change=move |ev| set_auto_verify.set(event_target_checked(&ev))
                                    disabled=move || submitting.get()
                                    style="margin-top:0.2rem;"
                                />
                                <span style="font-size:0.9rem;">
                                    <strong>"Auto-verify"</strong>
                                    " — also mark as verified (recommended). Generates the attendee's QR code immediately. Uncheck to record as pending for review."
                                </span>
                            </label>
                        </Show>
                    </div>

                    // ── Footer ────────────────────────────────────────────
                    <div
                        class="admin-modal-footer"
                        style="display:flex;justify-content:flex-end;gap:0.5rem;padding-top:0.75rem;margin-top:0.75rem;border-top:1px solid var(--border-muted, #e5e7eb);"
                    >
                        <button
                            class="btn btn-secondary"
                            on:click=move |_| close()
                            disabled=move || submitting.get()
                        >
                            "Cancel"
                        </button>
                        <button
                            class="btn btn-primary"
                            on:click=handle_submit
                            disabled=move || {
                                submitting.get()
                                || attendee_id.get().trim().is_empty()
                                || slip_preview.get().is_none()
                                || bank_account.get().trim().is_empty()
                                || bank_name.get().trim().is_empty()
                                || account_name.get().trim().is_empty()
                            }
                        >
                            {move || {
                                if submitting.get() {
                                    "Recording...".to_string()
                                } else if auto_verify.get() {
                                    "Record & Verify".to_string()
                                } else {
                                    "Record Slip".to_string()
                                }
                            }}
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}
