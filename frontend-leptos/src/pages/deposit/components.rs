//! Shared UI components used across deposit states.

use leptos::prelude::*;

use crate::components::{self, ToastType};
use crate::icons::{wallet_icon_name, Icon, IconName};
use crate::utils::{get_cluster, solscan_tx_url};

/// Wallet list with connect buttons.
pub fn wallet_list_view(
    wallets: &[String],
    on_click: impl Fn(String) + Clone + 'static,
) -> AnyView {
    let wallets_for_click = wallets.to_vec();
    view! {
        <div class="wallet-list">
            {wallets_for_click.into_iter().map(|w| {
                let w_clone = w.clone();
                let wallet_icon = wallet_icon_name(&w);
                let on_click = on_click.clone();
                view! {
                    <button
                        class="btn btn-primary btn-block wallet-btn-inner"
                        on:click={
                            let w = w.clone();
                            move |_| on_click(w.clone())
                        }
                    >
                        <Icon icon=wallet_icon class="icon-md wallet-icon-white" />
                        <span>{format!("Connect {}", &w_clone)}</span>
                    </button>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
    .into_any()
}

/// No wallets detected fallback message.
pub fn wallet_fallback_view() -> AnyView {
    view! {
        <div class="wallet-fallback-box">
            <p class="wallet-fallback-text">
                "No Solana wallet detected. Please install a wallet extension (Phantom, Backpack, Solflare) and refresh."
            </p>
        </div>
    }
        .into_any()
}

/// Wallet connected bar (shows wallet icon, name, address, connected badge).
pub fn wallet_connected_bar(wallet_name: &str, public_key: &str) -> AnyView {
    let wallet_icon = wallet_icon_name(wallet_name);
    let pk_short = super::types::truncate_pk(public_key);
    view! {
        <div class="wallet-connected-bar">
            <span class="wallet-icon-lg"><Icon icon=wallet_icon class="icon-lg wallet-icon-white" /></span>
            <div class="wallet-info-left">
                <div class="wallet-label">"Connected via " {wallet_name.to_string()}</div>
                <div class="wallet-address-bold">{pk_short}</div>
            </div>
            <span class="badge badge-success u-ml-auto"><Icon icon=IconName::Check class="icon-sm icon-success" />" Connected"</span>
        </div>
    }
        .into_any()
}

/// Generic "Go Back" button that transitions to a target state.
pub fn go_back_button(label: &str, on_click: impl Fn() + 'static) -> AnyView {
    view! {
        <button
            class="btn btn-outline btn-sm btn-action-secondary"
            on:click=move |_| on_click()
        >
            {label}
        </button>
    }
    .into_any()
}

/// Transaction hash display box.
pub fn tx_hash_box(sig_display: &str) -> AnyView {
    let sig = sig_display.to_string();
    view! {
        <div class="tx-hash-box">
            {format!("TX: {}", &sig)}
        </div>
    }
    .into_any()
}

/// Solscan explorer link.
pub fn solscan_link(tx_sig: &str) -> AnyView {
    let url = solscan_tx_url(tx_sig, &get_cluster());
    view! {
        <a href=&url target="_blank" class="tx-explorer-link">
            "View on Solscan ↗"
        </a>
    }
    .into_any()
}

/// Celebration emoji icon.
pub fn celebration_icon(icon: IconName) -> AnyView {
    view! {
        <div class="celebration-emoji"><Icon icon=icon class="icon-xl" /></div>
    }
    .into_any()
}

/// Spinner with loading text.
pub fn spinner_loading(text: &str) -> AnyView {
    let text = text.to_string();
    view! {
        <div class="spinner-wrap">
            <span class="spinner spinner-lg spinner-xl"></span>
        </div>
        <p class="hint-sm">{text}</p>
    }
    .into_any()
}

/// Back to event link.
pub fn back_to_event_link(event_slug: &str) -> AnyView {
    if event_slug.is_empty() {
        view! {
            <a href="/" class="link-back-home">
                "← Back to home"
            </a>
        }
        .into_any()
    } else {
        let slug = event_slug.to_string();
        view! {
            <a href=format!("/e/{slug}") class="link-back-home">
                "← Back to event"
            </a>
        }
        .into_any()
    }
}

/// Show a toast error.
pub fn show_error(set_toast: &WriteSignal<Option<components::ToastMessage>>, msg: &str) {
    components::show_toast(set_toast, msg, ToastType::Error);
}

/// Show a toast warning.
pub fn show_warning(set_toast: &WriteSignal<Option<components::ToastMessage>>, msg: &str) {
    components::show_toast(set_toast, msg, ToastType::Warning);
}

/// Show a toast success.
pub fn show_success(set_toast: &WriteSignal<Option<components::ToastMessage>>, msg: &str) {
    components::show_toast(set_toast, msg, ToastType::Success);
}
