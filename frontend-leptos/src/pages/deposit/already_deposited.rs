//! AlreadyDeposited + NotEnabled + Error + Loading views.

use leptos::prelude::*;

use crate::api::{DepositMethod, DepositStatusResponse};
use crate::utils::format_timestamp;

use super::types::*;

/// Loading view.
pub fn loading_view() -> AnyView {
    view! {
        <div class="dep2-card">
            <div class="dep2-confirming">
                <div class="dep2-confirming-dots">
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                </div>
                <p>"Loading deposit info..."</p>
            </div>
        </div>
    }
    .into_any()
}

/// Error view.
pub fn error_view(msg: &str) -> AnyView {
    let msg = msg.to_string();
    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">"Something went wrong"</span>
            </div>
            <p>{msg}</p>
            <a href="/" class="btn btn-primary">"Go Home"</a>
        </div>
    }
    .into_any()
}

/// Not enabled view.
///
/// Surfaces the **resolved** event name + slug so the organizer can immediately
/// tell whether the wrong event was looked up. The most common cause of this
/// view in production is the "no event_id in URL → first active event fallback"
/// path: the URL falls through to whatever happens to be the newest active
/// event, which may have deposits disabled even though the attendee's actual
/// event has them enabled. Showing the resolved event name makes that mismatch
/// self-diagnosable instead of looking like a backend bug.
pub fn not_enabled_view(data: &DepositStatusResponse) -> AnyView {
    let event_name = data.event_name.clone();
    let event_slug = data.event_slug.clone();
    let has_event_info = !event_name.is_empty();
    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">"Deposits Not Available"</span>
            </div>
            <p>"Deposits are not enabled for this event."</p>

            // Diagnostic block — surface which event the backend actually
            // resolved. Without this, "wrong event fallback" looks identical
            // to "deposits genuinely disabled", forcing a curl to debug.
            {if has_event_info {
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            {format!("Resolved event: {event_name}")}
                        </p>
                        {if !event_slug.is_empty() {
                            view! {
                                <p class="hint-note">
                                    {format!("Slug: {event_slug}")}
                                </p>
                            }.into_any()
                        } else {
                            ().into_any()
                        }}
                        <p class="hint-note">
                            "If this isn't the event you expected, check that the URL contains the correct \
                             \"?event_id=...\" for your event, or hard-refresh to bypass any stale cache."
                        </p>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}

            <a href="/" class="btn btn-primary">"Go Home"</a>
        </div>
    }
    .into_any()
}

/// Already deposited view.
pub fn already_deposited_view(
    data: &DepositStatusResponse,
    set_state: &WriteSignal<DepositPageState>,
) -> AnyView {
    let info = data.status.as_ref().unwrap();
    let (_method_icon, method_label) = deposit_method_display(&info.method);
    let is_credit = info.tx_signature.as_ref().is_some_and(|s| s.contains("CREDIT"))
        || info.wallet_address.as_ref().is_some_and(|w| w.contains("CREDIT"));
    let display_method_label = if is_credit {
        "Rolling Credit (Previous Event)".to_string()
    } else {
        method_label.to_string()
    };
    let verified_text = if info.verified {
        "Verified"
    } else {
        "Pending Verification"
    };
    let verified_class = if info.verified {
        "badge badge-success"
    } else {
        "badge badge-warning"
    };
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);
    let refund_info = compute_refund_info(data);
    // Refund window check: mirrors bethere-escrow refund instruction's
    // two-path model (checked-in → [event_end, ∞); no-show →
    // [event_end, refund_deadline)). The on-chain program is the source of
    // truth; this gate is advisory and hides the CTA to avoid offering a TX
    // that would revert. Fails safe on missing data. See
    // `event_refund_window_open` for the full contract.
    let event_ended =
        event_refund_window_open(data.event_end_ms, data.refund_deadline_ms, data.checked_in);

    let data_clone_for_refund = data.clone();
    let refund_info_clone = refund_info.clone();
    let data_clone_for_event_link = data.clone();
    let info_clone = data.status.clone();

    let set_state = *set_state;

    view! {
        <div class="dep2-card">
            // Header: title + badge
            <div class="dep2-card-header">
                <span class="dep2-card-title">"Spot Reserved"</span>
                <span class=verified_class>
                    {verified_text}
                </span>
            </div>

            // Success icon (only if verified)
            {if info.verified {
                view! {
                    <div class="dep2-success-icon">"✓"</div>
                }.into_any()
            } else {
                ().into_any()
            }}

            // Amount hero
            <div class="dep2-amount-hero">
                {{
                    let info = info_clone.as_ref().unwrap();
                    if info.currency == "THB" {
                        format!("฿{}", info.amount)
                    } else {
                        format!("{} {}", format_usdc(info.amount), info.currency)
                    }
                }}
                " deposited"
            </div>

            // Receipt block
            <div class="dep2-receipt">
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"Method"</span>
                    <span class="dep2-receipt-value">
                        {display_method_label.clone()}
                    </span>
                </div>
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"Amount"</span>
                    <span class="dep2-receipt-value">
                        {{
                            let info = info_clone.as_ref().unwrap();
                            if info.currency == "THB" {
                                format!("฿{}", info.amount)
                            } else {
                                format!("{} {}", format_usdc(info.amount), info.currency)
                            }
                        }}
                    </span>
                </div>
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"Status"</span>
                    <span class="dep2-receipt-value">
                        <span class=verified_class>
                            {verified_text}
                        </span>
                    </span>
                </div>
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"Date"</span>
                    <span class="dep2-receipt-value">
                        {format_timestamp(&info_clone.as_ref().unwrap().deposited_at)}
                    </span>
                </div>
            </div>

            // Refund info section
            {if info.verified && info.method == DepositMethod::Usdc {
                let data_clone_for_refund = data_clone_for_refund.clone();
                let refund_info_clone = refund_info_clone.clone();
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            {format!("Your {usdc_fmt} USDC is secured on-chain. Show up → get it all back.")}
                        </p>
                    </div>
                    // Refund deadline
                    {match refund_info_clone {
                        Some((deadline_date, duration_label)) => view! {
                            <div class="dep2-deadline dep2-deadline--warning">
                                <span class="dep2-deadline-text">
                                    {format!("Refund window: {duration_label} after event ends ({deadline_date}).")}
                                </span>
                            </div>
                        }.into_any(),
                        None => ().into_any(),
                    }}
                    // Refund CTA — three states:
                    //   (a) refundable tier AND event ended → claim button
                    //   (b) refundable tier AND event not ended → "available after event" notice
                    //   (c) not refundable tier → muted badge (unchanged)
                    {if info.refundable && event_ended {
                        let data_clone_for_refund = data_clone_for_refund.clone();
                        view! {
                            <div class="dep2-refund-cta">
                                <span class="dep2-refund-cta-text">
                                    {format!("Don't lose your {usdc_fmt} USDC — claim it now")}
                                </span>
                                <button
                                    class="btn btn-success btn-block"
                                    on:click=move |_| {
                                        set_state.set(DepositPageState::RefundChooseWallet(data_clone_for_refund.clone()));
                                    }
                                >
                                    "Claim Refund"
                                </button>
                            </div>
                        }.into_any()
                    } else if info.refundable {
                        view! {
                            <div class="dep2-info-note">
                                <p class="hint-note">
                                    "Refund will be available after the event ends."
                                </p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <span class="badge badge-muted">"Non-refundable deposit"</span>
                        }.into_any()
                    }}
                }.into_any()
            } else if !info.verified {
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            "Refund will be available after verification."
                        </p>
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            {format!("Your {usdc_fmt} USDC deposit is secured. Refund will be available after the event.")}
                        </p>
                    </div>
                }.into_any()
            }}

            // Back to event link
            <a
                href=if data_clone_for_event_link.event_slug.is_empty() { "/".to_string() } else { format!("/e/{}", data_clone_for_event_link.event_slug) }
                class="dep2-back"
            >
                "← Back to event"
            </a>
        </div>
    }
        .into_any()
}
