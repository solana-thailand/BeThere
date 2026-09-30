//! Community & Resources section.

use leptos::prelude::*;

use super::ctx::FormCtx;
use super::section::{FormSection, SectionBadge};
use super::types::MAX_COMMUNITY_LINKS;

/// Shared community, logistics and learning links.
#[component]
pub(super) fn CommunitySection(ctx: FormCtx) -> impl IntoView {
    let FormCtx {
        cl_links,
        set_cl_links,
        ..
    } = ctx;
    let add_community_link = move || {
        set_cl_links.update(|links| {
            if links.len() < MAX_COMMUNITY_LINKS {
                links.push(crate::api::CommunityLink {
                    platform: "discord".to_string(),
                    url: String::new(),
                    label: String::new(),
                });
            }
        });
    };

    view! {
        <FormSection icon_class="form-section-icon-community" title="Community & Resources" badge=SectionBadge::Optional open=false>
                <p class="quiz-setting-hint">
                    "Add community channels, logistics guides, or post-event learning resources. Resource types appear on a published recap in the order shown here; use clear labels such as ‘Workshop slides’ or ‘Example repository’."
                </p>
                {move || {
                    let links = cl_links.get();
                    let links_len = links.len();
                    links.into_iter().enumerate().map(|(i, link)| {
                        let idx = i;
                        let is_first = idx == 0;
                        let is_last = idx + 1 == links_len;
                        let platform = link.platform.clone();
                        let url = link.url.clone();
                        let label = link.label.clone();
                        view! {
                            <div class="community-link-row">
                                <select
                                    class="quiz-number-input community-link-platform"
                                    on:change=move |ev| {
                                        let val = event_target_value(&ev);
                                        set_cl_links.update(|links| {
                                            if idx < links.len() {
                                                links[idx].platform = val;
                                            }
                                        });
                                    }
                                >
                                    <option value="discord" selected=platform == "discord">"Discord"</option>
                                    <option value="telegram" selected=platform == "telegram">"Telegram"</option>
                                    <option value="x" selected=platform == "x">"X (Twitter)"</option>
                                    <option value="facebook" selected=platform == "facebook">"Facebook"</option>
                                    <option value="line" selected=platform == "line">"LINE"</option>
                                    <option value="website" selected=platform == "website">"Website"</option>
                                    <option value="guide" selected=platform == "guide">"Guide (logistics)"</option>
                                    <option value="resource" selected=platform == "resource">"Learning resource"</option>
                                    <option value="slides" selected=platform == "slides">"Slides"</option>
                                    <option value="source" selected=platform == "source">"Source code"</option>
                                    <option value="download" selected=platform == "download">"Download"</option>
                                </select>
                                <input
                                    type="url"
                                    class="quiz-number-input community-link-url"
                                    placeholder="https://..."
                                    prop:value=url
                                    on:input=move |ev| {
                                        let val = event_target_value(&ev);
                                        set_cl_links.update(|links| {
                                            if idx < links.len() {
                                                links[idx].url = val;
                                            }
                                        });
                                    }
                                />
                                <input
                                    type="text"
                                    class="quiz-number-input community-link-label"
                                    placeholder="Label (optional)"
                                    prop:value=label
                                    on:input=move |ev| {
                                        let val = event_target_value(&ev);
                                        set_cl_links.update(|links| {
                                            if idx < links.len() {
                                                links[idx].label = val;
                                            }
                                        });
                                    }
                                />
                                <div class="community-link-order-controls">
                                    <button
                                        type="button"
                                        class="btn btn-ghost btn-xs"
                                        disabled=is_first
                                        title="Move link up"
                                        aria-label=format!("Move link {} up", idx + 1)
                                        on:click=move |_| {
                                            set_cl_links.update(|links| {
                                                if idx > 0 && idx < links.len() {
                                                    links.swap(idx, idx - 1);
                                                }
                                            });
                                        }
                                    >"↑"</button>
                                    <button
                                        type="button"
                                        class="btn btn-ghost btn-xs"
                                        disabled=is_last
                                        title="Move link down"
                                        aria-label=format!("Move link {} down", idx + 1)
                                        on:click=move |_| {
                                            set_cl_links.update(|links| {
                                                if idx + 1 < links.len() {
                                                    links.swap(idx, idx + 1);
                                                }
                                            });
                                        }
                                    >"↓"</button>
                                </div>
                                <button
                                    type="button"
                                    class="btn btn-outline btn-xs community-link-remove"
                                    title="Remove link"
                                    aria-label=format!("Remove link {}", idx + 1)
                                    on:click=move |_| {
                                        set_cl_links.update(|links| {
                                            if idx < links.len() {
                                                links.remove(idx);
                                            }
                                        });
                                    }
                                >
                                    "×"
                                </button>
                            </div>
                        }
                    }).collect::<Vec<_>>()
                }}
                {move || {
                    let count = cl_links.get().len();
                    if count < MAX_COMMUNITY_LINKS {
                        view! {
                            <button
                                type="button"
                                class="btn btn-outline btn-sm community-link-add"
                                on:click=move |_| add_community_link()
                            >
                                "+ Add Link"
                            </button>
                        }.into_any()
                    } else {
                        ().into_any()
                    }
                }}
        </FormSection>

    }
}
