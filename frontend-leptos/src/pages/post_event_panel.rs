//! Organizer control for post-event lead capture (Plan 008 — Phase 3 §3.3.1).
//!
//! `PUT /api/events/{id}/post-event-registration` and its API client have
//! existed since Phase 3, but nothing ever called the client — the flag could
//! only be set by hand-crafting an authenticated request. That left 13 recap
//! QR codes pointing at `/events/{slug}/post-event-register`, which returns 409
//! until the flag is on. This panel is the missing control.
//!
//! Deliberately *not* part of `EventForm`. The flag is written only by the
//! dedicated endpoint, which refuses to open it on a non-Completed event;
//! `UpdateEventRequest` has no such field, so routing it through the form save
//! would have created a second writer with no guard.

use leptos::prelude::*;

use crate::api::{self, PutPostEventRegistrationBody};
use crate::components::{self, ToastType};

/// Parse a `<input type="date">` value (`YYYY-MM-DD`) into end-of-day epoch ms.
///
/// End of day rather than midnight: an organizer setting "closes on the 30th"
/// means the 30th is still open, and midnight would close it a day early.
///
/// Local time, via `js_sys::Date` like the rest of this crate's date handling —
/// the organizer picked the date in their own timezone, so that is the day they
/// mean. (The crate has no `chrono`; the Worker owns the UTC arithmetic.)
fn deadline_to_ms(date: &str) -> Option<i64> {
    let parsed = js_sys::Date::parse(&format!("{date}T23:59:59"));
    match parsed.is_nan() {
        true => None,
        false => Some(parsed as i64),
    }
}

/// Render epoch ms back into the `YYYY-MM-DD` an `<input type="date">` wants.
fn ms_to_deadline(ms: i64) -> String {
    if ms == 0 {
        return String::new();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    format!(
        "{:04}-{:02}-{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date()
    )
}

/// Post-event registration toggle for a completed event.
///
/// `status` is read reactively from the form so the panel appears the moment
/// the organizer picks "Completed" in the status selector, rather than only
/// after a save — the two controls are used together.
#[component]
pub fn PostEventRegistrationPanel(
    #[prop(name = "set_toast")] set_toast: WriteSignal<Option<components::ToastMessage>>,
    #[prop(name = "event_id")] event_id: ReadSignal<Option<String>>,
    #[prop(name = "status")] status: Signal<api::EventStatus>,
) -> impl IntoView {
    let (is_open, set_is_open) = signal(false);
    let (deadline, set_deadline) = signal(String::new());
    let (saving, set_saving) = signal(false);
    let (loaded, set_loaded) = signal(false);

    // Seed from the server. The flag is not carried in `EventForm`, so this is
    // the only place the current value can come from.
    Effect::new(move |_| {
        let Some(id) = event_id.get() else {
            return;
        };
        leptos::task::spawn_local(async move {
            match api::get_event_detail(&id).await {
                Ok(data) => {
                    set_is_open.set(data.event.post_event_registration_open);
                    set_deadline.set(
                        data.event
                            .post_event_registration_until_ms
                            .map(ms_to_deadline)
                            .unwrap_or_default(),
                    );
                    set_loaded.set(true);
                }
                Err(e) => {
                    log::warn!("[post-event-panel] could not read current state: {e}");
                    set_loaded.set(true);
                }
            }
        });
    });

    let is_completed = move || status.get() == api::EventStatus::Completed;

    let save = move |open: bool| {
        let Some(id) = event_id.get() else {
            return;
        };
        // Mirror the server's own validation so the organizer is told before
        // the round trip rather than by a 400 afterwards.
        let until_ms = match (open, deadline.get()) {
            (true, d) if !d.is_empty() => match deadline_to_ms(&d) {
                Some(ms) if ms > js_sys::Date::now() as i64 => Some(ms),
                Some(_) => {
                    components::show_toast(
                        &set_toast,
                        "The closing date must be in the future.",
                        ToastType::Error,
                    );
                    return;
                }
                None => {
                    components::show_toast(
                        &set_toast,
                        "That closing date is not a valid date.",
                        ToastType::Error,
                    );
                    return;
                }
            },
            _ => None,
        };

        set_saving.set(true);
        let body = PutPostEventRegistrationBody { open, until_ms };
        leptos::task::spawn_local(async move {
            match api::put_post_event_registration(&id, &body).await {
                Ok(state) => {
                    set_is_open.set(state.open);
                    set_deadline.set(state.until_ms.map(ms_to_deadline).unwrap_or_default());
                    let message = match state.open {
                        true => {
                            "Post-event registration is open. The recap link now accepts sign-ups."
                        }
                        false => "Post-event registration closed.",
                    };
                    components::show_toast(&set_toast, message, ToastType::Success);
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Could not update post-event registration: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_saving.set(false);
        });
    };

    view! {
        <Show when=move || event_id.get().is_some() fallback=|| view! { <div></div> }>
            <div class="quiz-setting-item">
                <label class="quiz-field-label">"Post-event registration"</label>
                <Show
                    when=is_completed
                    fallback=|| {
                        view! {
                            <span class="quiz-setting-hint">
                                "Available once the event status is Completed — before that, \
                                 people register normally."
                            </span>
                        }
                    }
                >
                    <div class="flex-row-gap">
                        <button
                            type="button"
                            class=move || {
                                match is_open.get() {
                                    true => "btn btn-outline btn-sm",
                                    false => "btn btn-primary btn-sm",
                                }
                            }
                            disabled=move || saving.get() || !loaded.get()
                            on:click=move |_| save(!is_open.get())
                        >
                            {move || {
                                match (saving.get(), is_open.get()) {
                                    (true, _) => "Saving…",
                                    (false, true) => "Close registration",
                                    (false, false) => "Open registration",
                                }
                            }}
                        </button>
                        <input
                            type="date"
                            class="quiz-number-input"
                            prop:value=move || deadline.get()
                            disabled=move || saving.get()
                            on:change=move |ev| set_deadline.set(event_target_value(&ev))
                        />
                    </div>
                    <span class="quiz-setting-hint">
                        {move || {
                            match (is_open.get(), deadline.get().is_empty()) {
                                (true, true) => {
                                    "Open with no closing date. Attendees can join from the \
                                     recap link indefinitely."
                                        .to_string()
                                }
                                (true, false) => {
                                    format!(
                                        "Open until {} (end of that day, your local time). Set a date \
                                         and press the button again to change it.",
                                        deadline.get(),
                                    )
                                }
                                (false, _) => {
                                    "Closed. /events/{slug}/post-event-register returns 409 \
                                     until this is open."
                                        .to_string()
                                }
                            }
                        }}
                    </span>
                </Show>
            </div>
        </Show>
    }
}
