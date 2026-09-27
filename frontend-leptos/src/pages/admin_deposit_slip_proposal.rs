//! The slip agent's shadow-mode proposal, shown on a pending slip
//! (`.plans/033` W1).
//!
//! Advisory only. The line says what the deterministic checks found and which
//! verdict they propose; the organizer's Approve / Reject is still the only
//! decision. It is worded so that nobody reads "accepted" as "approved".

use event_checkin_domain::slip_proposal::{Check, FactSource, Outcome, SlipProposal, Verdict};
use leptos::prelude::*;

fn check_label(check: Check) -> &'static str {
    match check {
        Check::Amount => "amount",
        Check::RefNew => "ref new",
        Check::InWindow => "in window",
        Check::Receiver => "receiver",
    }
}

fn outcome_mark(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pass => "✓",
        Outcome::Fail => "✗",
        Outcome::Unknown => "?",
    }
}

fn verdict_text(verdict: Verdict) -> (&'static str, &'static str) {
    match verdict {
        Verdict::Accepted => ("checks pass", "admin-dep-proposal admin-dep-proposal-pass"),
        Verdict::NeedsReview => ("needs your review", "admin-dep-proposal"),
        Verdict::Rejected => (
            "a check failed",
            "admin-dep-proposal admin-dep-proposal-fail",
        ),
    }
}

fn source_text(source: FactSource) -> &'static str {
    match source {
        FactSource::Qr => "read from the slip QR",
        FactSource::Vision => "read by AI",
    }
}

/// One line per slip: source, each check, the proposed verdict.
#[component]
pub fn SlipProposalLine(proposal: Option<SlipProposal>) -> impl IntoView {
    proposal.map(|p| {
        let checks = p
            .evaluation
            .checks
            .iter()
            .map(|(check, outcome)| format!("{} {}", check_label(*check), outcome_mark(*outcome)))
            .collect::<Vec<_>>()
            .join(" · ");
        let (verdict, class) = verdict_text(p.evaluation.verdict);
        let title = "Advisory, shadow mode: automatic checks on what was read off the slip. \
                     Your Approve / Reject is still the decision.";
        view! {
            <div class=class title=title>
                {format!("Slip check ({}): {checks} → {verdict}", source_text(p.source))}
            </div>
        }
    })
}
