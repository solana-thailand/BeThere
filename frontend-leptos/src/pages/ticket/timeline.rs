//! Progress timeline component for online attendee "What's Next?" section.

use leptos::prelude::*;

use crate::i18n::{t, use_i18n};

/// A single timeline step.
///
/// Text fields are views so catalog text follows a language switch.
#[derive(Clone)]
pub struct TimelineStep {
    /// Whether this step is completed
    pub done: bool,
    /// Step number (for display when not done)
    pub number: u32,
    /// Step title
    pub title: ViewFn,
    /// Step description
    pub desc: ViewFn,
    /// Optional action link (href, label)
    pub link: Option<(String, ViewFn)>,
}

/// Progress timeline for online attendees — shows "What's Next?" steps.
#[component]
pub fn Timeline(
    /// Ordered list of timeline steps to render
    #[prop(into)]
    steps: Vec<TimelineStep>,
) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="ticket-timeline">
            <h3 class="ticket-timeline-heading">{t!(i18n, ticket.timeline.heading)}</h3>
            <div class="ticket-timeline-steps">
                {steps.into_iter().map(|step| {
                    let dot_class = if step.done {
                        "ticket-timeline-dot ticket-timeline-dot--done"
                    } else {
                        "ticket-timeline-dot ticket-timeline-dot--pending"
                    };
                    let dot_content = if step.done {
                        "✓".to_string()
                    } else {
                        step.number.to_string()
                    };
                    view! {
                        <div class="ticket-timeline-step">
                            <div class=dot_class>{dot_content}</div>
                            <div class="ticket-timeline-content">
                                <div class="ticket-timeline-title">{step.title.run()}</div>
                                <div class="ticket-timeline-desc">{step.desc.run()}</div>
                                {if let Some((href, label)) = step.link {
                                    view! {
                                        <a
                                            href=href
                                            class="ticket-timeline-link"
                                        >
                                            {label.run()}
                                        </a>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
