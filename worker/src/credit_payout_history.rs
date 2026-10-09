//! What the "Paid out" list shows: one row per recorded held-credit payout
//! (`AuditAction::CreditRefundPaidOut` in the global audit log), and who may
//! see it.
//!
//! The audit entry is the only place that records who paid and the slip, so
//! the list reads it rather than the ledger. Visibility follows the payout
//! queue's scope (`credit_payout::payout_scope`):
//! - entries since 2026-10-09 carry `metadata.organizations`, the orgs of the
//!   credit that was paid; the caller must cover all of them;
//! - older entries carry only the slip path, whose `{org}` segment names the
//!   first org; the caller must hold an org with that segment;
//! - an entry with neither is shown to super-admins only.

use event_checkin_domain::models::credit_payout::PayoutScope;
use serde::Serialize;
use serde_json::Value;

use crate::storage::{
    PREFIX_CREDIT_PAYOUTS, credit_payout_org_segment, servable_credit_payout_url,
};

/// How many payouts the list returns, newest first.
pub const HISTORY_LIMIT: usize = 100;

/// One payout as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PayoutRecord {
    /// When it was recorded (audit timestamp, UTC).
    pub paid_at: String,
    /// The person paid (the audit target).
    pub contact: String,
    /// The staff member who recorded it (the audit actor).
    pub paid_by: String,
    pub thb: i64,
    pub usdc: i64,
    /// `organizer` (paid unasked) or `attendee` (from a request).
    pub initiated_by: String,
    /// Staff-only slip link, when a slip was attached.
    pub proof_url: Option<String>,
}

fn proof_of(meta: Option<&Value>) -> Option<&str> {
    meta?.get("proof")?.as_str().filter(|p| !p.is_empty())
}

/// The `{org}` segment of a recorded slip path (`…/credit-payouts/{org}/…`).
fn proof_org_segment(proof: &str) -> Option<&str> {
    let (_, rest) = proof.split_once(PREFIX_CREDIT_PAYOUTS)?;
    rest.split('/').next().filter(|s| !s.is_empty())
}

/// Whether a caller with `scope` may see the payout recorded with `meta`.
pub fn visible(scope: &PayoutScope, meta: Option<&Value>) -> bool {
    if matches!(scope, PayoutScope::All) {
        return true;
    }
    let PayoutScope::Orgs(allowed) = scope else {
        return false;
    };
    if let Some(orgs) = meta
        .and_then(|m| m.get("organizations"))
        .and_then(Value::as_array)
    {
        let ids: Vec<&str> = orgs.iter().filter_map(Value::as_str).collect();
        // `covers` is true for nothing at all; an empty list is not a scope.
        return !ids.is_empty() && scope.covers(ids);
    }
    match proof_of(meta).and_then(proof_org_segment) {
        Some(segment) => allowed
            .iter()
            .any(|org| credit_payout_org_segment(org) == segment),
        None => false,
    }
}

/// The row for one audit entry.
pub fn record_of(timestamp: &str, actor: &str, target: &str, meta: Option<&Value>) -> PayoutRecord {
    let int = |key: &str| {
        meta.and_then(|m| m.get(key))
            .and_then(Value::as_i64)
            .unwrap_or(0)
    };
    PayoutRecord {
        paid_at: timestamp.to_string(),
        contact: target.to_string(),
        paid_by: actor.to_string(),
        thb: int("thb"),
        usdc: int("usdc"),
        initiated_by: meta
            .and_then(|m| m.get("initiated_by"))
            .and_then(Value::as_str)
            .unwrap_or("attendee")
            .to_string(),
        proof_url: proof_of(meta).map(servable_credit_payout_url),
    }
}
