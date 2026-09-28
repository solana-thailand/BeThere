//! QR code section — collapsible toggle + fullscreen overlay.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use super::view_data::TicketViewData;
use crate::components::{ImageLightbox, LightboxSizing};
use crate::i18n::{t_string, use_i18n};
use crate::icons::{Icon, IconName};

/// Pulsing indicator that the backend is actively checking.
/// Stays on indefinitely — no fake progress steps.
#[component]
pub fn ReassuranceTicker(
    /// Deposit method — controls the messaging.
    method: Option<crate::api::DepositMethod>,
) -> impl IntoView {
    let i18n = use_i18n();
    let received_label = move || match method {
        Some(crate::api::DepositMethod::Thb) | Some(crate::api::DepositMethod::CreditThb) => {
            t_string!(i18n, ticket.qr.slip_received)
        }
        Some(crate::api::DepositMethod::Usdc) | Some(crate::api::DepositMethod::CreditUsdc) => {
            t_string!(i18n, ticket.qr.tx_sent)
        }
        None => t_string!(i18n, ticket.qr.received),
    };
    let verifying_label = move || match method {
        Some(crate::api::DepositMethod::Thb) | Some(crate::api::DepositMethod::CreditThb) => {
            t_string!(i18n, ticket.qr.verifying_payment)
        }
        Some(crate::api::DepositMethod::Usdc) | Some(crate::api::DepositMethod::CreditUsdc) => {
            t_string!(i18n, ticket.qr.confirming_onchain)
        }
        None => t_string!(i18n, ticket.qr.verifying),
    };

    view! {
        <div class="ticket-ticker">
            <span class="ticket-ticker-step ticket-ticker-step--done">
                "\u{2705} "
                {received_label}
            </span>
            <span class="ticket-ticker-step ticket-ticker-step--active">
                "\u{23f3} "
                {verifying_label}
            </span>
        </div>
    }
}

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
    let deposit_method = view_data.deposit_info.as_ref().map(|d| d.method);
    let i18n = use_i18n();

    if is_checked_in {
        // Collapsible QR after check-in
        view! {
            <div class="ticket-qr-section">
                <button
                    class="btn btn-outline btn-sm"
                    on:click=move |_| set_show_qr.set(!show_qr.get())
                >
                    {move || if show_qr.get() {
                        t_string!(i18n, ticket.qr.hide)
                    } else {
                        t_string!(i18n, ticket.qr.show)
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
                                    alt=crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.qr.alt))
                                    class="ticket-qr-img"
                                />
                            </div>
                            <div class="ticket-qr-actions">
                                <button
                                    class="btn btn-outline btn-sm"
                                    on:click=move |_| set_fullscreen_qr.set(true)
                                >
                                    <Icon icon=IconName::Expand class="icon-sm" />
                                    " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.qr.full_screen))}
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
                                alt=crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.qr.alt))
                                class="ticket-qr-img"
                            />
                        </div>
                        <div class="ticket-qr-actions">
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_| set_fullscreen_qr.set(true)
                            >
                                <Icon icon=IconName::Expand class="icon-sm" />
                                " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.qr.full_screen))}
                            </button>
                            <button
                                class="btn btn-outline btn-sm"
                                on:click={
                                    let name = name.clone();
                                    move |_| {
                                        let qr = qr_image.clone();
                                        if let Some(ref data_url) = qr
                                            && !data_url.is_empty() {
                                                download_data_url(
                                                    data_url,
                                                    &format!("{name}-qrcode.svg"),
                                                );
                                            }
                                    }
                                }
                            >
                                <Icon icon=IconName::Save class="icon-sm" />
                                " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.qr.save))}
                            </button>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="ticket-qr-placeholder">
                            <div class="ticket-qr-placeholder-ghost">
                                <svg width="120" height="120" viewBox="0 0 120 120" fill="none" xmlns="http://www.w3.org/2000/svg">
                                    // Blurred QR silhouette
                                    <rect x="10" y="10" width="40" height="40" rx="4" fill="currentColor" opacity="0.08" />
                                    <rect x="55" y="10" width="20" height="20" rx="2" fill="currentColor" opacity="0.06" />
                                    <rect x="80" y="10" width="30" height="30" rx="3" fill="currentColor" opacity="0.07" />
                                    <rect x="10" y="55" width="20" height="25" rx="2" fill="currentColor" opacity="0.06" />
                                    <rect x="35" y="55" width="15" height="15" rx="2" fill="currentColor" opacity="0.05" />
                                    <rect x="55" y="55" width="25" height="25" rx="2" fill="currentColor" opacity="0.07" />
                                    <rect x="85" y="55" width="25" height="20" rx="2" fill="currentColor" opacity="0.06" />
                                    <rect x="10" y="85" width="30" height="25" rx="3" fill="currentColor" opacity="0.07" />
                                    <rect x="45" y="85" width="20" height="20" rx="2" fill="currentColor" opacity="0.05" />
                                    <rect x="70" y="85" width="40" height="25" rx="3" fill="currentColor" opacity="0.06" />
                                    // Lock overlay
                                    <circle cx="60" cy="60" r="22" fill="var(--bg-primary, #fff)" opacity="0.9" />
                                    <rect x="48" y="56" width="24" height="18" rx="3" fill="currentColor" opacity="0.25" />
                                    <path d="M52 56V50C52 46.686 54.686 44 58 44H62C65.314 44 68 46.686 68 50V56" stroke="currentColor" stroke-width="2.5" fill="none" opacity="0.25" />
                                    <circle cx="60" cy="64" r="2" fill="var(--bg-primary, #fff)" opacity="0.9" />
                                </svg>
                            </div>
                            <p class="ticket-qr-placeholder-text">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.qr.preparing))}
                            </p>
                            <p class="ticket-qr-placeholder-hint">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.qr.preparing_hint))}
                            </p>
                            <ReassuranceTicker method=deposit_method />
                        </div>
                    }.into_any()
                }}
            </div>
        }
        .into_any()
    }
}

/// Fullscreen QR overlay — delegates to the shared `ImageLightbox` so the
/// poster, slip preview, and QR all use the same click-to-dismiss pattern.
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
    // Convert Memo<String> -> Signal<String> so it satisfies ImageLightbox's
    // caption prop. With `#[prop(optional, into)]`, the call site passes the
    // inner `Signal<String>` (the macro wraps it in `Some`).
    let caption_signal: Signal<String> = get_name.into();
    let i18n = use_i18n();
    // `alt` and `hint` are plain strings on the lightbox, so rebuild it when
    // the language switches.
    view! {
        {move || view! {
            <ImageLightbox
                visible=fullscreen_qr
                set_visible=set_fullscreen_qr
                src=get_qr_image
                alt=t_string!(i18n, ticket.qr.lightbox_alt).to_string()
                caption=caption_signal
                hint=t_string!(i18n, ticket.qr.lightbox_hint).to_string()
                sizing=LightboxSizing::Square
            />
        }}
    }
}
