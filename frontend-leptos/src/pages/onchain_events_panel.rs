//! Collapsible on-chain escrow events panel — loaded on demand.
//!
//! Displays a color-coded timeline of on-chain escrow events
//! (deposits, check-ins, refunds, closes, etc.) fetched from the backend.

use leptos::prelude::*;

use crate::api::{self, EscrowInstruction, OnChainEvent};

/// Collapsible panel that displays on-chain escrow events for a specific event.
///
/// Loads events lazily — only fetches from the API the first time the
/// panel is opened. Only shown when the event has an escrow address.
#[component]
pub fn OnchainEventsPanel(event_id: String) -> impl IntoView {
    let (open, set_open) = signal(false);
    let (events, set_events) = signal(Vec::<OnChainEvent>::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let load_events = move || {
        let eid = event_id.clone();
        leptos::task::spawn_local(async move {
            set_loading.set(true);
            set_error.set(None);
            match api::get_onchain_events(&eid).await {
                Ok(data) => {
                    set_events.set(data.events);
                }
                Err(e) => {
                    set_error.set(Some(e.to_string()));
                }
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="form-section onchain-panel">
            <div class="form-section-header" on:click=move |_| {
                let was_open = open.get();
                set_open.set(!was_open);
                if !was_open && events.get().is_empty() {
                    load_events();
                }
            }>
                <span class="form-section-icon onchain-icon">"⛓"</span>
                <span class="form-section-title">"On-Chain Events"</span>
                <span class="form-section-badge onchain-badge-count">
                    {move || {
                        let count = events.get().len();
                        if count > 0 { format!("{count} events") } else { "Click to load".to_string() }
                    }}
                </span>
                <span class="form-section-toggle" class:form-section-toggle-open=move || open.get()>"▼"</span>
            </div>
            <div class="form-section-body" class:form-section-body-hidden=move || !open.get()>
                <Show when=move || loading.get() fallback=|| view! { <div></div> }>
                    <div class="onchain-loading">
                        <span class="spinner spinner-md"></span>
                        " Loading on-chain events..."
                    </div>
                </Show>
                <Show when=move || error.get().is_some() && !loading.get() fallback=|| view! { <div></div> }>
                    <div class="onchain-error">
                        {move || error.get().unwrap_or_default()}
                    </div>
                </Show>
                <Show when=move || !loading.get() && error.get().is_none() && events.get().is_empty() fallback=|| view! { <div></div> }>
                    <div class="onchain-empty">
                        "No on-chain events indexed yet. Click Sync to poll for recent transactions."
                    </div>
                </Show>
                <Show when=move || !events.get().is_empty() && !loading.get() fallback=|| view! { <div></div> }>
                    // Legend
                    <div class="onchain-legend">
                        <LegendItem color=EscrowInstruction::CreateEvent.color().to_string() label="Create" />
                        <LegendItem color=EscrowInstruction::Deposit.color().to_string() label="Deposit" />
                        <LegendItem color=EscrowInstruction::MarkCheckedIn.color().to_string() label="Check-in" />
                        <LegendItem color=EscrowInstruction::Refund.color().to_string() label="Refund" />
                        <LegendItem color=EscrowInstruction::ClaimForfeited.color().to_string() label="Forfeit" />
                        <LegendItem color=EscrowInstruction::DeactivateEvent.color().to_string() label="Deactivate" />
                        <LegendItem color=EscrowInstruction::CloseEvent.color().to_string() label="Close" />
                    </div>
                    // Timeline
                    <div class="onchain-timeline">
                        {move || events.get().into_iter().map(|e| {
                            let label = e.instruction.label().to_string();
                            let time = format_block_time(e.block_time);
                            let sig_short = truncate_signature(&e.signature);
                            let amount_str = e.amount.map(format_usdc_amount).unwrap_or_default();
                            let attendee_short = e.attendee.as_ref().map(|a| truncate_address(a)).unwrap_or_default();
                            let solscan_url = crate::utils::solscan_tx_url(&e.signature, "devnet");
                            let dot_style = format!("background: {}", e.instruction.color());
                            let badge_style = format!("background: {}", e.instruction.color());

                            view! {
                                <div class="onchain-event-item">
                                    // Color dot
                                    <div class="onchain-event-dot" style=dot_style></div>
                                    // Time
                                    <div class="onchain-event-time">{time}</div>
                                    // Badge
                                    <div class="onchain-event-badge-wrap">
                                        <span class="onchain-event-badge" style=badge_style>
                                            {label}
                                        </span>
                                    </div>
                                    // Details
                                    <div class="onchain-event-details">
                                        {if !amount_str.is_empty() {
                                            view! { <span class="onchain-event-amount">{amount_str.clone()}</span> }.into_any()
                                        } else {
                                            view! { <span></span> }.into_any()
                                        }}
                                        {if !attendee_short.is_empty() {
                                            view! { <span class="onchain-event-attendee">{attendee_short.clone()}</span> }.into_any()
                                        } else {
                                            view! { <span></span> }.into_any()
                                        }}
                                    </div>
                                    // TX link
                                    <a
                                        class="onchain-event-tx-link"
                                        href=solscan_url
                                        target="_blank"
                                        rel="noopener noreferrer"
                                    >
                                        {sig_short}
                                    </a>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                </Show>
            </div>
        </div>
    }
}

/// Legend item for the color key.
#[component]
fn LegendItem(color: String, label: &'static str) -> impl IntoView {
    let dot_style = format!("background: {color}");
    view! {
        <div class="onchain-legend-item">
            <div class="onchain-legend-dot" style=dot_style></div>
            {label}
        </div>
    }
}

/// Format a unix timestamp (block_time) into "Mon DD, HH:MM".
fn format_block_time(ts: i64) -> String {
    let js_ts = (ts as f64) * 1000.0;
    let parsed = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(js_ts));
    if parsed.get_time().is_nan() {
        return format!("{ts}");
    }

    let month = match parsed.get_month() as u8 {
        0 => "Jan", 1 => "Feb", 2 => "Mar", 3 => "Apr",
        4 => "May", 5 => "Jun", 6 => "Jul", 7 => "Aug",
        8 => "Sep", 9 => "Oct", 10 => "Nov", 11 => "Dec",
        _ => "???",
    };
    let day = parsed.get_date();
    let hours = parsed.get_hours();
    let minutes = parsed.get_minutes();

    format!("{month} {day:02}, {hours:02}:{minutes:02}")
}

/// Truncate a Solana signature for display: first 6...last 4 chars.
fn truncate_signature(sig: &str) -> String {
    if sig.len() <= 14 {
        sig.to_string()
    } else {
        let start: String = sig.chars().take(6).collect();
        let end: String = sig.chars().rev().take(4).collect();
        format!("{start}…{end}")
    }
}

/// Truncate a Solana address for display: first 4...last 4 chars.
fn truncate_address(addr: &str) -> String {
    if addr.len() <= 12 {
        addr.to_string()
    } else {
        let start: String = addr.chars().take(4).collect();
        let end: String = addr.chars().rev().take(4).collect();
        format!("{start}…{end}")
    }
}

/// Format USDC lamports to human-readable string.
fn format_usdc_amount(lamports: u64) -> String {
    let usdc = lamports as f64 / 1_000_000.0;
    format!("${usdc:.2}")
}
