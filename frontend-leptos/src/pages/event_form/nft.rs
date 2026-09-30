//! NFT Attendance Badge section.

use leptos::prelude::*;

use super::ctx::FormCtx;
use super::section::{FormSection, SectionBadge};
use super::types::get_self_hosted_nft_urls;
use crate::icons::{Icon, IconName};

/// Badge name, symbol, image and metadata.
#[component]
pub(super) fn NftSection(ctx: FormCtx) -> impl IntoView {
    let FormCtx {
        form,
        set_form,
        editing_id,
        ..
    } = ctx;
    view! {
        <FormSection icon_class="form-section-icon-nft" title="NFT Attendance Badge" badge=SectionBadge::Recommended>
            // ── Quick-fill default badge ──
            <div class="quiz-setting-item event-form-span-full">
                <div class="hint-info event-form-nft-intro">
                    "NFT badges reward attendees for showing up. Use the default BeThere badge or skip if you don't need one."
                </div>
                <div class="event-form-nft-actions">
                    <button
                        class="btn btn-outline btn-sm"
                        on:click=move |_| {
                            let (img_url, meta_prefix) = get_self_hosted_nft_urls();
                            let eid = editing_id.get().unwrap_or_default();
                            set_form.update(|f| {
                                f.nft_image_url = img_url;
                                if !eid.is_empty() {
                                    f.nft_metadata_uri = format!("{meta_prefix}{eid}");
                                }
                                f.nft_name_template = "BeThere - {event_name}".to_string();
                                f.nft_symbol = "BETHERE".to_string();
                                f.nft_description_template = "Proof of attendance at {event_name}".to_string();
                            });
                        }
                    >
                        <Icon icon=IconName::Palette class="icon-sm"/>" Use default badge"
                    </button>
                    <button
                        class="btn btn-outline btn-sm"
                        on:click=move |_| {
                            set_form.update(|f| {
                                f.nft_image_url = String::new();
                                f.nft_metadata_uri = String::new();
                                f.nft_name_template = String::new();
                                f.nft_symbol = String::new();
                                f.nft_description_template = String::new();
                                f.nft_collection_mint = String::new();
                                f.merkle_tree = String::new();
                            });
                        }
                    >
                        "✕ Skip NFT"
                    </button>
                    // Badge preview
                    <Show
                        when=move || !form.get().nft_image_url.is_empty()
                        fallback=|| view! { <div></div> }
                    >
                        <img
                            src=move || form.get().nft_image_url
                            alt="NFT badge preview"
                            class="event-form-badge-preview"
                        />
                        <span class="event-form-badge-type-label">
                            {move || {
                                let url = form.get().nft_image_url;
                                if url.contains("/api/badge") { "Default badge" } else { "Custom badge" }.to_string()
                            }}
                        </span>
                    </Show>
                </div>
            </div>
            <div class="quiz-settings-grid">
            <div class="quiz-setting-item">
                <label class="quiz-field-label">"Collection Mint"</label>
                <input
                    type="text"
                    class="quiz-number-input"
                    placeholder="NFT collection mint address"
                    prop:value=move || form.get().nft_collection_mint
                    on:input=move |ev| set_form.update(|f| f.nft_collection_mint = event_target_value(&ev))
                />
                <Show
                    when=move || {
                        let v = form.get().nft_collection_mint.trim().to_string();
                        !v.is_empty() && !v.chars().all(|c| c.is_ascii_alphanumeric() && (c.is_ascii_digit() || ('A'..='H').contains(&c) || ('J'..='N').contains(&c) || ('P'..='Z').contains(&c) || ('a'..='k').contains(&c) || ('m'..='z').contains(&c)))
                    }
                    fallback=|| view! { <div></div> }
                >
                    <div class="hint-warning-xs">
                        "Invalid base58 characters detected (expected Solana address format)"
                    </div>
                </Show>
                <div class="quiz-setting-hint event-form-hint-row">
                    <span>"Solana mint address (base58)"</span>
                    <Show
                        when=move || !form.get().nft_collection_mint.trim().is_empty()
                        fallback=|| view! { <span></span> }
                    >
                        <a
                            href=move || crate::utils::metaplex_explorer_url(form.get().nft_collection_mint.trim(), &crate::utils::get_cluster())
                            target="_blank"
                            rel="noopener noreferrer"
                            class="event-form-link-sm"
                        >
                            "Verify on Metaplex ↗"
                        </a>
                    </Show>
                </div>
            </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Merkle Tree"</label>
                    <input
                        type="text"
                        class="quiz-number-input"
                        placeholder="Tree address (base58) — leave empty for Helius default"
                        prop:value=move || form.get().merkle_tree
                        on:input=move |ev| set_form.update(|f| f.merkle_tree = event_target_value(&ev))
                    />
                    <span class="quiz-setting-hint">"Reserved for future use. Helius mints to its own managed tree."</span>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Metadata URI"</label>
                <input
                    type="text"
                    class="quiz-number-input"
                    placeholder="https://..."
                    prop:value=move || form.get().nft_metadata_uri
                    on:input=move |ev| set_form.update(|f| f.nft_metadata_uri = event_target_value(&ev))
                />
                <Show
                    when=move || {
                        let v = form.get().nft_metadata_uri.trim().to_string();
                        !v.is_empty() && !v.starts_with("http://") && !v.starts_with("https://")
                    }
                    fallback=|| view! { <div></div> }
                >
                    <div class="hint-warning-xs">
                        "URI must start with http:// or https://"
                    </div>
                </Show>
                <Show
                    when=move || editing_id.get().unwrap_or_default().is_empty()
                    fallback=|| view! { <div></div> }
                >
                    <span class="quiz-setting-hint">"Save the event first, then edit to auto-fill this with the dynamic metadata URL."</span>
                </Show>
            </div>
            <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Image URL"</label>
                <input
                    type="text"
                    class="quiz-number-input"
                    placeholder="https://..."
                    prop:value=move || form.get().nft_image_url
                    on:input=move |ev| set_form.update(|f| f.nft_image_url = event_target_value(&ev))
                />
                <Show
                    when=move || {
                        let v = form.get().nft_image_url.trim().to_string();
                        !v.is_empty() && !v.starts_with("http://") && !v.starts_with("https://")
                    }
                    fallback=|| view! { <div></div> }
                >
                    <div class="hint-warning-xs">
                        "URL must start with http:// or https://"
                    </div>
                </Show>
            </div>
            <div class="quiz-setting-item">
                <label class="quiz-field-label">"Name Template"</label>
                <input
                    type="text"
                    class="quiz-number-input"
                    placeholder="{event_name} #1"
                    prop:value=move || form.get().nft_name_template
                    on:input=move |ev| set_form.update(|f| f.nft_name_template = event_target_value(&ev))
                />
                <span class="quiz-setting-hint">"Use {event_name} placeholder"</span>
                <Show
                    when=move || {
                        let f = form.get();
                        let resolved = if f.nft_name_template.is_empty() {
                            format!("BeThere - {}", f.name)
                        } else {
                            f.nft_name_template.replace("{event_name}", &f.name)
                        };
                        resolved.len() > 32
                    }
                    fallback=|| view! { <div></div> }
                >
                    <div class="hint-warning-xs">
                        "Resolved name exceeds 32-char limit (Bubblegum max). Name will be truncated."
                    </div>
                </Show>
            </div>
            <div class="quiz-setting-item">
                <label class="quiz-field-label">"Symbol"</label>
                <input
                    type="text"
                    class="quiz-number-input"
                    placeholder="NFT"
                    prop:value=move || form.get().nft_symbol
                    on:input=move |ev| set_form.update(|f| f.nft_symbol = event_target_value(&ev))
                />
            </div>
            <div class="quiz-setting-item">
                <label class="quiz-field-label">"Description Template"</label>
                <textarea
                    class="quiz-textarea quiz-textarea-sm"
                    placeholder="NFT description..."
                    prop:value=move || form.get().nft_description_template
                    on:input=move |ev| set_form.update(|f| f.nft_description_template = event_target_value(&ev))
                ></textarea>
            </div>
        </div>
        </FormSection>

    }
}
