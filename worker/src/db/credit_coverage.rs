//! Atomic rolling-credit application and deposit projection.
//!
//! The credit ledger is the money source of truth. Applying credit and exposing
//! the verified deposit to the ticket flow must therefore commit together.

use event_checkin_domain::models::deposit::DepositMethod;
use worker::D1Database;
use worker::d1::D1Type;

const CREDIT_MARKER: &str = "ROLLING_CREDIT_AUTO_APPLIED";
const CREDIT_ACTOR: &str = "SYSTEM_ROLLING_CREDIT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyCreditOutcome {
    Covered { newly_spent: bool },
    Insufficient,
    ConflictingDeposit,
}

pub struct ApplyCredit<'a> {
    pub email: &'a str,
    pub organization_id: &'a str,
    pub event_id: &'a str,
    pub attendee_id: &'a str,
    pub attendee_name: &'a str,
    pub currency: &'a str,
    pub amount: u64,
    pub apply_key: &'a str,
    pub recorded_at: &'a str,
}

#[derive(serde::Deserialize)]
struct CoverageState {
    authorized: i64,
    deposit_ready: i64,
    status_ready: i64,
    conflicting: i64,
}

/// Spend rolling credit and create the verified deposit projection in one D1
/// transaction. Repeating the same application is safe and repairs projections
/// left by the pre-atomic implementation without spending credit twice.
pub async fn apply(db: &D1Database, input: &ApplyCredit<'_>) -> Result<ApplyCreditOutcome, String> {
    if input.amount == 0 || input.amount > i32::MAX as u64 {
        return Err("credit amount must fit a positive D1 INTEGER binding".to_string());
    }
    let email = input.email.trim().to_lowercase();
    let currency = input.currency.trim().to_lowercase();
    let (method, display_currency) = match currency.as_str() {
        "thb" => (DepositMethod::CreditThb, "THB"),
        "usdc" => (DepositMethod::CreditUsdc, "USDC"),
        _ => return Err(format!("unsupported credit currency '{currency}'")),
    };
    let method = method.to_string();
    let amount = input.amount as i32;

    // The spend guard refuses to consume credit when another payment already
    // owns this attendee/event. Existing matching credit rows are allowed so a
    // retry can heal an incomplete projection from the old multi-write flow.
    let spend = db
        .prepare(
            "INSERT INTO credit_ledger \
             (email, organization_id, currency, delta, reason, event_id, deposit_id) \
             SELECT ?1, ?2, ?3, -1 * ?4, 'apply', ?5, ?6 \
             WHERE (SELECT COALESCE(SUM(delta), 0) FROM credit_ledger \
                    WHERE email = ?1 AND organization_id = ?2 AND currency = ?3) >= ?4 \
               AND NOT EXISTS (SELECT 1 FROM deposit_statuses s \
                 WHERE s.event_id = ?5 AND s.attendee_id = ?7 AND s.method <> ?8 \
                   AND NOT (?8='credit_thb' AND s.method='thb' AND EXISTS(SELECT 1 FROM thb_deposits d \
                     WHERE d.event_id=?5 AND d.attendee_id=?7 AND d.slip_url='ROLLING_CREDIT_AUTO_APPLIED'))) \
               AND (?3 <> 'thb' OR NOT EXISTS (SELECT 1 FROM thb_deposits \
                    WHERE event_id = ?5 AND attendee_id = ?7 \
                      AND COALESCE(slip_url, '') <> 'ROLLING_CREDIT_AUTO_APPLIED')) \
             ON CONFLICT (deposit_id, reason) WHERE deposit_id IS NOT NULL DO NOTHING",
        )
        .bind_refs(&[
            D1Type::Text(&email),
            D1Type::Text(input.organization_id),
            D1Type::Text(&currency),
            D1Type::Integer(amount),
            D1Type::Text(input.event_id),
            D1Type::Text(input.apply_key),
            D1Type::Text(input.attendee_id),
            D1Type::Text(&method),
        ])
        .map_err(|e| format!("D1 atomic credit spend bind: {e:?}"))?;

    let mut statements = vec![spend];
    if currency == "thb" {
        // thb_deposits predates its composite uniqueness invariant. The batch's
        // preceding ledger write serializes this workflow; NOT EXISTS keeps the
        // insert idempotent while preserving every real cash row.
        statements.push(
            db.prepare(
                "INSERT INTO thb_deposits \
                 (attendee_id,event_id,amount_thb,slip_url,verified,verified_by,verified_at,uploaded_at,refunded,attendee_name,held_as_credit) \
                 SELECT ?1,?2,?3,'ROLLING_CREDIT_AUTO_APPLIED',1,'SYSTEM_ROLLING_CREDIT',?4,?4,0,?5,0 \
                 WHERE EXISTS (SELECT 1 FROM credit_ledger WHERE deposit_id=?6 AND reason='apply' \
                   AND email=?7 AND organization_id=?8 AND currency='thb' AND delta=-1*?3 AND event_id=?2) \
                   AND NOT EXISTS (SELECT 1 FROM thb_deposits WHERE event_id=?2 AND attendee_id=?1)",
            )
            .bind_refs(&[
                D1Type::Text(input.attendee_id), D1Type::Text(input.event_id),
                D1Type::Integer(amount), D1Type::Text(input.recorded_at),
                D1Type::Text(input.attendee_name), D1Type::Text(input.apply_key),
                D1Type::Text(&email), D1Type::Text(input.organization_id),
            ])
            .map_err(|e| format!("D1 credit THB projection insert bind: {e:?}"))?,
        );
        statements.push(
            db.prepare(
                "UPDATE thb_deposits SET amount_thb=?1,verified=1,verified_by=?2,verified_at=?3,attendee_name=?4 \
                 WHERE event_id=?5 AND attendee_id=?6 AND slip_url=?7 \
                   AND EXISTS (SELECT 1 FROM credit_ledger WHERE deposit_id=?8 AND reason='apply' \
                     AND email=?9 AND organization_id=?10 AND currency='thb' AND delta=-1*?1 AND event_id=?5)",
            )
            .bind_refs(&[
                D1Type::Integer(amount), D1Type::Text(CREDIT_ACTOR),
                D1Type::Text(input.recorded_at), D1Type::Text(input.attendee_name),
                D1Type::Text(input.event_id), D1Type::Text(input.attendee_id),
                D1Type::Text(CREDIT_MARKER), D1Type::Text(input.apply_key),
                D1Type::Text(&email), D1Type::Text(input.organization_id),
            ])
            .map_err(|e| format!("D1 credit THB projection repair bind: {e:?}"))?,
        );
    }

    statements.push(
        db.prepare(
            "INSERT INTO deposit_statuses \
             (attendee_id,event_id,method,amount,currency,verified,deposited_at,deposit_order,refundable,rejected) \
             SELECT ?1,?2,?3,?4,?5,1,?6,0,0,0 \
             WHERE EXISTS (SELECT 1 FROM credit_ledger WHERE deposit_id=?7 AND reason='apply' \
               AND email=?8 AND organization_id=?9 AND currency=?10 AND delta=-1*?4 AND event_id=?2) \
             ON CONFLICT(event_id,attendee_id) DO UPDATE SET \
               method=excluded.method,amount=excluded.amount,currency=excluded.currency,verified=1,deposited_at=excluded.deposited_at,refundable=0,rejected=0 \
             WHERE deposit_statuses.method=excluded.method OR \
               (excluded.method='credit_thb' AND deposit_statuses.method='thb' AND EXISTS(SELECT 1 FROM thb_deposits d \
                 WHERE d.event_id=excluded.event_id AND d.attendee_id=excluded.attendee_id AND d.slip_url='ROLLING_CREDIT_AUTO_APPLIED'))",
        )
        .bind_refs(&[
            D1Type::Text(input.attendee_id), D1Type::Text(input.event_id),
            D1Type::Text(&method), D1Type::Integer(amount), D1Type::Text(display_currency),
            D1Type::Text(input.recorded_at), D1Type::Text(input.apply_key),
            D1Type::Text(&email), D1Type::Text(input.organization_id), D1Type::Text(&currency),
        ])
        .map_err(|e| format!("D1 credit deposit status bind: {e:?}"))?,
    );

    statements.push(
        db.prepare(
            "SELECT \
               EXISTS(SELECT 1 FROM credit_ledger WHERE deposit_id=?1 AND reason='apply' AND email=?2 \
                 AND organization_id=?3 AND currency=?4 AND delta=-1*?5 AND event_id=?6) AS authorized, \
               (?4<>'thb' OR EXISTS(SELECT 1 FROM thb_deposits WHERE event_id=?6 AND attendee_id=?7 \
                 AND slip_url='ROLLING_CREDIT_AUTO_APPLIED' AND amount_thb=?5 AND verified=1)) AS deposit_ready, \
               EXISTS(SELECT 1 FROM deposit_statuses WHERE event_id=?6 AND attendee_id=?7 AND method=?8 \
                 AND amount=?5 AND currency=?9 AND verified=1 AND refundable=0) AS status_ready, \
               (EXISTS(SELECT 1 FROM deposit_statuses WHERE event_id=?6 AND attendee_id=?7 AND method<>?8) \
                 OR (?4='thb' AND EXISTS(SELECT 1 FROM thb_deposits WHERE event_id=?6 AND attendee_id=?7 \
                   AND COALESCE(slip_url,'')<>'ROLLING_CREDIT_AUTO_APPLIED'))) AS conflicting",
        )
        .bind_refs(&[
            D1Type::Text(input.apply_key), D1Type::Text(&email),
            D1Type::Text(input.organization_id), D1Type::Text(&currency),
            D1Type::Integer(amount), D1Type::Text(input.event_id),
            D1Type::Text(input.attendee_id), D1Type::Text(&method),
            D1Type::Text(display_currency),
        ])
        .map_err(|e| format!("D1 credit coverage verification bind: {e:?}"))?,
    );

    let results = db
        .batch(statements)
        .await
        .map_err(|e| format!("D1 atomic credit coverage batch: {e:?}"))?;
    let newly_spent = results
        .first()
        .and_then(|r| r.meta().ok().flatten())
        .and_then(|m| m.changes)
        .unwrap_or(0)
        > 0;
    let state = results
        .last()
        .ok_or_else(|| "D1 credit coverage batch returned no verification result".to_string())?
        .results::<CoverageState>()
        .map_err(|e| format!("D1 credit coverage verification decode: {e:?}"))?
        .into_iter()
        .next()
        .ok_or_else(|| "D1 credit coverage verification returned no row".to_string())?;

    if state.conflicting != 0 {
        return Ok(ApplyCreditOutcome::ConflictingDeposit);
    }
    if state.authorized == 0 {
        return Ok(ApplyCreditOutcome::Insufficient);
    }
    if state.deposit_ready == 0 || state.status_ready == 0 {
        return Err(
            "credit was authorized but its verified deposit projection is incomplete".to_string(),
        );
    }
    Ok(ApplyCreditOutcome::Covered { newly_spent })
}
