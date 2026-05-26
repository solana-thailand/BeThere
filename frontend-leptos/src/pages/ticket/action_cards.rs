//! Action card components for deposit, refund, claim, and reclaim flows.

use crate::api::DepositMethod;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

/// Deposit required action card — prompts attendee to pay their deposit.
#[component]
pub fn DepositActionCard(
    /// Deposit amount in USDC (smallest unit, e.g. 15000000 = 15 USDC). 0 = not configured.
    amount_usdc: u64,
    /// Deposit amount in THB. 0 = not configured.
    amount_thb: u64,
    /// Whether the on-chain escrow is closed (USDC deposit unavailable).
    #[prop(default = false)]
    escrow_closed: bool,
    /// Deadline in hours after registration
    deadline_hours: Option<u32>,
    /// Link to the deposit page
    #[prop(into)]
    deposit_href: String,
) -> impl IntoView {
    let show_usdc = amount_usdc > 0 && !escrow_closed;
    let show_thb = amount_thb > 0;

    // Build primary label
    let primary_label = if show_thb {
        format!("{amount_thb} THB")
    } else if show_usdc {
        format!("${:.2} USDC", amount_usdc as f64 / 1_000_000.0)
    } else {
        "Deposit Required".to_string()
    };

    view! {
        <div class="ticket-action-card ticket-action-card--deposit">
            <div class="ticket-action-icon">
                <Icon icon=IconName::CreditCard class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">
                    {format!("Deposit Required: {primary_label}")}
                </div>
                // Show secondary payment method when both are available
                {if show_thb && show_usdc {
                    let usdc_str = format!("${:.2} USDC", amount_usdc as f64 / 1_000_000.0);
                    view! {
                        <div class="ticket-action-desc" style="margin-bottom:0.25rem;">
                            <span style="color:var(--text-secondary);font-size:0.8rem;">
                                "Also payable as "{usdc_str}" via Solana"
                            </span>
                        </div>
                    }.into_any()
                } else if amount_usdc > 0 && escrow_closed {
                    view! {
                        <div class="ticket-action-desc" style="margin-bottom:0.25rem;">
                            <span style="color:var(--text-secondary);font-size:0.8rem;">
                                "USDC deposit is no longer available (escrow closed)"
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                <div class="ticket-action-desc">
                    {if let Some(hours) = deadline_hours {
                        format!("Complete your deposit within {hours} hours of registration to keep your in-person spot.")
                    } else {
                        "Complete your deposit to secure your in-person spot.".to_string()
                    }}
                </div>
                <a href=deposit_href class="btn btn-primary btn-sm ticket-action-btn">
                    <Icon icon=IconName::CreditCard class="icon-sm" />
                    " Pay Deposit Now"
                </a>
            </div>
        </div>
    }
}

/// Deposit verified notice — shown when deposit has been confirmed.
#[component]
pub fn DepositVerifiedCard() -> impl IntoView {
    view! {
        <div class="ticket-action-card ticket-action-card--verified">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Check class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"Deposit: Verified ✓"</div>
            </div>
        </div>
    }
}

/// Deposit pending notice — shown while deposit is being verified.
#[component]
pub fn DepositPendingCard(
    /// Deposit method — controls the messaging
    method: DepositMethod,
) -> impl IntoView {
    let (label, desc) = match method {
        DepositMethod::Thb => (
            "Payment Slip: Pending Verification",
            "Your payment slip has been submitted. We'll verify it shortly — check back in a few minutes.",
        ),
        DepositMethod::Usdc => (
            "Deposit: Pending Confirmation",
            "Your deposit is being confirmed on-chain.",
        ),
        DepositMethod::CreditThb | DepositMethod::CreditUsdc => (
            "Credit Deposit: Pending",
            "Your credit deposit is being processed.",
        ),
    };

    view! {
        <div class="ticket-action-card ticket-action-card--pending">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Hourglass class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">{label}</div>
                <div class="ticket-action-desc">{desc}</div>
            </div>
        </div>
    }
}

/// Refund processed notice — shown when a deposit refund has been completed.
#[component]
pub fn RefundCard(
    /// URL to the refund proof/receipt (empty = hidden)
    #[prop(into)]
    refund_proof_url: String,
) -> impl IntoView {
    let url = refund_proof_url.clone();
    view! {
        <div class="ticket-action-card ticket-action-card--refund">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Recycle class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"Refund: Processed ✓"</div>
                {if !url.is_empty() {
                    view! {
                        <a
                            href=url
                            target="_blank"
                            rel="noopener noreferrer"
                            class="ticket-action-link"
                        >
                            "View Refund Receipt →"
                        </a>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>
        </div>
    }
}

/// NFT claim CTA — shown when attendee is checked in but hasn't claimed their NFT.
#[component]
pub fn ClaimActionCard(
    /// Link to the claim page
    #[prop(into)]
    claim_href: String,
) -> impl IntoView {
    view! {
        <div class="ticket-action-card ticket-action-card--claim">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Gift class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"You're checked in!"</div>
                <a href=claim_href class="btn btn-primary btn-sm ticket-action-btn">
                    <Icon icon=IconName::Gift class="icon-sm" />
                    " Claim Your NFT Badge →"
                </a>
            </div>
        </div>
    }
}

/// Reclaim spot prompt — shown when deposit deadline passed but spots are still available.
#[component]
pub fn ReclaimActionCard(
    /// Link to the deposit page for reclaiming
    #[prop(into)]
    reclaim_href: String,
) -> impl IntoView {
    view! {
        <div class="ticket-action-card ticket-action-card--reclaim">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Warning class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"Deadline Passed — Reclaim Your Spot"</div>
                <div class="ticket-action-desc">
                    "Your deposit deadline has passed and you've been moved to the online track. \
                     However, in-person spots are still available!"
                </div>
                <a href=reclaim_href class="btn btn-success btn-sm ticket-action-btn">
                    <Icon icon=IconName::CreditCard class="icon-sm" />
                    " Deposit Now to Reclaim"
                </a>
            </div>
        </div>
    }
}

/// Moved to online track notice — shown when deposit deadline passed and no in-person spots.
#[component]
pub fn MovedOnlineCard() -> impl IntoView {
    view! {
        <div class="ticket-action-card ticket-action-card--moved-online">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Warning class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"Moved to Online Track"</div>
                <div class="ticket-action-desc">
                    "Your deposit deadline has passed. In-person spots are now full, \
                     so you've been automatically moved to the online track. \
                     You can still claim your NFT after the event."
                </div>
            </div>
        </div>
    }
}

/// Rollover deposit CTA — shown when attendee has a verified deposit on a past event
/// and a new event from the same organizer is available.
#[component]
pub fn RolloverActionCard(
    /// Name of the target event to roll deposit into.
    #[prop(into)]
    target_event_name: String,
    /// Callback invoked when attendee clicks "Roll to next event".
    on_rollover: Box<dyn Fn()>,
) -> impl IntoView {
    view! {
        <div class="ticket-action-card ticket-action-card--rollover">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Refresh class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"Roll Deposit to Next Event"</div>
                <div class="ticket-action-desc">
                    {format!(
                        "Your deposit is ready to roll over to {}. \
                         No extra payment needed — your USDC transfers atomically.",
                        target_event_name
                    )}
                </div>
                <button
                    class="btn btn-primary btn-sm ticket-action-btn"
                    on:click=move |_| on_rollover()
                >
                    <Icon icon=IconName::Refresh class="icon-sm" />
                    " Roll to Next Event"
                </button>
            </div>
        </div>
    }
}
