//! People and Ticket Announcements sections.

use event_checkin_domain::models::event::{MAX_POSTPONED_NOTE_CHARS, MAX_TICKET_NOTE_CHARS};
use leptos::prelude::*;

use super::ctx::FormCtx;
use super::section::{FormSection, SectionBadge};

/// Staff and admin emails.
#[component]
pub(super) fn PeopleSection(ctx: FormCtx) -> impl IntoView {
    let FormCtx { form, set_form, .. } = ctx;
    view! {
        <FormSection icon_class="form-section-icon-people" title="People" badge=SectionBadge::Optional>
                <div class="quiz-settings-grid">
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Organizer Emails"</label>
                    <textarea
                        class="quiz-textarea quiz-textarea-sm"
                        placeholder="admin@example.com, organizer@example.com"
                        prop:value=move || form.get().organizer_emails
                        on:input=move |ev| set_form.update(|f| f.organizer_emails = event_target_value(&ev))
                    ></textarea>
                    <span class="quiz-setting-hint">"Comma-separated"</span>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Staff Emails"</label>
                    <textarea
                        class="quiz-textarea quiz-textarea-sm"
                        placeholder="staff1@example.com, staff2@example.com"
                        prop:value=move || form.get().staff_emails
                        on:input=move |ev| set_form.update(|f| f.staff_emails = event_target_value(&ev))
                    ></textarea>
                    <span class="quiz-setting-hint">"Comma-separated"</span>
                </div>
            </div>
        </FormSection>

    }
}

/// Ticket announcements. Two boxes rather than one: the two audiences need
/// opposite things on the day, and each attendee is shown only the one that
/// matches how they are taking part.
#[component]
pub(super) fn AnnouncementsSection(ctx: FormCtx) -> impl IntoView {
    let FormCtx { form, set_form, .. } = ctx;
    view! {
        <FormSection icon_class="form-section-icon-community" title="Ticket Announcements" badge=SectionBadge::Optional open=false>
                <p class="quiz-setting-hint">
                    "Shown on each attendee's ticket page. Everyone sees only the box that matches how they are attending. Leave one empty to hide it for that audience. Links starting with http:// or https:// become clickable; line breaks are kept."
                </p>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"In-person attendees"</label>
                    <textarea
                        class="quiz-textarea"
                        maxlength=MAX_TICKET_NOTE_CHARS
                        placeholder="Doors open 12:30 at Building B. Free parking in the basement — tell the guard you are here for the meetup.\n\nSlides: https://example.com/deck\nJoin the group: https://example.com/invite"
                        prop:value=move || form.get().ticket_note_in_person
                        on:input=move |ev| set_form.update(|f| f.ticket_note_in_person = event_target_value(&ev))
                    ></textarea>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Online attendees"</label>
                    <textarea
                        class="quiz-textarea"
                        maxlength=MAX_TICKET_NOTE_CHARS
                        placeholder="The YouTube live link goes up about 30 minutes before we start.\n\nWatch: https://example.com/live\nClaim your badge from this page once the session begins."
                        prop:value=move || form.get().ticket_note_online
                        on:input=move |ev| set_form.update(|f| f.ticket_note_online = event_target_value(&ev))
                    ></textarea>
                </div>
                // Postponed notice (migration 0053). A note, not a
                // status: the event stays Active and keeps taking
                // registrations.
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Postponed notice"</label>
                    <p class="quiz-setting-hint">
                        "Leave empty unless the event is postponed. Shown as a banner on the event page and tickets."
                    </p>
                    <textarea
                        class="quiz-textarea"
                        maxlength=MAX_POSTPONED_NOTE_CHARS
                        placeholder="Postponed from 27 Sep because of flooding. New date: 4 Oct, same venue."
                        prop:value=move || form.get().postponed_note
                        on:input=move |ev| set_form.update(|f| f.postponed_note = event_target_value(&ev))
                    ></textarea>
                </div>
        </FormSection>

    }
}
