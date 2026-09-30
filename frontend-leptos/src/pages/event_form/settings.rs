//! Settings section, the format and visibility rows, and Capacity.

use leptos::prelude::*;

use super::ctx::FormCtx;
use super::section::{FormSection, SectionBadge};
use crate::api;

/// Settings, then the event format and visibility rows beside it.
#[component]
pub(super) fn SettingsSection(ctx: FormCtx) -> impl IntoView {
    let FormCtx {
        form,
        set_form,
        set_toast,
        editing_id,
        is_create,
        ..
    } = ctx;
    view! {
        <FormSection icon_class="form-section-icon-settings" title="Settings" badge=SectionBadge::Optional>
            <div class="quiz-settings-grid">
                <div class="quiz-setting-item">
                <label class="quiz-field-label">"Claim Base URL"<span class="field-optional-badge">"Auto"</span></label>
                <input
                    type="text"
                    class="quiz-number-input"
                    placeholder="Leave empty to auto-use current domain"
                    prop:value=move || form.get().claim_base_url
                    on:input=move |ev| set_form.update(|f| f.claim_base_url = event_target_value(&ev))
                />
                <span class="quiz-setting-hint">
                    "The base URL for attendee claim links (e.g. "
                    <code class="event-form-code-inherit">"https://bethere.solana-thailand.workers.dev/claim"</code>
                    "). Leave empty — the system auto-generates claim links from your current domain."
                </span>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Quiz Enabled"</label>
                    <label class="quiz-toggle-label event-form-toggle-label">
                        <input
                            type="checkbox"
                            class="quiz-toggle-checkbox"
                            prop:checked=move || form.get().quiz_enabled
                            on:change=move |ev| {
                                let checked = event_target_checked(&ev);
                                set_form.update(|f| f.quiz_enabled = checked);
                            }
                        />
                        <span class="quiz-toggle-switch"></span>
                        <span class="quiz-toggle-text">
                            {move || if form.get().quiz_enabled { "Yes" } else { "No" }}
                        </span>
                    </label>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Require Contact Info"</label>
                    <label class="quiz-toggle-label event-form-toggle-label">
                        <input
                            type="checkbox"
                            class="quiz-toggle-checkbox"
                            prop:checked=move || form.get().require_contact_info
                            on:change=move |ev| {
                                let checked = event_target_checked(&ev);
                                set_form.update(|f| f.require_contact_info = checked);
                            }
                        />
                        <span class="quiz-toggle-switch"></span>
                        <span class="quiz-toggle-text">
                            {move || if form.get().require_contact_info { "Yes" } else { "No" }}
                        </span>
                    </label>
                    <span class="quiz-setting-hint">
                        "When enabled, self-registration requires attendees to provide a contact channel (Telegram/Line/Facebook/X) and username. Disable for events that don't need it."
                    </span>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Require Photo Consent (PDPA)"</label>
                    <label class="quiz-toggle-label event-form-toggle-label">
                        <input
                            type="checkbox"
                            class="quiz-toggle-checkbox"
                            prop:checked=move || form.get().require_photo_consent
                            on:change=move |ev| {
                                let checked = event_target_checked(&ev);
                                set_form.update(|f| f.require_photo_consent = checked);
                            }
                        />
                        <span class="quiz-toggle-switch"></span>
                        <span class="quiz-toggle-text">
                            {move || if form.get().require_photo_consent { "Yes" } else { "No" }}
                        </span>
                    </label>
                    <span class="quiz-setting-hint">
                        "When enabled, attendees must consent to photo/video capture during the event. Required for Thai events with photography (PDPA compliance)."
                    </span>
                </div>
                // Status selector (edit only)
                <Show when=move || !is_create fallback=|| view! { <div></div> }>
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Status"</label>
                        <select
                            class="quiz-number-input"
                            on:change=move |ev| {
                                let val = event_target_value(&ev);
                                let status = match val.as_str() {
                                    "active" => api::EventStatus::Active,
                                    "completed" => api::EventStatus::Completed,
                                    _ => api::EventStatus::Draft,
                                };
                                set_form.update(|f| f.status = status);
                            }
                            prop:value=move || {
                                match form.get().status {
                                    api::EventStatus::Active => "active".to_string(),
                                    api::EventStatus::Completed => "completed".to_string(),
                                    api::EventStatus::Draft => "draft".to_string(),
                                    api::EventStatus::Archived => "archived".to_string(),
                                }
                            }
                        >
                            <option value="draft">"Draft"</option>
                            <option value="active">"Active"</option>
                            <option value="completed">"Completed"</option>
                        </select>
                    </div>
                </Show>
                // Post-event lead capture — sits beside Status because
                // the two are set together: an event is marked Completed
                // and then opened for retrospective sign-ups.
                <Show when=move || !is_create fallback=|| view! { <div></div> }>
                    <crate::pages::post_event_panel::PostEventRegistrationPanel
                        set_toast=set_toast
                        event_id=editing_id
                        status=Signal::derive(move || form.get().status)
                    />
                </Show>
            </div>
        </FormSection>

            // ── Event Format ──
            <div class="dep-config-row">
                <span class="dep-config-label">"Format"</span>
                <select
                    class="form-select form-select-sm"
                    on:change=move |ev| {
                        let val = event_target_value(&ev);
                        let fmt = match val.as_str() {
                            "online" => api::EventFormat::Online,
                            "hybrid" => api::EventFormat::Hybrid,
                            _ => api::EventFormat::InPerson,
                        };
                        set_form.update(|f| {
                            f.event_format = fmt.clone();
                            // Auto-sync deposit: in-person/hybrid = enabled
                            f.deposit_enabled = fmt.has_in_person();
                        });
                    }
                    prop:value=move || form.get().event_format.as_str()
                >
                    <option value="in_person">"In-Person"</option>
                    <option value="online">"Online"</option>
                    <option value="hybrid">"Hybrid"</option>
                </select>
            </div>
            // Format description
            <div class="dep-info-note">
                <p class="hint-note">
                    {move || match form.get().event_format {
                        api::EventFormat::InPerson => "Physical event with deposit commitment. Attendees get their deposit back when they attend.",
                        api::EventFormat::Online => "Virtual event. No deposit — quest completion serves as virtual check-in.",
                        api::EventFormat::Hybrid => "Both in-person and online tracks. In-person attendees deposit; online attendees complete quests.",
                    }}
                </p>
            </div>

            // ── Visibility ──
            <div class="dep-config-row">
                <span class="dep-config-label">"Visibility"</span>
                <div class="radio-group">
                    <label class="radio-label">
                        <input type="radio" name="visibility" value="public"
                            checked=move || form.get().visibility == api::EventVisibility::Public
                            on:change=move |_| set_form.update(|f| f.visibility = api::EventVisibility::Public)
                        />
                        <span>"🌐 Public"</span>
                        <span class="form-hint">" — visible on landing page, anyone can register"</span>
                    </label>
                    <label class="radio-label">
                        <input type="radio" name="visibility" value="private"
                            checked=move || form.get().visibility == api::EventVisibility::Private
                            on:change=move |_| set_form.update(|f| f.visibility = api::EventVisibility::Private)
                        />
                        <span>"🔒 Private"</span>
                        <span class="form-hint">" — hidden from landing, requires sign-in + access"</span>
                    </label>
                </div>
            </div>
    }
}

/// Capacity and registration control (in-person formats only).
#[component]
pub(super) fn CapacitySection(ctx: FormCtx) -> impl IntoView {
    let FormCtx { form, set_form, .. } = ctx;
    view! {
        <FormSection icon_class="form-section-icon-settings" title="Capacity & Registration Control" badge=SectionBadge::Optional>
                <div class="quiz-settings-grid">
                    // In-person capacity
                    <Show when=move || form.get().event_format != api::EventFormat::Online fallback=|| view! { <div></div> }>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"In-Person Capacity"<span class="field-optional-badge">"Unlimited if empty"</span></label>
                            <input
                                type="number"
                                class="quiz-number-input"
                                placeholder="e.g., 150"
                                min="1"
                                step="1"
                                prop:value=move || form.get().in_person_capacity
                                on:input=move |ev| set_form.update(|f| f.in_person_capacity = event_target_value(&ev))
                            />
                            <span class="quiz-setting-hint">"Maximum number of on-site attendees. Leave empty for unlimited. Includes walk-ins."</span>
                        </div>
                    </Show>
                    // Online capacity (hybrid only)
                    <Show when=move || form.get().event_format == api::EventFormat::Hybrid fallback=|| view! { <div></div> }>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Online Capacity"<span class="field-optional-badge">"Unlimited if empty"</span></label>
                            <input
                                type="number"
                                class="quiz-number-input"
                                placeholder="e.g., 500"
                                min="1"
                                step="1"
                                prop:value=move || form.get().online_capacity
                                on:input=move |ev| set_form.update(|f| f.online_capacity = event_target_value(&ev))
                            />
                            <span class="quiz-setting-hint">"Maximum online attendees. Leave empty for unlimited. Prevents NFT exhaustion for large events."</span>
                        </div>
                    </Show>
                    // Online open mode (hybrid only)
                    <Show when=move || form.get().event_format == api::EventFormat::Hybrid fallback=|| view! { <div></div> }>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Online Registration"</label>
                            <select
                                class="quiz-number-input"
                                on:change=move |ev| {
                                    let val = event_target_value(&ev);
                                    let mode = match val.as_str() {
                                        "auto_on_full" => api::OnlineOpenMode::AutoOnFull,
                                        "manual" => api::OnlineOpenMode::Manual,
                                        _ => api::OnlineOpenMode::Always,
                                    };
                                    set_form.update(|f| f.online_open_mode = mode);
                                }
                                prop:value=move || form.get().online_open_mode.as_str()
                            >
                                <option value="always">"Always Open"</option>
                                <option value="auto_on_full">"Auto (when in-person full)"</option>
                                <option value="manual">"Manual Toggle"</option>
                            </select>
                            <span class="quiz-setting-hint">
                                {move || match form.get().online_open_mode {
                                    api::OnlineOpenMode::Always => "Both in-person and online tracks open from the start.",
                                    api::OnlineOpenMode::AutoOnFull => "Online registration opens automatically when in-person capacity is reached.",
                                    api::OnlineOpenMode::Manual => "You control when online registration opens via the toggle below.",
                                }}
                            </span>
                        </div>
                    </Show>
                    // Manual toggle (only when Manual mode selected)
                    <Show when=move || form.get().online_open_mode == api::OnlineOpenMode::Manual && form.get().event_format == api::EventFormat::Hybrid fallback=|| view! { <div></div> }>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Online Registration Open"</label>
                            <label class="quiz-toggle-label event-form-toggle-label">
                                <input
                                    type="checkbox"
                                    class="quiz-toggle-checkbox"
                                    prop:checked=move || form.get().online_registration_open
                                    on:change=move |ev| {
                                        let checked = event_target_checked(&ev);
                                        set_form.update(|f| f.online_registration_open = checked);
                                    }
                                />
                                <span class="quiz-toggle-switch"></span>
                                <span class="quiz-toggle-text">
                                    {move || if form.get().online_registration_open { "Open" } else { "Closed" }}
                                </span>
                            </label>
                            <span class="quiz-setting-hint">"Toggle online registration on/off. Attendees see the online option only when this is enabled."</span>
                        </div>
                    </Show>
                    // Deposit deadline (in-person / hybrid only)
                    <Show when=move || form.get().deposit_enabled && form.get().event_format != api::EventFormat::Online fallback=|| view! { <div></div> }>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Deposit Deadline"<span class="field-optional-badge">"Hours"</span></label>
                            <input
                                type="number"
                                class="quiz-number-input"
                                placeholder="e.g., 24"
                                min="1"
                                step="1"
                                prop:value=move || form.get().deposit_deadline_hours
                                on:input=move |ev| set_form.update(|f| f.deposit_deadline_hours = event_target_value(&ev))
                            />
                            <span class="quiz-setting-hint">"Hours after registration to complete deposit. Attendees who miss the deadline are auto-switched to online track. Leave empty for no deadline."</span>
                        </div>
                    </Show>
                </div>
        </FormSection>

    }
}
