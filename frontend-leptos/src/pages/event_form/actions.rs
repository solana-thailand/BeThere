//! Save / Cancel / Archive buttons.

use leptos::prelude::*;

use super::ctx::FormCtx;
use super::save::save_event;
use super::types::OnDone;
use crate::{api, components};

/// The form action row.
#[component]
pub(super) fn FormActions(ctx: FormCtx, on_done: OnDone) -> impl IntoView {
    let FormCtx {
        form,
        set_toast,
        editing_id,
        is_create,
        saving,
        create_wallet_pk,
        ..
    } = ctx;
    let on_done_save = on_done.clone();
    let handle_save = move |_: web_sys::MouseEvent| save_event(ctx, on_done_save.clone());
    // Wrap on_done in store_value so the view! macro's Fn closure can clone it
    // without moving the original Arc.
    let stored_on_done = StoredValue::new(on_done);

    view! {
        <div class="form-actions-row">
            <button
                class="btn btn-primary"
                on:click=handle_save
                disabled=move || saving.get()
            >
                {move || {
                    if saving.get() {
                        "Saving...".to_string()
                    } else if !is_create {
                        "Update Event".to_string()
                    } else if !create_wallet_pk.get().is_empty() && form.get().deposit_enabled {
                        "Create Event + Initialize Escrow".to_string()
                    } else {
                        "Create Event".to_string()
                    }
                }}
            </button>
            <button class="btn btn-outline" on:click=move |_| { stored_on_done.get_value()(); }>
                "Cancel"
            </button>
            <Show
                when=move || {
                    !is_create && !editing_id.get().unwrap_or_default().is_empty()
                }
                fallback=|| view! { <div></div> }
            >
                <button
                    class="btn btn-outline btn-archive"
                    on:click=move |_| {
                        let aid = editing_id.get().unwrap_or_default();
                        let set_toast = set_toast;
                        let on_done_ref = stored_on_done.get_value().clone();
                        leptos::task::spawn_local(async move {
                            match api::archive_event(&aid).await {
                                Ok(data) => {
                                    components::show_mutation_toast(
                                        &set_toast,
                                        &format!("Event '{}' archived", data.name),
                                        &data.warnings,
                                    );
                                    on_done_ref();
                                }
                                Err(e) => {
                                    log::error!("[event-form] archive failed: {e}");
                                    components::show_toast(
                                        &set_toast,
                                        &format!("Failed to archive: {e}"),
                                        components::ToastType::Error,
                                    );
                                }
                            }
                        });
                    }
                >
                    "Archive Event"
                </button>
            </Show>
        </div>
    }
}
