//! Admin dashboard state types and the roster's pure decisions.

use leptos::prelude::*;

use crate::api::AttendeeListItem;
use crate::components::{self, ToastType};
use crate::utils;

// ===== Tab Type =====

/// Admin dashboard section selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum AdminSection {
    Attendance,
    Deposits,
    Escrow,
    Cancellation,
    Feedback,
    Quiz,
    FormBuilder,
    Adventure,
    Campaigns,
    Events,
}

/// Dashboard tab selection (within Attendance section).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum DashboardTab {
    InPerson,
    Online,
}

impl DashboardTab {
    pub(super) fn label(&self) -> &'static str {
        match self {
            DashboardTab::InPerson => "In-Person",
            DashboardTab::Online => "Online",
        }
    }

    /// Whether an attendee belongs to this tab.
    pub(super) fn matches(&self, participation_type: &str) -> bool {
        match self {
            DashboardTab::InPerson => utils::is_on_site_roster(participation_type),
            DashboardTab::Online => !utils::is_on_site_roster(participation_type),
        }
    }
}

// ===== Filter Pills =====

/// Attendee list filter pill selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum FilterPill {
    All,
    CheckedIn,
    NotCheckedIn,
    Vip,
    Walkin,
}

impl FilterPill {
    /// Whether an attendee passes this filter.
    pub(super) fn matches(&self, a: &AttendeeListItem) -> bool {
        match self {
            FilterPill::All => true,
            FilterPill::CheckedIn => a.checked_in_at.is_some(),
            FilterPill::NotCheckedIn => a.checked_in_at.is_none(),
            FilterPill::Vip => a.ticket_name.to_lowercase().contains("vip"),
            FilterPill::Walkin => a.ticket_name.eq_ignore_ascii_case("Walk-in"),
        }
    }
}

/// Check if a ticket name indicates VIP status.
pub(super) fn is_vip_ticket(ticket_name: &str) -> bool {
    ticket_name.to_lowercase().contains("vip")
}

/// Which deposit badge an in-person attendee gets, if any.
///
/// Pulled out of the row view so it can be tested: this is the line the
/// organizer reads at the door to decide whether to chase someone for money.
///
/// **Order is settled-first, deliberately.** A deposit that is refunded,
/// comped or credit-covered is DONE, and calling any of them "pending" sends
/// an organizer after money nobody owes.
///
/// `.issues/137` — this used to consult only `deposit_amount` (USDC) and
/// `deposit_verified` (derived from `attendees.deposit_status`). **Nothing in
/// the THB flow writes either**: `save_deposit_status_to_d1` is dead code with
/// zero callers. So a staff comp and a credit-covered registration were
/// indistinguishable from an unpaid attendee, and the roster said "Deposit
/// pending" for people who owed nothing. The `thb_*` fields are the roster's
/// first look at `thb_deposits`.
pub(super) fn deposit_badge_for(
    a: &AttendeeListItem,
    is_in_person: bool,
    deposit_required: bool,
) -> Option<(&'static str, &'static str)> {
    if !is_in_person {
        return None; // online attendees don't deposit
    }
    // A zero amount is not a deposit. `Some("0")` is `is_some()`, which is
    // exactly how a staff comp (deposit_amount_usdc = 0, not NULL) fell
    // through to "Deposit pending".
    let has_deposit = a
        .deposit_amount
        .as_deref()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .is_some_and(|v| v > 0.0);
    let is_deposit_verified = a.deposit_verified.as_deref() == Some("true");
    let thb = a.thb_source.as_deref();

    match () {
        _ if a.refund_status.is_some() || a.thb_refunded => {
            Some(("badge badge-refunded", "Refunded"))
        }
        // Staff waive, or an organizer's "Admit, no refund owed". Shown
        // distinctly from a paid deposit because the refund obligation is
        // precisely what differs.
        _ if thb == Some("comp") => Some(("badge badge-info", "Comp \u{2713}")),
        _ if thb == Some("credit") || a.used_credit => {
            Some(("badge badge-info", "Credit \u{2713}"))
        }
        _ if a.thb_verified || is_deposit_verified => {
            Some(("badge badge-success", "Deposit \u{2713}"))
        }
        // A slip IS in, but nobody has checked it. Waiting on the ORGANIZER.
        _ if thb.is_some() || has_deposit => Some(("badge badge-warning", "Deposit pending")),
        // Nothing submitted at all, on an event that requires a deposit.
        // Waiting on the ATTENDEE — a different action, so a different badge.
        //
        // This state had no badge of its own until 2026-09-23. It was covered
        // by accident: `deposit_amount` is `Some("0")` for most rows, so the
        // old `is_some()` check painted everyone "Deposit pending" — wrong for
        // comps and credit users, but right often enough that the organizer
        // relied on it to see who still owed money. Fixing that
        // (`.issues/137`) removed the accident and the signal with it, which
        // the owner noticed immediately.
        //
        // "No slip yet" rather than "Not paid": the system knows a slip is
        // absent, not that the money is. `.issues/138` is exactly that case —
        // people HAD transferred and could not upload, and a roster calling
        // them unpaid would have been both wrong and accusatory.
        _ if deposit_required => Some(("badge badge-danger", "No slip yet")),
        _ => None,
    }
}

/// The two signals nearly every admin action writes: the toast and the
/// reload trigger.
#[derive(Clone, Copy)]
pub(super) struct Notify {
    pub(super) set_toast: WriteSignal<Option<components::ToastMessage>>,
    pub(super) set_refresh_counter: WriteSignal<u32>,
}

impl Notify {
    pub(super) fn toast(&self, msg: &str, kind: ToastType) {
        components::show_toast(&self.set_toast, msg, kind);
    }

    /// Bump the refresh counter so the dashboard reloads its data.
    pub(super) fn reload(&self) {
        self.set_refresh_counter.update(|c| *c += 1);
    }
}

#[cfg(test)]
mod deposit_badge_tests {
    use super::*;

    /// A roster row with nothing settled. Each test flips only what it means to.
    fn row() -> AttendeeListItem {
        AttendeeListItem {
            api_id: "a1".into(),
            name: "Somchai".into(),
            email: "s@example.com".into(),
            ..Default::default()
        }
    }

    /// Deposit-enabled event, the normal case for an in-person roster.
    fn label(a: &AttendeeListItem) -> Option<&'static str> {
        deposit_badge_for(a, true, true).map(|(_, text)| text)
    }

    /// The reported bug: a staff comp carries `deposit_amount_usdc = 0` — a
    /// ZERO, not a NULL — so `deposit_amount.is_some()` was true and the row
    /// read "Deposit pending" forever. Two of RTM#6's in-person rows were in
    /// exactly this state on 2026-09-23.
    #[test]
    fn a_staff_comp_is_not_pending() {
        let mut a = row();
        a.deposit_amount = Some("0".into()); // the zero that used to lie
        a.thb_source = Some("comp".into());
        a.thb_verified = true;
        assert_eq!(label(&a), Some("Comp \u{2713}"));
    }

    /// Seven of RTM#6's rows were credit-covered with
    /// `attendees.deposit_status = 'none'`. They only escaped "pending" via a
    /// separate ledger annotation; if that query ever fails, `thb_source`
    /// must still carry them.
    #[test]
    fn a_credit_covered_registration_is_not_pending() {
        let mut a = row();
        a.thb_source = Some("credit".into());
        a.thb_verified = true;
        assert_eq!(label(&a), Some("Credit \u{2713}"));

        // ...and still correct when the ledger annotation is the only signal.
        let mut b = row();
        b.used_credit = true;
        assert_eq!(label(&b), Some("Credit \u{2713}"));
    }

    /// The state that SHOULD say pending: a slip is in, nobody has checked it.
    #[test]
    fn an_unverified_slip_is_pending() {
        let mut a = row();
        a.thb_source = Some("cash".into());
        a.thb_verified = false;
        assert_eq!(label(&a), Some("Deposit pending"));
    }

    #[test]
    fn a_verified_cash_deposit_is_done() {
        let mut a = row();
        a.thb_source = Some("cash".into());
        a.thb_verified = true;
        assert_eq!(label(&a), Some("Deposit \u{2713}"));
    }

    /// Settled-first ordering: refunded outranks everything, because chasing a
    /// refunded attendee for money is the worst of the wrong answers.
    #[test]
    fn refunded_outranks_every_other_state() {
        for source in ["cash", "credit", "comp"] {
            let mut a = row();
            a.thb_source = Some(source.into());
            a.thb_verified = true;
            a.thb_refunded = true;
            assert_eq!(label(&a), Some("Refunded"), "source={source}");
        }
    }

    /// Nobody has submitted anything, on an event that requires a deposit:
    /// this is the person the organizer still has to chase, and it must be
    /// visible. Losing it was the regression the owner reported the same day
    /// `.issues/137` shipped.
    #[test]
    fn an_attendee_who_has_submitted_nothing_is_flagged() {
        assert_eq!(label(&row()), Some("No slip yet"));
        // A zero USDC amount is not a deposit — same state.
        let mut a = row();
        a.deposit_amount = Some("0".into());
        assert_eq!(label(&a), Some("No slip yet"));
        // ...and an empty string must not parse into something truthy.
        let mut b = row();
        b.deposit_amount = Some("".into());
        assert_eq!(label(&b), Some("No slip yet"));
    }

    /// The two "needs action" states are NOT the same and must not share a
    /// badge: one waits on the organizer to verify, the other waits on the
    /// attendee to send anything at all.
    #[test]
    fn nothing_submitted_reads_differently_from_an_unchecked_slip() {
        let nothing = label(&row());
        let mut submitted = row();
        submitted.thb_source = Some("cash".into());
        submitted.thb_verified = false;
        assert_eq!(nothing, Some("No slip yet"));
        assert_eq!(label(&submitted), Some("Deposit pending"));
        assert_ne!(nothing, label(&submitted));
    }

    /// An event with no deposit at all must not accuse anyone of owing one.
    #[test]
    fn an_event_without_deposits_flags_nobody() {
        assert_eq!(deposit_badge_for(&row(), true, false), None);
        // ...but a deposit that somehow exists is still reported.
        let mut a = row();
        a.thb_source = Some("cash".into());
        a.thb_verified = true;
        assert_eq!(
            deposit_badge_for(&a, true, false).map(|(_, t)| t),
            Some("Deposit \u{2713}")
        );
    }

    /// A real USDC deposit still reads pending — the escrow path is unchanged
    /// by this fix and must not regress.
    #[test]
    fn a_usdc_deposit_still_reads_pending_then_verified() {
        let mut a = row();
        a.deposit_amount = Some("500".into());
        assert_eq!(label(&a), Some("Deposit pending"));
        a.deposit_verified = Some("true".into());
        assert_eq!(label(&a), Some("Deposit \u{2713}"));
    }

    /// Online attendees never deposit, whatever the columns say.
    #[test]
    fn online_attendees_never_get_a_deposit_badge() {
        let mut a = row();
        a.thb_source = Some("cash".into());
        a.deposit_amount = Some("500".into());
        assert_eq!(deposit_badge_for(&a, false, true), None);
    }
}
