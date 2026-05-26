//! QR code section — collapsible toggle + fullscreen overlay.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use super::view_data::TicketViewData;
use crate::icons::{Icon, IconName};

#[wasm_bindgen(module = "/js/download.js")]
extern "C" {
    #[wasm_bindgen(js_name = "downloadDataUrl")]
    fn download_data_url(data_url: &str, filename: &str);
}

/// Props for the in-page QR code section.
#[component]
pub fn QrSection(
    /// Pre-computed view data
    view_data: TicketViewData,
    /// Whether QR section is expanded (reactive)
    show_qr: ReadSignal<bool>,
    /// Toggle QR expansion
    set_show_qr: WriteSignal<bool>,
    /// Open fullscreen overlay
    set_fullscreen_qr: WriteSignal<bool>,
) -> impl IntoView {
    let qr_image = view_data.qr_image.clone();
    let has_qr = view_data.has_qr;
    let name = view_data.name.clone();
    let is_checked_in = view_data.is_checked_in;

    if is_checked_in {
        // Collapsible QR after check-in
        view! {
            <div class="ticket-qr-section">
                <button
                    class="btn btn-outline btn-sm"
                    on:click=move |_| set_show_qr.set(!show_qr.get())
                >
                    {move || if show_qr.get() {
                        "▲ Hide QR Code"
                    } else {
                        "▼ Show QR Code"
                    }}
                </button>
                <Show
                    when=move || show_qr.get()
                    fallback=|| view! { <div></div> }
                >
                    {if has_qr {
                        view! {
                            <div class="ticket-qr-wrapper">
                                <img
                                    src=qr_image.clone().unwrap_or_default()
                                    alt="Check-in QR Code"
                                    class="ticket-qr-img"
                                />
                            </div>
                            <div class="ticket-qr-actions">
                                <button
                                    class="btn btn-outline btn-sm"
                                    on:click=move |_| set_fullscreen_qr.set(true)
                                >
                                    <Icon icon=IconName::Expand class="icon-sm" />
                                    " Full Screen"
                                </button>
                            </div>
                        }.into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }}
                </Show>
            </div>
        }
        .into_any()
    } else {
        // Pre-checkin: QR as hero
        view! {
            <div class="ticket-qr-section">
                {if has_qr {
                    view! {
                        <div class="ticket-qr-wrapper">
                            <img
                                src=qr_image.clone().unwrap_or_default()
                                alt="Check-in QR Code"
                                class="ticket-qr-img"
                            />
                        </div>
                        <div class="ticket-qr-actions">
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_| set_fullscreen_qr.set(true)
                            >
                                <Icon icon=IconName::Expand class="icon-sm" />
                                " Full Screen"
                            </button>
                            <button
                                class="btn btn-outline btn-sm"
                                on:click={
                                    let name = name.clone();
                                    move |_| {
                                        let qr = qr_image.clone();
                                        if let Some(ref data_url) = qr {
                                            if !data_url.is_empty() {
                                                download_data_url(
                                                    data_url,
                                                    &format!("{name}-qrcode.svg"),
                                                );
                                            }
                                        }
                                    }
                                }
                            >
                                <Icon icon=IconName::Save class="icon-sm" />
                                " Save QR Code"
                            </button>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="ticket-qr-placeholder">
                            <Icon icon=IconName::QrCode class="icon-xl" />
                            <p class="hint">"QR code not yet generated"</p>
                        </div>
                    }.into_any()
                }}
            </div>
        }
        .into_any()
    }
}

/// Fullscreen QR overlay — rendered outside main layout.
#[component]
pub fn FullscreenQrOverlay(
    /// Whether the overlay is visible
    fullscreen_qr: ReadSignal<bool>,
    /// Close the overlay
    set_fullscreen_qr: WriteSignal<bool>,
    /// Reactive state for getting current attendee name/qr
    #[prop(into)]
    get_name: Memo<String>,
    #[prop(into)] get_qr_image: Memo<String>,
) -> impl IntoView {
    view! {
        <Show
            when=move || fullscreen_qr.get()
            fallback=|| view! { <div></div> }
        >
            <div
                class="ticket-fullscreen-overlay"
                on:click=move |_| set_fullscreen_qr.set(false)
            >
                <div
                    class="ticket-fullscreen-card"
                    on:click=move |ev| ev.stop_propagation()
                >
                    <div class="ticket-fullscreen-header">
                        <span class="ticket-fullscreen-name">
                            {move || get_name.get()}
                        </span>
                        <button
                            class="ticket-fullscreen-close"
                            on:click=move |_| set_fullscreen_qr.set(false)
                        >
                            "✕"
                        </button>
                    </div>
                    <img
                        src=move || get_qr_image.get()
                        alt="QR Code"
                        class="ticket-fullscreen-qr"
                    />
                    <p class="ticket-fullscreen-hint">"Show this code to staff"</p>
                </div>
            </div>
        </Show>
    }
}
