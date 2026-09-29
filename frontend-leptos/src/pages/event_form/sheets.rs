//! Google Sheets section.

use leptos::prelude::*;

use super::ctx::FormCtx;
use super::section::{FormSection, SectionBadge};
use crate::icons::{Icon, IconName};

/// Attendee and staff sheet ids.
#[component]
pub(super) fn SheetsSection(ctx: FormCtx) -> impl IntoView {
    let FormCtx { form, set_form, .. } = ctx;
    view! {
        <FormSection icon_class="form-section-icon-sheets" title="Google Sheets" badge=SectionBadge::Required>
            // ── Quick Guide: How to get Google Sheet ID ──
            <div class="sheet-guide-box event-form-sheet-guide">
                <div class="sheet-guide-heading"><Icon icon=IconName::Copy class="icon-sm"/>" Quick Guide"</div>
                <ol class="sheet-guide-steps">
                    <li>
                        <a
                            href="https://docs.google.com/forms/create"
                            target="_blank"
                            rel="noopener"
                            class="sheet-guide-link"
                        >
                            "Create a Google Form"
                        </a>
                        " → add your registration fields"
                    </li>
                    <li>"In the Form editor → Responses tab → click 'Link to Sheets'"</li>
                    <li>"Copy the Sheet ID from the URL:"</li>
                </ol>
                <div class="sheet-guide-url">
                    <code>"docs.google.com/spreadsheets/d/"</code>
                    <code class="sheet-guide-highlight">"YOUR_SHEET_ID"</code>
                    <code>"/edit"</code>
                </div>
            </div>

            <div class="quiz-settings-grid">
                <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Sheet ID"<span class="field-required-badge">"Required"</span></label>
                    <input
                        type="text"
                        class="quiz-number-input"
                        placeholder="Paste Google Sheet ID here"
                        prop:value=move || form.get().sheet_id
                        on:input=move |ev| set_form.update(|f| f.sheet_id = event_target_value(&ev))
                    />
                    <div class="quiz-setting-hint event-form-hint-row">
                        <Show
                            when=move || !form.get().sheet_id.trim().is_empty()
                            fallback=|| view! { <span></span> }
                        >
                            <a
                                href=move || crate::utils::google_sheet_url(&form.get().sheet_id)
                                target="_blank"
                                rel="noopener noreferrer"
                                class="event-form-link-sm"
                            >
                                "Open in Google Sheets ↗"
                            </a>
                        </Show>
                    </div>
                </div>
            <div class="quiz-setting-item">
                <label class="quiz-field-label">"Sheet Name"</label>
                <input
                    type="text"
                    class="quiz-number-input"
                    placeholder="Attendees"
                    prop:value=move || form.get().sheet_name
                    on:input=move |ev| set_form.update(|f| f.sheet_name = event_target_value(&ev))
                />
            </div>
            <div class="quiz-setting-item">
                <label class="quiz-field-label">"Staff Sheet Name"</label>
                <input
                    type="text"
                    class="quiz-number-input"
                    placeholder="staff"
                    prop:value=move || form.get().staff_sheet_name
                    on:input=move |ev| set_form.update(|f| f.staff_sheet_name = event_target_value(&ev))
                />
            </div>
        </div>
        </FormSection>

    }
}
