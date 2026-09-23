//! Rolling deposit-credit chip for the ticket page (Issue #061 Phase 1 polish).
//!
//! Shows the attendee their own rolling credit balance — the money side of the
//! hold flow (`HoldDepositCard`). Held credit lives on the org-scoped credit
//! ledger and is auto-applied at the next registration, so without this chip the
//! balance is invisible between events: the attendee holds a deposit, and the
//! next signal they get is a deposit-free registration.
//!
//! Renders **nothing** until the balance loads, and nothing at all when the
//! balance is zero *and* nothing is locked, or the read fails — a credit
//! balance is the exception, not the norm, so an empty/failed read must not
//! leave a placeholder on the page.
//!
//! Two mounts share one fetch and one markup builder:
//!
//! * [`CreditBalanceChip`] — the ticket page, where the refund card is already
//!   rendered separately;
//! * [`CreditWallet`] — My Registrations, where this is the *only* place a
//!   holder whose credit has rolled on to a later event can still find it and
//!   ask for it back (issue #120 §1).

use leptos::prelude::*;

use crate::api;
use crate::api::{CreditBalanceResponse, LockedCredit};
use crate::icons::{Icon, IconName};
use crate::pages::ticket::action_cards::RequestCreditRefundCard;

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

/// The amount one event is holding, in its own currency.
///
/// THB is whole units; USDC is the 6-decimal smallest unit and goes through the
/// shared helper, same rule as [`credit_balance_label`]. A negative amount
/// cannot happen (the query negates an `apply`), but clamping keeps the cast
/// honest rather than wrapping into a huge USDC figure.
pub fn locked_amount_label(locked: &LockedCredit) -> String {
    let amount = locked.amount.max(0);
    match locked.currency.as_str() {
        "usdc" => format!("{} USDC", api::format_usdc(amount as u64)),
        _ => format!("{amount} THB"),
    }
}

/// "500 THB is covering RTM #6" — one line per event still holding credit.
///
/// The event name comes from the D1 mirror and can be missing; the id is a
/// slug, so it still reads as something the attendee can recognise.
pub fn locked_credit_label(locked: &LockedCredit) -> String {
    let event = match locked.event_name.is_empty() {
        true => locked.event_id.as_str(),
        false => locked.event_name.as_str(),
    };
    format!("{} is covering {event}", locked_amount_label(locked))
}

/// The chip markup for a loaded balance, or `None` when there is nothing to
/// say — no spendable credit and nothing locked.
///
/// Locked credit is listed under the balance rather than added to it: it is the
/// attendee's money, but it cannot be spent or refunded until the event it
/// covers ends, and showing one number for both would make "0 THB" appear the
/// moment the credit rolls forward.
fn chip_view(balance: &CreditBalanceResponse) -> Option<AnyView> {
    let label = credit_balance_label(balance.credit_thb, balance.credit_usdc);
    if label.is_none() && balance.locked.is_empty() {
        return None;
    }
    let locked: Vec<String> = balance.locked.iter().map(locked_credit_label).collect();
    Some(
        view! {
            <div class="ticket-credit-chip">
                <Icon icon=IconName::MoneyWings class="icon-sm" />
                <span class="ticket-credit-chip-label">"Deposit Credit"</span>
                {label.map(|text| view! {
                    <span class="ticket-credit-chip-value">{text}</span>
                    <span class="ticket-credit-chip-hint">
                        "Auto-applied to your next registration"
                    </span>
                })}
                {(!locked.is_empty()).then(|| view! {
                    <ul class="ticket-credit-chip-locked">
                        {locked.into_iter().map(|line| view! {
                            <li class="ticket-credit-chip-locked-item">
                                {line}
                                <span class="ticket-credit-chip-hint">
                                    " — it returns when that event ends"
                                </span>
                            </li>
                        }).collect::<Vec<_>>()}
                    </ul>
                })}
            </div>
        }
        .into_any(),
    )
}

/// Fetch the caller's own balance once on mount.
///
/// Signed out or a failed read degrades to `None` — "render nothing" — because
/// every surface built on this is informational, and a broken read must never
/// imply zero credit. `get_credit_balance` never redirects: the ticket page is
/// public, and a signed-out attendee is the normal case there (`.issues/142`).
fn balance_signal(tag: &'static str) -> ReadSignal<Option<CreditBalanceResponse>> {
    let (balance, set_balance) = signal(None::<CreditBalanceResponse>);
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match api::get_credit_balance().await {
                Some(loaded) => set_balance.set(Some(loaded)),
                None => log::debug!("[{tag}] no balance (signed out or read failed)"),
            }
        });
    });
    balance
}

/// Deposit-credit chip. Fetches `GET /api/deposit/credit-balance` on mount.
///
/// Mounted on the in-person ticket view once the attendee is checked in — that
/// is the point at which holding a deposit becomes possible, so it is also the
/// point at which a balance is worth surfacing.
#[component]
pub fn CreditBalanceChip() -> impl IntoView {
    let balance = balance_signal("credit-chip");
    view! { {move || balance.get().as_ref().and_then(chip_view)} }
}

/// Deposit credit wherever the holder is signed in, with the way out attached.
///
/// The ticket page only shows the chip on the event whose deposit was held, and
/// only once checked in, so everyone whose credit has since rolled forward had
/// to find an old ticket to see their money or ask for it back (issue #120 §1).
/// The backend is already cross-event, so this needs no new endpoint.
///
/// The refund card renders whenever there is credit **or** a lock: a holder
/// whose whole balance is committed to an upcoming event is exactly the person
/// most likely to want out, and the organizer's queue now keeps that request
/// open until the event releases the money rather than clearing it to nothing.
#[component]
pub fn CreditWallet() -> impl IntoView {
    let balance = balance_signal("credit-wallet");
    view! {
        {move || balance.get().as_ref().and_then(|loaded| {
            chip_view(loaded).map(|chip| view! {
                <section class="landing-credit-wallet">
                    {chip}
                    <RequestCreditRefundCard />
                </section>
            }.into_any())
        })}
    }
}

#[cfg(test)]
mod tests {
    use super::{credit_balance_label, locked_amount_label, locked_credit_label};
    use crate::api::LockedCredit;

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
    fn locked_thb_names_the_event_holding_it() {
        let locked = LockedCredit {
            event_id: "rtm-6".to_string(),
            event_name: "RTM #6".to_string(),
            currency: "thb".to_string(),
            amount: 500,
            event_end_ms: 0,
        };
        assert_eq!(locked_credit_label(&locked), "500 THB is covering RTM #6");
    }

    #[test]
    fn locked_falls_back_to_the_event_id() {
        // The D1 events mirror can be missing the row; the id is a slug, so it
        // still reads as something the attendee recognises.
        let locked = LockedCredit {
            event_id: "rtm-6".to_string(),
            currency: "thb".to_string(),
            amount: 500,
            ..Default::default()
        };
        assert_eq!(locked_credit_label(&locked), "500 THB is covering rtm-6");
    }

    #[test]
    fn locked_usdc_is_scaled_from_smallest_unit() {
        let locked = LockedCredit {
            currency: "usdc".to_string(),
            amount: 15_000_000,
            ..Default::default()
        };
        assert_eq!(locked_amount_label(&locked), "15.00 USDC");
    }

    #[test]
    fn both_currencies_are_combined() {
        assert_eq!(
            credit_balance_label(500, 1_500_000).as_deref(),
            Some("500 THB + 1.50 USDC")
        );
    }
}
