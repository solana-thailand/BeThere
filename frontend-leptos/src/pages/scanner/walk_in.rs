//! Walk-in registration on the scanner: the form, its handlers, and the
//! registering / capacity-override / success screens. Moved verbatim from
//! `page.rs` (.issues/052); names are unchanged so the diff is a move.

use leptos::prelude::*;

use crate::api::{self, WalkinRegisterRequest};
use crate::components::{self, ToastType};

use super::interop::*;
use super::state::*;
use super::views::*;

/// The walk-in form's three fields. `page.rs` owns them because its "Register
/// Walk-in" button clears them before opening the form.
#[derive(Clone, Copy)]
pub(super) struct WalkinFields {
    pub walkin_name: ReadSignal<String>,
    pub set_walkin_name: WriteSignal<String>,
    pub walkin_email: ReadSignal<String>,
    pub set_walkin_email: WriteSignal<String>,
    pub walkin_phone: ReadSignal<String>,
    pub set_walkin_phone: WriteSignal<String>,
}

impl WalkinFields {
    pub(super) fn new() -> Self {
        let (walkin_name, set_walkin_name) = signal(String::new());
        let (walkin_email, set_walkin_email) = signal(String::new());
        let (walkin_phone, set_walkin_phone) = signal(String::new());
        Self {
            walkin_name,
            set_walkin_name,
            walkin_email,
            set_walkin_email,
            walkin_phone,
            set_walkin_phone,
        }
    }
}

/// The walk-in screens inside the scanner's result overlay. Renders nothing
/// unless `check_in_state` is one of the four walk-in states.
#[allow(clippy::too_many_arguments)] // the signals the moved code already used; a struct would only rename them
pub(super) fn walkin_overlay<R>(
    fields: WalkinFields,
    check_in_state: ReadSignal<CheckInState>,
    set_check_in_state: WriteSignal<CheckInState>,
    set_scan_round: WriteSignal<u32>,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    active_event_id: ReadSignal<Option<String>>,
    handle_reset: R,
) -> impl IntoView
where
    R: Fn(web_sys::MouseEvent) + Clone + Copy + Send + Sync + 'static,
{
    let WalkinFields {
        walkin_name,
        set_walkin_name,
        walkin_email,
        set_walkin_email,
        walkin_phone,
        set_walkin_phone,
    } = fields;

    // Handler: cancel walk-in form → back to Idle
    let handle_walkin_cancel = move |_: web_sys::MouseEvent| {
        set_check_in_state.set(CheckInState::Idle);
        set_scan_round.update(|r| *r += 1);
    };

    // Handler: submit walk-in registration
    let handle_walkin_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let name = walkin_name.get().trim().to_string();
        let email = walkin_email.get().trim().to_string();
        let phone = walkin_phone.get().trim().to_string();

        if name.is_empty() || email.is_empty() {
            components::show_toast(
                &set_toast,
                "Name and email are required",
                ToastType::Warning,
            );
            return;
        }

        let event_id = match active_event_id.get() {
            Some(eid) if !eid.is_empty() => eid,
            _ => {
                components::show_toast(
                    &set_toast,
                    "No event selected. Select an event first.",
                    ToastType::Warning,
                );
                return;
            }
        };

        set_check_in_state.set(CheckInState::WalkinRegistering);

        let set_state = set_check_in_state;
        let set_t = set_toast;
        let name_for_callback = name.clone();
        let phone_for_warning = if phone.is_empty() {
            None
        } else {
            Some(phone.clone())
        };

        leptos::task::spawn_local(async move {
            let req = WalkinRegisterRequest {
                event_id,
                name: name.clone(),
                email: email.clone(),
                phone: if phone_for_warning.is_some() {
                    Some(phone_for_warning.clone().unwrap())
                } else {
                    None
                },
                override_capacity: false,
            };
            match api::register_walkin(&req).await {
                Ok(resp) => {
                    log::info!("[scanner] walk-in registered: {}", resp.claim_url);
                    feedback_success_js(); // Walk-in registered — vibration + beep
                    set_state.set(CheckInState::WalkinSuccess {
                        claim_url: resp.claim_url,
                        name: name_for_callback,
                    });
                    components::show_toast(
                        &set_t,
                        "Walk-in attendee registered!",
                        ToastType::Success,
                    );
                }
                Err(err) => {
                    let err_msg = err.message.clone();
                    if err_msg.starts_with("CAPACITY_REACHED") {
                        log::warn!("[scanner] walk-in capacity reached: {err_msg}");
                        set_state.set(CheckInState::WalkinCapacityWarning {
                            pending_name: name_for_callback,
                            pending_email: email.clone(),
                            pending_phone: phone_for_warning,
                        });
                    } else {
                        log::error!("[scanner] walk-in register failed: {err}");
                        set_state.set(CheckInState::WalkinForm);
                        components::show_toast(
                            &set_t,
                            &format!("Registration failed: {err}"),
                            ToastType::Error,
                        );
                    }
                }
            }
        });
    };

    // Handler: override capacity and register walk-in anyway
    let handle_walkin_override = move |_: web_sys::MouseEvent| {
        let state = check_in_state.get();
        let (pending_name, pending_email, pending_phone) = match &state {
            CheckInState::WalkinCapacityWarning {
                pending_name,
                pending_email,
                pending_phone,
            } => (
                pending_name.clone(),
                pending_email.clone(),
                pending_phone.clone(),
            ),
            _ => return,
        };

        let event_id = match active_event_id.get() {
            Some(eid) if !eid.is_empty() => eid,
            _ => return,
        };

        set_check_in_state.set(CheckInState::WalkinRegistering);

        let set_state = set_check_in_state;
        let set_t = set_toast;

        leptos::task::spawn_local(async move {
            let req = WalkinRegisterRequest {
                event_id,
                name: pending_name.clone(),
                email: pending_email.clone(),
                phone: pending_phone,
                override_capacity: true,
            };
            match api::register_walkin(&req).await {
                Ok(resp) => {
                    log::info!("[scanner] walk-in override registered: {}", resp.claim_url);
                    feedback_success_js();
                    set_state.set(CheckInState::WalkinSuccess {
                        claim_url: resp.claim_url,
                        name: pending_name,
                    });
                    components::show_toast(
                        &set_t,
                        "Walk-in registered (capacity override)",
                        ToastType::Success,
                    );
                }
                Err(err) => {
                    log::error!("[scanner] walk-in override failed: {err}");
                    set_state.set(CheckInState::WalkinForm);
                    components::show_toast(
                        &set_t,
                        &format!("Override failed: {err}"),
                        ToastType::Error,
                    );
                }
            }
        });
    };

    view! {
        // Walk-in registration form
        <Show
            when=move || matches!(check_in_state.get(), CheckInState::WalkinForm)
            fallback=|| view! { <div></div> }
        >
            <div>
                <div class="scanner-state-header">
                    <h2>"Register Walk-in Attendee"</h2>
                </div>
                <form on:submit=handle_walkin_submit>
                    <div class="scanner-walkin-form-fields">
                        <div>
                            <label class="scanner-walkin-label">"Name *"</label>
                            <input
                                type="text"
                                class="manual-input scanner-walkin-input"
                                placeholder="Attendee name"
                                required=true
                                prop:value=move || walkin_name.get()
                                on:input=move |ev| set_walkin_name.set(event_target_value(&ev))
                            />
                        </div>
                        <div>
                            <label class="scanner-walkin-label">"Email *"</label>
                            <input
                                type="email"
                                class="manual-input scanner-walkin-input"
                                placeholder="attendee@email.com"
                                required=true
                                prop:value=move || walkin_email.get()
                                on:input=move |ev| set_walkin_email.set(event_target_value(&ev))
                            />
                        </div>
                        <div>
                            <label class="scanner-walkin-label">"Phone (optional)"</label>
                            <input
                                type="tel"
                                class="manual-input scanner-walkin-input"
                                placeholder="+66..."
                                prop:value=move || walkin_phone.get()
                                on:input=move |ev| set_walkin_phone.set(event_target_value(&ev))
                            />
                        </div>
                    </div>
                    <button
                        class="btn btn-success btn-block"
                        type="submit"
                    >
                        "Register"
                    </button>
                    <button
                        class="btn btn-outline btn-block"
                        type="button"
                        on:click=handle_walkin_cancel
                    >
                        "Cancel"
                    </button>
                </form>
            </div>
        </Show>
        // Walk-in registering spinner
        <Show
            when=move || matches!(check_in_state.get(), CheckInState::WalkinRegistering)
            fallback=|| view! { <div></div> }
        >
            <div class="scanner-state-loading">
                <div class="page-loading">
                    <span class="spinner spinner-lg"></span>
                    <span>"Registering walk-in..."</span>
                </div>
            </div>
        </Show>
        // Walk-in capacity warning — show override dialog
        <Show
            when=move || matches!(check_in_state.get(), CheckInState::WalkinCapacityWarning { .. })
            fallback=|| view! { <div></div> }
        >
            {move || {
                let state = check_in_state.get();
                match state {
                    CheckInState::WalkinCapacityWarning { ref pending_name, .. } => {
                        let name_display = pending_name.clone();
                        view! {
                            <div class="scanner-capacity-card">
                                <div class="scanner-capacity-card-inner">
                                    <div class="scanner-capacity-icon">"⚠"</div>
                                    <h2 class="scanner-capacity-title">"In-Person Capacity Reached"</h2>
                                    <p class="scanner-capacity-desc">{name_display}" has reached the in-person capacity limit."</p>
                                </div>
                                <div class="scanner-capacity-actions">
                                    <button
                                        class="btn btn-success btn-block"
                                        on:click=handle_walkin_override
                                    >
                                        "Register Anyway (Override)"
                                    </button>
                                    <button
                                        class="btn btn-outline btn-block"
                                        on:click=handle_walkin_cancel
                                    >
                                        "Cancel"
                                    </button>
                                </div>
                            </div>
                        }.into_any()
                    }
                    _ => view! { <div></div> }.into_any(),
                }
            }}
        </Show>
        // Walk-in success — show claim QR
        <Show
            when=move || matches!(check_in_state.get(), CheckInState::WalkinSuccess { .. })
            fallback=|| view! { <div></div> }
        >
            {move || {
                let state = check_in_state.get();
                match state {
                    CheckInState::WalkinSuccess { ref claim_url, ref name } => {
                        let qr_data_url = generate_qr_data_url(claim_url, 240);
                        let claim_url_for_display = claim_url.clone();
                        let name_clone = name.clone();
                        view! {
                            <div>
                                <div class="result-success">
                                    <div class="success-check">
                                        <svg viewBox="0 0 24 24">
                                            <polyline points="20 6 9 17 4 12"></polyline>
                                        </svg>
                                    </div>
                                    <h2 class="claim-success-title">"Walk-in Registered!"</h2>
                                    <div class="result-details">
                                        <p class="scanner-attendee-name">{name_clone}</p>
                                    </div>
                                </div>
                                {move || {
                                    let url = claim_url_for_display.clone();
                                    match &qr_data_url {
                                        Some(img_src) => {
                                            view! {
                                                <ClaimQrCard
                                                    qr_src=img_src.clone()
                                                    claim_url=url
                                                    label="Show this QR to the attendee:"
                                                />
                                            }
                                                .into_any()
                                        }
                                        None => view! { <div></div> }.into_any(),
                                    }
                                }}
                                <button
                                    class="btn btn-success btn-block"
                                    on:click=handle_reset
                                >
                                    "Scan Another"
                                </button>
                            </div>
                        }
                            .into_any()
                    }
                    _ => view! { <div></div> }.into_any(),
                }
            }}
        </Show>
    }
}
