//! Event Poster section (marketing hero image).

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use super::ctx::FormCtx;
use super::section::{FormSection, SectionBadge};
use crate::{api, components};

/// Poster upload and preview.
#[component]
pub(super) fn PosterSection(ctx: FormCtx) -> impl IntoView {
    let FormCtx {
        form,
        set_form,
        set_toast,
        editing_id,
        poster_busy,
        set_poster_busy,
        ..
    } = ctx;
    view! {
        <FormSection icon_class="form-section-icon-nft" title="Event Poster" badge=SectionBadge::Optional>
            <div class="quiz-setting-item event-form-span-full">
                <div class="hint-info event-form-nft-intro">
                    "Shown at the top of the public event page (/e/{{slug}}). Falls back to the NFT badge if not set."
                </div>
                // Nudge (.plans/038 P3-b): what a poster buys on Discover.
                <Show when=move || form.get().poster_url.is_empty() fallback=|| ()>
                    <div class="hint-info event-form-poster-nudge">
                        "Add a poster: events with one show as a card on Discover instead of a plain row."
                    </div>
                </Show>
                // Live preview — mirrors the badge preview pattern
                <div class="event-form-nft-actions">
                    <Show
                        when=move || !form.get().poster_url.is_empty()
                        fallback=|| view! { <div></div> }
                    >
                        <img
                            src=move || form.get().poster_url
                            alt="Event poster preview"
                            class="event-form-badge-preview"
                        />
                        <span class="event-form-badge-type-label">
                            {move || {
                                let url = form.get().poster_url;
                                if url.starts_with("/api/storage/posters") { "Uploaded poster" } else { "External poster" }.to_string()
                            }}
                        </span>
                    </Show>
                </div>
            </div>
            <div class="quiz-settings-grid">
                // File upload — only available once the event exists (needs an event_id).
                <div class="quiz-setting-item event-form-span-full">
                    <label class="quiz-field-label">"Upload Image"</label>
                    <Show
                        when=move || !editing_id.get().unwrap_or_default().is_empty()
                        fallback=|| view! {
                            <div class="quiz-setting-hint">
                                "\u{1F4CE} File upload needs a saved event. For now, paste an image URL just below \u{2014} or Save the event first, then re-open it (Edit) to upload a file."
                            </div>
                        }
                    >
                        <div class="event-form-nft-actions">
                            <input
                                type="file"
                                accept="image/*"
                                prop:disabled=move || poster_busy.get()
                                on:change=move |ev| {
                                    let target = match ev.target() {
                                        Some(t) => t,
                                        None => return,
                                    };
                                    let input: web_sys::HtmlInputElement = target.unchecked_into();
                                    let file = input.files().and_then(|fl| fl.item(0));
                                    let eid = editing_id.get().unwrap_or_default();
                                    if eid.is_empty() {
                                        components::show_toast(&set_toast, "Save the event before uploading a poster", components::ToastType::Error);
                                        return;
                                    }
                                    let Some(file) = file else { return };
                                    let content_type = file.type_();
                                    if !content_type.starts_with("image/") {
                                        components::show_toast(&set_toast, "Please choose an image file", components::ToastType::Error);
                                        return;
                                    }
                                    set_poster_busy.set(true);
                                    leptos::task::spawn_local(async move {
                                        match api::upload_poster(&eid, &file, &content_type).await {
                                            Ok(res) => {
                                                let url = res.poster_url.clone();
                                                set_form.update(|f| f.poster_url = url.clone());
                                                components::show_mutation_toast(&set_toast, "Poster uploaded", &res.warnings);
                                            }
                                            Err(e) => {
                                                log::error!("[event-form] poster upload failed: {e}");
                                                components::show_toast(&set_toast, &format!("Failed to upload poster: {e}"), components::ToastType::Error);
                                            }
                                        }
                                        set_poster_busy.set(false);
                                    });
                                }
                            />
                            <Show
                                when=move || poster_busy.get()
                                fallback=|| view! { <span></span> }
                            >
                                <span class="quiz-setting-hint">"Uploading…"</span>
                            </Show>
                        </div>
                    </Show>
                    <span class="quiz-setting-hint">"Max 5 MB. PNG / JPG / WebP / SVG."</span>
                </div>
                // URL override — always available (also covers pre-save case).
                <div class="quiz-setting-item event-form-span-full">
                    <label class="quiz-field-label">"Poster URL (override)"</label>
                    <div class="event-form-nft-actions">
                        <input
                            type="text"
                            class="quiz-number-input"
                            placeholder="/api/storage/posters/<id> or https://..."
                            prop:value=move || form.get().poster_url
                            on:input=move |ev| set_form.update(|f| f.poster_url = event_target_value(&ev))
                        />
                        <Show
                            when=move || !form.get().poster_url.is_empty() && !editing_id.get().unwrap_or_default().is_empty()
                            fallback=|| view! { <span></span> }
                        >
                            <button
                                class="btn btn-outline btn-sm"
                                prop:disabled=move || poster_busy.get()
                                on:click=move |_| {
                                    let eid = editing_id.get().unwrap_or_default();
                                    if eid.is_empty() {
                                        set_form.update(|f| f.poster_url = String::new());
                                        return;
                                    }
                                    set_poster_busy.set(true);
                                    leptos::task::spawn_local(async move {
                                        match api::delete_poster(&eid).await {
                                            Ok(res) => {
                                                set_form.update(|f| f.poster_url = String::new());
                                                components::show_mutation_toast(&set_toast, "Poster removed", &res.warnings);
                                            }
                                            Err(e) => {
                                                log::error!("[event-form] poster delete failed: {e}");
                                                components::show_toast(&set_toast, &format!("Failed to remove poster: {e}"), components::ToastType::Error);
                                            }
                                        }
                                        set_poster_busy.set(false);
                                    });
                                }
                            >
                                "✕ Remove"
                            </button>
                        </Show>
                    </div>
                    <Show
                        when=move || {
                            let v = form.get().poster_url.trim().to_string();
                            !v.is_empty()
                                && !v.starts_with("http://")
                                && !v.starts_with("https://")
                                && !v.starts_with("/api/storage/posters")
                        }
                        fallback=|| view! { <div></div> }
                    >
                        <div class="hint-warning-xs">
                            "URL should start with http://, https://, or /api/storage/posters"
                        </div>
                    </Show>
                </div>
            </div>
        </FormSection>

    }
}
