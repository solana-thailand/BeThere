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
        <div class="form-section" style="margin-top: 1rem">
            <div class="form-section-header" on:click=move |_| {
                let was_open = open.get();
                set_open.set(!was_open);
                if !was_open && events.get().is_empty() {
                    load_events();
                }
            }>
                <span class="form-section-icon" style="background: #6366f1">"⛓"</span>
                <span class="form-section-title">"On-Chain Events"</span>
                <span class="form-section-badge" style="background: #eef2ff; color: #4f46e5">
                    {move || {
                        let count = events.get().len();
                        if count > 0 { format!("{count} events") } else { "Click to load".to_string() }
                    }}
                </span>
                <span class="form-section-toggle" class:form-section-toggle-open=move || open.get()>"▼"</span>
            </div>
            <div class="form-section-body" class:form-section-body-hidden=move || !open.get()>
                <Show when=move || loading.get() fallback=|| view! { <div></div> }>
                    <div style="text-align: center; padding: 1rem; color: #6b7280">
                        <span class="spinner spinner-md"></span>
                        " Loading on-chain events..."
                    </div>
                </Show>
                <Show when=move || error.get().is_some() && !loading.get() fallback=|| view! { <div></div> }>
                    <div style="color: #dc2626; padding: 0.5rem">
                        {move || error.get().unwrap_or_default()}
                    </div>
                </Show>
                <Show when=move || !loading.get() && error.get().is_none() && events.get().is_empty() fallback=|| view! { <div></div> }>
                    <div style="text-align: center; padding: 1rem; color: #6b7280">
                        "No on-chain events indexed yet. Click Sync to poll for recent transactions."
                    </div>
                </Show>
                <Show when=move || !events.get().is_empty() && !loading.get() fallback=|| view! { <div></div> }>
                    // Legend
                    <div style="display: flex; flex-wrap: wrap; gap: 0.5rem; margin-bottom: 0.75rem; padding: 0.5rem; background: #f9fafb; border-radius: 6px">
                        <LegendItem color=EscrowInstruction::CreateEvent.color().to_string() label="Create" />
                        <LegendItem color=EscrowInstruction::Deposit.color().to_string() label="Deposit" />
                        <LegendItem color=EscrowInstruction::MarkCheckedIn.color().to_string() label="Check-in" />
                        <LegendItem color=EscrowInstruction::Refund.color().to_string() label="Refund" />
                        <LegendItem color=EscrowInstruction::ClaimForfeited.color().to_string() label="Forfeit" />
                        <LegendItem color=EscrowInstruction::DeactivateEvent.color().to_string() label="Deactivate" />
                        <LegendItem color=EscrowInstruction::CloseEvent.color().to_string() label="Close" />
                    </div>
                    // Timeline
                    <div class="onchain-timeline" style="max-height: 400px; overflow-y: auto">
                        {move || events.get().into_iter().map(|e| {
                            let label = e.instruction.label().to_string();
                            let time = format_block_time(e.block_time);
                            let sig_short = truncate_signature(&e.signature);
                            let amount_str = e.amount.map(|a| format_usdc_amount(a)).unwrap_or_default();
                            let attendee_short = e.attendee.as_ref().map(|a| truncate_address(a)).unwrap_or_default();
                            let solscan_url = format!("https://solscan.io/tx/{}?cluster=devnet", e.signature);
                            let dot_style = format!("flex-shrink: 0; width: 10px; height: 10px; border-radius: 50%; background: {}; margin-top: 4px", e.instruction.color());
                            let badge_style = format!("padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 600; color: white; background: {}", e.instruction.color());

                            view! {
                                <div class="onchain-event-item" style="display: flex; align-items: flex-start; gap: 0.75rem; padding: 0.5rem 0; border-bottom: 1px solid #f3f4f6; font-size: 0.85rem">
                                    // Color dot
                                    <div style=dot_style></div>
                                    // Time
                                    <div style="flex-shrink: 0; width: 80px; color: #6b7280">{time}</div>
                                    // Badge
                                    <div style="flex-shrink: 0">
                                        <span style=badge_style>
                                            {label}
                                        </span>
                                    </div>
                                    // Details
                                    <div style="flex: 1; color: #374151; min-width: 0">
                                        {if !amount_str.is_empty() {
                                            view! { <span style="font-weight: 600">{amount_str.clone()}</span> }.into_any()
                                        } else {
                                            view! { <span></span> }.into_any()
                                        }}
                                        {if !attendee_short.is_empty() {
                                            view! { <span style="color: #6b7280; margin-left: 0.25rem">{attendee_short.clone()}</span> }.into_any()
                                        } else {
                                            view! { <span></span> }.into_any()
                                        }}
                                    </div>
                                    // TX link
                                    <a
                                        href=solscan_url
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        style="flex-shrink: 0; color: #3b82f6; font-size: 0.75rem; text-decoration: none"
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
    let dot_style = format!("width: 8px; height: 8px; border-radius: 50%; background: {color}");
    view! {
        <div style="display: flex; align-items: center; gap: 4px; font-size: 0.75rem; color: #4b5563">
            <div style=dot_style></div>
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
