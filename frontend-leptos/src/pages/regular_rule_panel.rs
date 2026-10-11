//! Organizer control for the regular rule (Super GOAT SG1b). Off unless the
//! organizer turns it on: regulars with N door check-ins (their record, as
//! `/me/record` counts it) hold a seat without a deposit. Shows how many of
//! the current registrants qualify, before and after saving. Like the meals
//! panel, its own endpoint; a form save never writes it.

use leptos::prelude::*;

use event_checkin_domain::models::record::RegularRule;

use crate::components::{self, ToastType};

/// The threshold an organizer starts from when turning the rule on.
const DEFAULT_MIN: u32 = 3;

#[component]
pub fn RegularRulePanel(
    #[prop(name = "set_toast")] set_toast: WriteSignal<Option<components::ToastMessage>>,
    #[prop(name = "event_id")] event_id: ReadSignal<Option<String>>,
) -> impl IntoView {
    let saved = RwSignal::new(None::<RegularRule>);
    let min = RwSignal::new(DEFAULT_MIN);
    let qualifying = RwSignal::new(0u32);
    let busy = RwSignal::new(false);

    // The saved rule and its count.
    Effect::new(move |_| {
        let Some(id) = event_id.get() else {
            return;
        };
        leptos::task::spawn_local(async move {
            match crate::api::get_regular_rule(&id, None).await {
                Ok(r) => {
                    saved.set(r.rule);
                    if let Some(rule) = r.rule {
                        min.set(rule.min_door_checkins);
                    }
                    qualifying.set(r.qualifying_registrants);
                }
                Err(e) => log::warn!("[regular-rule] could not read the rule: {e}"),
            }
        });
    });

    // A preview count when the number changes.
    let preview = move |n: u32| {
        let Some(id) = event_id.get_untracked() else {
            return;
        };
        leptos::task::spawn_local(async move {
            if let Ok(r) = crate::api::get_regular_rule(&id, Some(n)).await {
                qualifying.set(r.qualifying_registrants);
            }
        });
    };

    let save = move |rule: Option<RegularRule>| {
        let Some(id) = event_id.get_untracked() else {
            return;
        };
        busy.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::put_regular_rule(&id, rule).await {
                Ok(r) => {
                    saved.set(r.rule);
                    qualifying.set(r.qualifying_registrants);
                    let msg = match r.rule {
                        Some(_) => "Regular rule saved.",
                        None => "Regular rule turned off. Everyone pays the normal deposit.",
                    };
                    components::show_toast(&set_toast, msg, ToastType::Success);
                }
                Err(e) => components::show_toast(
                    &set_toast,
                    &format!("Could not save the regular rule: {e}"),
                    ToastType::Error,
                ),
            }
            busy.set(false);
        });
    };

    view! {
        <div class="quiz-setting-item">
            <label class="quiz-field-label" for="regular-min">"Regulars without a deposit"</label>
            <div class="flex-row-gap">
                <input
                    id="regular-min"
                    type="number"
                    min="1"
                    max="100"
                    inputmode="numeric"
                    class="quiz-number-input"
                    prop:value=move || min.get().to_string()
                    disabled=move || busy.get()
                    on:change=move |ev| {
                        if let Ok(n) = event_target_value(&ev).trim().parse::<u32>()
                            && n > 0
                        {
                            min.set(n);
                            preview(n);
                        }
                    }
                />
                <button
                    type="button"
                    class="btn btn-outline btn-sm"
                    disabled=move || busy.get()
                    on:click=move |_| save(Some(RegularRule { min_door_checkins: min.get_untracked(), deposit_thb: 0 }))
                >
                    {move || match saved.get().is_some() {
                        true => "Update",
                        false => "Turn on",
                    }}
                </button>
                {move || saved.get().is_some().then(|| view! {
                    <button
                        type="button"
                        class="btn btn-outline btn-sm"
                        disabled=move || busy.get()
                        on:click=move |_| save(None)
                    >
                        "Turn off"
                    </button>
                })}
            </div>
            <span class="quiz-setting-hint">
                {move || {
                    let state = match saved.get() {
                        Some(r) => format!("On: {} or more door check-ins hold a seat without a deposit.", r.min_door_checkins),
                        None => "Off: everyone pays the normal deposit.".to_string(),
                    };
                    format!(
                        "{state} Door check-ins are counted across the community series, as on each person's record. {} of the current registrants have {} or more.",
                        qualifying.get(),
                        min.get(),
                    )
                }}
            </span>
        </div>
    }
}
