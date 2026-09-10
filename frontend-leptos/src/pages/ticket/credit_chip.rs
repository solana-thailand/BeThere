//! Rolling deposit-credit chip for the ticket page (Issue #061 Phase 1 polish).
//!
//! Shows the attendee their own rolling credit balance — the money side of the
//! hold flow (`HoldDepositCard`). Held credit lives on the org-scoped credit
//! ledger and is auto-applied at the next registration, so without this chip the
//! balance is invisible between events: the attendee holds a deposit, and the
//! next signal they get is a deposit-free registration.
//!
//! Renders **nothing** until the balance loads, and nothing at all when the
//! balance is zero or the read fails — a credit balance is the exception, not
//! the norm, so an empty/failed read must not leave a placeholder on the page.

use leptos::prelude::*;

use crate::api;
use crate::icons::{Icon, IconName};

/// Human-readable balance line, or `None` when there is no credit to show.
///
/// THB is a whole-unit integer; USDC is the 6-decimal smallest unit, so it goes
/// through the shared [`api::format_usdc`] helper rather than being printed raw
/// (a raw `15000000 USDC` would be off by six orders of magnitude).
pub fn credit_balance_label(credit_thb: u64, credit_usdc: u64) -> Option<String> {
    match (credit_thb, credit_usdc) {
        (0, 0) => None,
        (thb, 0) => Some(format!("{thb} THB")),
        (0, usdc) => {
            let usdc = api::format_usdc(usdc);
            Some(format!("{usdc} USDC"))
        }
        (thb, usdc) => {
            let usdc = api::format_usdc(usdc);
            Some(format!("{thb} THB + {usdc} USDC"))
        }
    }
}

/// Deposit-credit chip. Fetches `GET /api/deposit/credit-balance` on mount.
///
/// Mounted on the in-person ticket view once the attendee is checked in — that
/// is the point at which holding a deposit becomes possible, so it is also the
/// point at which a balance is worth surfacing.
#[component]
pub fn CreditBalanceChip() -> impl IntoView {
    let (label, set_label) = signal(None::<String>);

    // On mount: read own balance. A failed read degrades to "render nothing" —
    // the chip is informational, and a broken read must never imply zero credit.
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match api::get_credit_balance().await {
                Ok(balance) => {
                    set_label.set(credit_balance_label(
                        balance.credit_thb,
                        balance.credit_usdc,
                    ));
                }
                Err(e) => {
                    log::warn!("[credit-chip] balance unavailable: {}", e.message);
                }
            }
        });
    });

    view! {
        {move || label.get().map(|text| view! {
            <div class="ticket-credit-chip">
                <Icon icon=IconName::MoneyWings class="icon-sm" />
                <span class="ticket-credit-chip-label">"Deposit Credit"</span>
                <span class="ticket-credit-chip-value">{text}</span>
                <span class="ticket-credit-chip-hint">
                    "Auto-applied to your next registration"
                </span>
            </div>
        })}
    }
}

#[cfg(test)]
mod tests {
    use super::credit_balance_label;

    #[test]
    fn no_credit_renders_nothing() {
        assert_eq!(credit_balance_label(0, 0), None);
    }

    #[test]
    fn thb_only_is_whole_units() {
        assert_eq!(credit_balance_label(500, 0).as_deref(), Some("500 THB"));
    }

    #[test]
    fn usdc_only_is_scaled_from_smallest_unit() {
        assert_eq!(
            credit_balance_label(0, 15_000_000).as_deref(),
            Some("15.00 USDC")
        );
    }

    #[test]
    fn both_currencies_are_combined() {
        assert_eq!(
            credit_balance_label(500, 1_500_000).as_deref(),
            Some("500 THB + 1.50 USDC")
        );
    }
}
