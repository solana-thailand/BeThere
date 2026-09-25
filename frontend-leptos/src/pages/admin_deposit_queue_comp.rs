//! Refund Queue "Comp — no refund owed" action.
//!
//! An approved deposit sits in the refund queue as ฿ the organizer has promised
//! back. When it turns out nothing was really sent (a staff member made to pay
//! by an old rule, a slip approved by mistake), the organizer needs to write it
//! off without refunding. `POST /api/deposit/thb/comp` already does that for
//! pending slips (`.issues/129`); this is the same call from the queue, with a
//! reason the server requires for an approved deposit and keeps in the audit log.

use leptos::prelude::*;

use crate::api::{self, CompDepositRequest};
use crate::components::{self, ToastMessage, ToastType};

/// One Refund Queue row's comp control: a button that opens a reason field,
/// and a confirm that stays disabled until a reason is typed.
#[component]
pub fn QueueCompAction(
    event_id: String,
    attendee_id: String,
    set_toast: WriteSignal<Option<ToastMessage>>,
    set_refresh_counter: WriteSignal<u32>,
) -> impl IntoView {
    let (open, set_open) = signal(false);
    let (reason, set_reason) = signal(String::new());
    let (pending, set_pending) = signal(false);

    let submit = move |_| {
        let why = reason.get_untracked().trim().to_string();
        if why.is_empty() {
            components::show_toast(
                &set_toast,
                "A reason is required to write off an approved deposit",
                ToastType::Error,
            );
            return;
        }
        set_pending.set(true);
        let body = CompDepositRequest {
            event_id: event_id.clone(),
            attendee_id: attendee_id.clone(),
            reason: Some(why),
        };
        leptos::task::spawn_local(async move {
            match api::comp_thb_deposit(&body).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        "Written off as comp — no refund owed",
                        ToastType::Success,
                    );
                    set_open.set(false);
                    set_reason.set(String::new());
                    set_refresh_counter.update(|c| *c = c.wrapping_add(1));
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to write off: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_pending.set(false);
        });
    };

    view! {
        <div class="admin-dep-hold-row">
            <Show
                when=move || open.get()
                fallback=move || view! {
                    <button
                        class="btn btn-outline btn-sm"
                        title="Nothing was really received (e.g. staff, or a slip approved by mistake): admit without owing a refund"
                        on:click=move |_| set_open.set(true)
                    >
                        "Comp — no refund owed"
                    </button>
                }
            >
                <div style="display:flex;flex-direction:column;gap:0.25rem">
                    <input
                        type="text"
                        class="form-input dep-input"
                        placeholder="Reason (required, kept in the audit log)"
                        prop:value=move || reason.get()
                        on:input=move |ev| set_reason.set(event_target_value(&ev))
                    />
                    <div class="admin-dep-confirm-row">
                        <button
                            class="btn btn-primary btn-sm"
                            disabled=move || pending.get() || reason.get().trim().is_empty()
                            on:click=submit.clone()
                        >
                            {move || if pending.get() { "Writing off..." } else { "Confirm comp" }}
                        </button>
                        <button
                            class="btn btn-danger btn-sm"
                            disabled=move || pending.get()
                            on:click=move |_| set_open.set(false)
                        >
                            "Cancel"
                        </button>
                    </div>
                </div>
            </Show>
        </div>
    }
}
