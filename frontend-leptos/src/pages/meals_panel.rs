//! Organizer control for meals ordered (.plans/045 R4.11). Like the
//! post-event panel, not part of `EventForm`: the count has its own endpoint
//! (`PUT /api/events/{id}/meals`) and its own table, so a form save never
//! writes it. Only an aggregate of it is public (`/api/public/stats`).

use leptos::prelude::*;

use crate::api::{self, MealsOrdered};
use crate::components::{self, ToastType};

/// `""` is none entered; anything else must be a whole number above 0.
fn parse_count(text: &str) -> Result<Option<u32>, ()> {
    match text.trim() {
        "" => Ok(None),
        t => t.parse::<u32>().ok().filter(|n| *n > 0).map(Some).ok_or(()),
    }
}

#[component]
pub fn MealsOrderedPanel(
    #[prop(name = "set_toast")] set_toast: WriteSignal<Option<components::ToastMessage>>,
    #[prop(name = "event_id")] event_id: ReadSignal<Option<String>>,
) -> impl IntoView {
    let (count, set_count) = signal(String::new());
    let (saving, set_saving) = signal(false);

    Effect::new(move |_| {
        let Some(id) = event_id.get() else {
            return;
        };
        leptos::task::spawn_local(async move {
            match api::get_meals_ordered(&id).await {
                Ok(m) => set_count.set(m.ordered.map(|n| n.to_string()).unwrap_or_default()),
                Err(e) => log::warn!("[meals-panel] could not read the count: {e}"),
            }
        });
    });

    let save = move |_| {
        let Some(id) = event_id.get() else {
            return;
        };
        let Ok(ordered) = parse_count(&count.get()) else {
            components::show_toast(
                &set_toast,
                "Meals ordered must be a whole number above 0, or empty.",
                ToastType::Error,
            );
            return;
        };
        set_saving.set(true);
        leptos::task::spawn_local(async move {
            match api::put_meals_ordered(&id, &MealsOrdered { ordered }).await {
                Ok(m) => {
                    set_count.set(m.ordered.map(|n| n.to_string()).unwrap_or_default());
                    components::show_toast(&set_toast, "Meals ordered saved.", ToastType::Success);
                }
                Err(e) => components::show_toast(
                    &set_toast,
                    &format!("Could not save meals ordered: {e}"),
                    ToastType::Error,
                ),
            }
            set_saving.set(false);
        });
    };

    view! {
        <div class="quiz-setting-item">
            <label class="quiz-field-label" for="meals-ordered">"Meals ordered"</label>
            <div class="flex-row-gap">
                <input
                    id="meals-ordered"
                    type="number"
                    min="1"
                    inputmode="numeric"
                    class="quiz-number-input"
                    prop:value=move || count.get()
                    disabled=move || saving.get()
                    on:input=move |ev| set_count.set(event_target_value(&ev))
                />
                <button
                    type="button"
                    class="btn btn-outline btn-sm"
                    disabled=move || saving.get()
                    on:click=save
                >
                    {move || match saving.get() {
                        true => "Saving…",
                        false => "Save",
                    }}
                </button>
            </div>
            <span class="quiz-setting-hint">
                "How many meals you ordered for the people on site. Only the total \
                 across events is public: registered, came, and the meals not ordered."
            </span>
        </div>
    }
}
