//! The scanner's event selector bar: a dropdown when several events are
//! active, the name when one is, a warning when none is. Moved verbatim from
//! `page.rs` (.issues/052).

use leptos::prelude::*;

use crate::api;
use crate::icons::{Icon, IconName};

pub(super) fn event_bar(
    events_list: ReadSignal<Vec<api::EventMeta>>,
    events_loading: ReadSignal<bool>,
    active_event_id: ReadSignal<Option<String>>,
    set_active_event_id: WriteSignal<Option<String>>,
) -> impl IntoView {
    move || {
        let all_events = events_list.get();
        let active_events: Vec<_> = all_events
            .iter()
            .filter(|e| e.status == api::EventStatus::Active)
            .collect();

        if events_loading.get() {
            view! {
                <div class="scanner-event-bar">
                    <span class="scanner-event-label">"Loading events…"</span>
                </div>
            }
            .into_any()
        } else if active_events.is_empty() {
            view! {
                <div class="scanner-event-bar scanner-event-bar--danger">
                    <span class="scanner-event-label scanner-event-label--danger"><Icon icon=IconName::Warning class="icon-sm icon-warning" />" No active events found"</span>
                </div>
            }.into_any()
        } else if active_events.len() == 1 {
            // Only one active event — no need for dropdown, but show name
            let name = active_events[0].name.clone();
            view! {
                <div class="scanner-event-bar">
                    <span class="scanner-event-label">"Event:"</span>
                    <span class="scanner-event-name">{name}</span>
                </div>
            }
            .into_any()
        } else {
            // Multiple active events — show dropdown
            let options = active_events.clone();
            view! {
                <div class="scanner-event-bar">
                    <label class="scanner-event-label" for="scanner-event-select">"Event:"</label>
                    <select
                        id="scanner-event-select"
                        class="scanner-event-select"
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            set_active_event_id.set(
                                if val.is_empty() { None } else { Some(val) }
                            );
                        }
                        prop:value=move || active_event_id.get().unwrap_or_default()
                    >
                        {options.into_iter().map(|e| {
                            let id = e.id.clone();
                            let name = e.name.clone();
                            view! {
                                <option value=id>{name}</option>
                            }
                        }).collect::<Vec<_>>()}
                    </select>
                </div>
            }
            .into_any()
        }
    }
}
