//! "Can you still come?" answers on the admin roster (migration 0052).
//!
//! After a postponement the organizer contacts every registrant and records
//! what they said: coming, not sure yet, or can't come. The picker sits on
//! each roster row; the filter bar narrows the roster to one answer (or to the
//! people nobody has an answer from yet) and shows how many are in each.

use event_checkin_domain::models::attendee::AttendanceAnswer;
use leptos::prelude::*;

use crate::api::{self, AttendeeListItem};
use crate::components::{self, ToastMessage, ToastType};

/// Roster filter by recorded answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerFilter {
    All,
    Answered(AttendanceAnswer),
    NoAnswer,
}

impl AnswerFilter {
    pub fn matches(self, attendee: &AttendeeListItem) -> bool {
        match self {
            Self::All => true,
            Self::Answered(want) => attendee.attendance_answer == Some(want),
            Self::NoAnswer => attendee.attendance_answer.is_none(),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "Any answer",
            Self::Answered(answer) => answer.label(),
            Self::NoAnswer => "No answer yet",
        }
    }
}

/// Every filter, in the order the bar shows them.
fn filters() -> [AnswerFilter; 5] {
    let [coming, undecided, not_coming] = AttendanceAnswer::ALL;
    [
        AnswerFilter::All,
        AnswerFilter::NoAnswer,
        AnswerFilter::Answered(coming),
        AnswerFilter::Answered(undecided),
        AnswerFilter::Answered(not_coming),
    ]
}

/// Filter pills with a count per answer, over the rows of the current tab.
#[component]
pub fn AnswerFilterBar(
    #[prop(into)] rows: Signal<Vec<AttendeeListItem>>,
    filter: ReadSignal<AnswerFilter>,
    set_filter: WriteSignal<AnswerFilter>,
) -> impl IntoView {
    view! {
        <div class="filter-pills" aria-label="Filter by attendance answer">
            {filters()
                .into_iter()
                .map(|f| {
                    let count = move || rows.with(|r| r.iter().filter(|a| f.matches(a)).count());
                    view! {
                        <button
                            class="filter-pill"
                            class:active=move || filter.get() == f
                            on:click=move |_| set_filter.set(f)
                        >
                            {f.label()} " (" {count} ")"
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// One roster row's answer picker. Saving refreshes the roster.
#[component]
pub fn AnswerPicker(
    attendee_id: String,
    event_id: Option<String>,
    current: Option<AttendanceAnswer>,
    set_toast: WriteSignal<Option<ToastMessage>>,
    set_refresh_counter: WriteSignal<u32>,
) -> impl IntoView {
    let (pending, set_pending) = signal(false);
    let selected = current.map_or("", AttendanceAnswer::as_str);

    let on_change = move |ev: leptos::ev::Event| {
        let raw = event_target_value(&ev);
        let answer = AttendanceAnswer::parse(&raw);
        let aid = attendee_id.clone();
        let eid = event_id.clone();
        set_pending.set(true);
        leptos::task::spawn_local(async move {
            match api::set_attendance_answer(&aid, eid.as_deref(), answer).await {
                Ok(_) => {
                    api::invalidate_attendee_cache();
                    set_refresh_counter.update(|c| *c += 1);
                    components::show_toast(
                        &set_toast,
                        answer.map_or("Answer cleared", AttendanceAnswer::label),
                        ToastType::Success,
                    );
                }
                Err(e) => components::show_toast(
                    &set_toast,
                    &format!("Could not save the answer: {e}"),
                    ToastType::Error,
                ),
            }
            set_pending.set(false);
        });
    };

    view! {
        <select
            class="admin-refund-form-select admin-answer-picker"
            title="What did they answer when asked if they can still come?"
            disabled=move || pending.get()
            on:change=on_change
        >
            <option value="" selected=selected.is_empty()>"Answer…"</option>
            {AttendanceAnswer::ALL
                .into_iter()
                .map(|a| view! {
                    <option value=a.as_str() selected=selected == a.as_str()>{a.label()}</option>
                })
                .collect_view()}
        </select>
    }
}
