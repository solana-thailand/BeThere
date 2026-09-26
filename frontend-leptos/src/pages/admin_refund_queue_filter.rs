//! Refund Queue filter: after a postponement, the rows that are really owed
//! money back are the people who moved online or answered "can't come". The
//! queue lists every verified cash deposit, so without this the organizer
//! picks them out by memory.

use std::collections::HashMap;

use event_checkin_domain::models::attendee::{AttendanceAnswer, ParticipationType};
use event_checkin_domain::models::deposit::RefundQueueContext;
use leptos::prelude::*;

use crate::api::ThbDepositInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefundQueueFilter {
    All,
    /// Participation is Online now (typically switched after a postponement).
    Online,
    /// Answered "can't come" when asked.
    NotComing,
    /// Not checked in: nobody scanned them at the door.
    NotCheckedIn,
}

impl RefundQueueFilter {
    const ALL: [Self; 4] = [Self::All, Self::Online, Self::NotComing, Self::NotCheckedIn];

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Online => "Moved online",
            Self::NotComing => "Can't come",
            Self::NotCheckedIn => "Not checked in",
        }
    }

    /// A row with no context (D1 read failed) only passes `All`: the
    /// narrowing filters must not guess who is owed money.
    pub fn matches(self, context: Option<&RefundQueueContext>) -> bool {
        match (self, context) {
            (Self::All, _) => true,
            (_, None) => false,
            (Self::Online, Some(c)) => c.participation_type == ParticipationType::Online,
            (Self::NotComing, Some(c)) => c.attendance_answer == Some(AttendanceAnswer::NotComing),
            (Self::NotCheckedIn, Some(c)) => !c.checked_in,
        }
    }
}

/// Filter pills with a count per filter over the whole queue.
#[component]
pub fn RefundQueueFilterBar(
    #[prop(into)] refunds: Signal<Vec<ThbDepositInfo>>,
    #[prop(into)] context: Signal<HashMap<String, RefundQueueContext>>,
    filter: ReadSignal<RefundQueueFilter>,
    set_filter: WriteSignal<RefundQueueFilter>,
) -> impl IntoView {
    view! {
        <div class="filter-pills" aria-label="Filter the refund queue">
            {RefundQueueFilter::ALL
                .into_iter()
                .map(|f| {
                    let count = move || {
                        context.with(|ctx| {
                            refunds.with(|rows| {
                                rows.iter().filter(|d| f.matches(ctx.get(&d.attendee_id))).count()
                            })
                        })
                    };
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
