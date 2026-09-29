//! Basic Info and Schedule sections.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use super::ctx::FormCtx;
use super::section::{FormSection, SectionBadge};
use super::types::{format_datetime_local, generate_slug, parse_date_to_ms};
use crate::components;

/// Name, slug, tagline, description, location and link.
#[component]
pub(super) fn BasicSection(ctx: FormCtx) -> impl IntoView {
    let FormCtx {
        form,
        set_form,
        set_toast,
        editing_id,
        events,
        slug_taken,
        set_slug_taken,
        ..
    } = ctx;
    let (slug_manually_edited, set_slug_manually_edited) = signal(false);
    // Handle name input (auto-generate slug if not manually edited)
    let handle_name_input = move |ev| {
        let name = event_target_value(&ev);
        let edited = slug_manually_edited.get();
        set_form.update(|f| {
            f.name = name.clone();
            if !edited {
                f.slug = generate_slug(&name);
            }
        });
    };

    // Handle slug input
    let handle_slug_input = move |ev| {
        set_slug_manually_edited.set(true);
        let val = event_target_value(&ev);
        set_form.update(|f| f.slug = val);
        set_slug_taken.set(false);
    };

    // Check slug availability on blur (client-side check against loaded events)
    let handle_slug_blur = move |_| {
        let current_slug = form.get().slug.trim().to_lowercase();
        if current_slug.is_empty() {
            set_slug_taken.set(false);
            return;
        }
        let editing = editing_id.get().unwrap_or_default();
        let taken = events
            .get()
            .iter()
            .any(|e| e.slug.to_lowercase() == current_slug && e.id != editing);
        set_slug_taken.set(taken);
    };

    view! {
        <FormSection icon_class="form-section-icon-basic" title="Basic Info" badge=SectionBadge::Required>
            <div class="quiz-settings-grid">
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Name"<span class="field-required-badge">"Required"</span></label>
                    <input
                        type="text"
                        class="quiz-number-input"
                        placeholder="Event Name"
                        prop:value=move || form.get().name
                        on:input=handle_name_input
                    />
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Slug"<span class="field-required-badge">"Required"</span></label>
                    <input
                        type="text"
                        class="quiz-number-input"
                        placeholder="event-slug"
                        prop:value=move || form.get().slug
                        on:input=handle_slug_input
                        on:focusout=handle_slug_blur
                    />
                    <Show
                        when=move || slug_taken.get()
                        fallback=|| view! { <div></div> }
                    >
                        <div class="hint-warning-xs">
                            "This slug is already taken by another event"
                        </div>
                    </Show>
                    <span class="quiz-setting-hint">"Auto-generated from name"</span>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Tagline"<span class="field-optional-badge">"Optional"</span></label>
                    <input
                        type="text"
                        class="quiz-number-input"
                        placeholder="A short description"
                        prop:value=move || form.get().tagline
                        on:input=move |ev| set_form.update(|f| f.tagline = event_target_value(&ev))
                    />
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">
                        "Description"<span class="field-optional-badge">"Optional"</span>
                    </label>
                    <textarea
                        class="quiz-textarea"
                        rows="8"
                        placeholder="Agenda, schedule, what to bring, links…&#10;&#10;13:00  Registration&#10;13:30  Keynote&#10;15:00  Workshop"
                        prop:value=move || form.get().description
                        on:input=move |ev| set_form.update(|f| f.description = event_target_value(&ev))
                    ></textarea>
                    <span class="quiz-setting-hint">
                        "Shown publicly under \"About this Event\". Line breaks are preserved, so an agenda can be pasted straight in."
                    </span>
                    <label class="quiz-field-label" style="margin-top: 0.5rem;">
                        "Import an Ontime rundown"
                    </label>
                    <input
                        type="file"
                        accept=".csv,text/csv"
                        on:change=move |ev| {
                            // Parsed entirely in the browser — the file is
                            // never uploaded. The organizer reviews (and can
                            // edit) the generated text before saving.
                            let Some(target) = ev.target() else { return };
                            let input: web_sys::HtmlInputElement = target.unchecked_into();
                            let Some(file) = input.files().and_then(|fl| fl.item(0)) else {
                                return;
                            };
                            // Allow re-selecting the same file after a failed
                            // parse; without this `change` never fires again.
                            input.set_value("");
                            leptos::task::spawn_local(async move {
                                let text = match wasm_bindgen_futures::JsFuture::from(file.text()).await {
                                    Ok(v) => v.as_string().unwrap_or_default(),
                                    Err(_) => {
                                        components::show_toast(
                                            &set_toast,
                                            "Could not read that file",
                                            components::ToastType::Error,
                                        );
                                        return;
                                    }
                                };
                                use event_checkin_domain::models::rundown;
                                match rundown::parse_ontime_csv(&text) {
                                    Ok(sessions) => {
                                        let count = sessions.len();
                                        let agenda = rundown::to_agenda_text(&sessions);
                                        set_form.update(|f| f.description = agenda);
                                        components::show_toast(
                                            &set_toast,
                                            &format!(
                                                "Imported {count} sessions — review below, then Save"
                                            ),
                                            components::ToastType::Success,
                                        );
                                    }
                                    Err(e) => {
                                        // Leave the textarea untouched on failure.
                                        components::show_toast(
                                            &set_toast,
                                            &format!("Could not import rundown: {e}"),
                                            components::ToastType::Error,
                                        );
                                    }
                                }
                            });
                        }
                    />
                    <span class="quiz-setting-hint">
                        "Export the rundown from Ontime as CSV. Skipped rows are left out; run-of-show notes and colours are not published."
                    </span>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Link"<span class="field-optional-badge">"Optional"</span></label>
                    <input
                        type="text"
                        class="quiz-number-input"
                        placeholder="https://example.com"
                        prop:value=move || form.get().link
                        on:input=move |ev| set_form.update(|f| f.link = event_target_value(&ev))
                    />
                </div>
            </div>
        </FormSection>

    }
}

/// Start and end times.
#[component]
pub(super) fn ScheduleSection(ctx: FormCtx) -> impl IntoView {
    let FormCtx { form, set_form, .. } = ctx;
    view! {
        <FormSection icon_class="form-section-icon-schedule" title="Schedule" badge=SectionBadge::Required>
            <div class="quiz-settings-grid">
            <div class="quiz-setting-item">
                <label class="quiz-field-label">"Event Start"<span class="field-required-badge">"Required"</span></label>
                <input
                    type="datetime-local"
                    class="quiz-number-input"
                    prop:value=move || format_datetime_local(parse_date_to_ms(&form.get().event_start).unwrap_or(0))
                    on:input=move |ev| {
                        let val = event_target_value(&ev);
                        let ms = parse_date_to_ms(&val).unwrap_or(0);
                        set_form.update(|f| f.event_start = if ms > 0 { format_datetime_local(ms) } else { val });
                    }
                />
                <span class="quiz-setting-hint">"Times in your local timezone"</span>
            </div>
            <div class="quiz-setting-item">
                <label class="quiz-field-label">"Event End"<span class="field-required-badge">"Required"</span></label>
                <input
                    type="datetime-local"
                    class="quiz-number-input"
                    prop:value=move || format_datetime_local(parse_date_to_ms(&form.get().event_end).unwrap_or(0))
                    on:input=move |ev| {
                        let val = event_target_value(&ev);
                        let ms = parse_date_to_ms(&val).unwrap_or(0);
                        set_form.update(|f| f.event_end = if ms > 0 { format_datetime_local(ms) } else { val });
                    }
                />
                <span class="quiz-setting-hint">"Times in your local timezone"</span>
            </div>
            <div class="quiz-setting-item event-form-span-full">
                <label class="event-form-tba-label">
                    <input
                        type="checkbox"
                        prop:checked=move || form.get().time_tba
                        on:change=move |ev| set_form.update(|f| f.time_tba = event_target_checked(&ev))
                    />
                    <span class="quiz-field-label event-form-tba-field-label">"Time TBA (To Be Announced)"</span>
                    <span class="form-hint event-form-tba-hint">"Show \"TBA\" instead of specific time"</span>
                </label>
            </div>
            <Show
                when=move || {
                    let start = parse_date_to_ms(&form.get().event_start).unwrap_or(0);
                    let end = parse_date_to_ms(&form.get().event_end).unwrap_or(0);
                    start > 0 && end > 0 && end <= start
                }
                fallback=|| view! { <div></div> }
            >
                <div class="hint-warning-xs">
                    "Event end must be after event start"
                </div>
            </Show>
            <div class="quiz-setting-item event-form-span-full">
                <label class="quiz-field-label">"Location"<span class="field-optional-badge">"Optional"</span></label>
                <input
                    type="text"
                    class="quiz-number-input"
                    placeholder="e.g. Impact Exhibition Center, Bangkok"
                    prop:value=move || form.get().location
                    on:input=move |ev| set_form.update(|f| f.location = event_target_value(&ev))
                />
                <span class="quiz-setting-hint">"Venue name and address for in-person events"</span>
            </div>
            <div class="quiz-setting-item event-form-span-full">
                <label class="quiz-field-label">"Google Maps Link"<span class="field-optional-badge">"Optional"</span></label>
                <input
                    type="url"
                    class="quiz-number-input"
                    placeholder="e.g. https://maps.app.goo.gl/abc123"
                    prop:value=move || form.get().location_map_url
                    on:input=move |ev| set_form.update(|f| f.location_map_url = event_target_value(&ev))
                />
                <span class="quiz-setting-hint">"Google Maps → Share → Copy link. Attendees tap the location to open the map"</span>
            </div>
            <div class="quiz-setting-item event-form-span-full">
                <label class="quiz-field-label">"Guests who don\'t pay a deposit"<span class="field-optional-badge">"Optional"</span></label>
                <textarea
                    class="quiz-number-input"
                    rows="3"
                    placeholder="speaker@example.com\nsponsor@example.com"
                    prop:value=move || form.get().comp_emails
                    on:input=move |ev| set_form.update(|f| f.comp_emails = event_target_value(&ev))
                ></textarea>
                <span class="quiz-setting-hint">
                    "One email per line (or comma-separated). When they register, the deposit is waived automatically and no refund is owed — they still get a ticket QR. Case doesn\'t matter. This grants no admin access."
                </span>
            </div>
            <div class="quiz-setting-item event-form-span-full">
                <label class="quiz-field-label">"Video / Livestream URL"<span class="field-optional-badge">"Optional"</span></label>
                <input
                    type="url"
                    class="quiz-number-input"
                    placeholder="e.g. https://youtube.com/live/abc123"
                    prop:value=move || form.get().video_url
                    on:input=move |ev| set_form.update(|f| f.video_url = event_target_value(&ev))
                />
                <span class="quiz-setting-hint">"YouTube livestream or recording link — shown to attendees on the event page"</span>
            </div>
            <div class="quiz-setting-item event-form-span-full">
                <label class="quiz-field-label">"Calendar Subscribe URL"<span class="field-optional-badge">"Optional"</span></label>
                <input
                    type="url"
                    class="quiz-number-input"
                    placeholder="e.g. https://calendar.google.com/calendar/embed?src=..."
                    prop:value=move || form.get().calendar_subscribe_url
                    on:input=move |ev| set_form.update(|f| f.calendar_subscribe_url = event_target_value(&ev))
                />
                <span class="quiz-setting-hint">"Google Calendar embed URL — shown as 'Our Event Calendar' on ticket page"</span>
            </div>
        </div>
        </FormSection>

    }
}
