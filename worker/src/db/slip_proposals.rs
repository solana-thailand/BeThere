//! `slip_proposals` — the slip agent's shadow-mode record (migration 0054,
//! `.plans/033` W1). Read by the admin slip list; never by a money path.

use std::collections::HashMap;

use event_checkin_domain::slip_proposal::{
    Check, Evaluation, FactSource, Outcome, SlipFacts, SlipProposal, Verdict,
};
use worker::D1Database;
use worker::d1::D1Type;

use super::d1_int::uint_bind;

/// Why a proposal was not stored.
#[derive(Debug)]
pub enum SaveError {
    /// Another row already owns `bank_ref`: the reference was claimed between
    /// the caller's [`ref_claimed_elsewhere`] and this insert.
    RefTaken,
    /// Anything else D1 refused.
    Other(String),
}

/// Whether a row other than `(event_id, attendee_id)` carries `claimed_ref`.
pub async fn ref_claimed_elsewhere(
    db: &D1Database,
    claimed_ref: &str,
    event_id: &str,
    attendee_id: &str,
) -> Result<bool, String> {
    let stmt = db
        .prepare(
            "SELECT 1 AS hit FROM slip_proposals \
             WHERE claimed_ref = ?1 AND NOT (event_id = ?2 AND attendee_id = ?3) LIMIT 1",
        )
        .bind_refs(&[
            D1Type::Text(claimed_ref),
            D1Type::Text(event_id),
            D1Type::Text(attendee_id),
        ])
        .map_err(|e| format!("D1 slip_proposals ref_claimed_elsewhere bind: {e:?}"))?;
    let rows = super::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 slip_proposals ref_claimed_elsewhere: {e}"))?;
    Ok(!rows.is_empty())
}

/// Insert or replace the proposal for `(event_id, attendee_id)`.
///
/// `owns_ref` sets `bank_ref` (the UNIQUE column) to the claimed reference;
/// pass it only when the reference was not claimed elsewhere.
pub async fn upsert(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
    proposal: &SlipProposal,
    owns_ref: bool,
) -> Result<(), SaveError> {
    let checks = serde_json::to_string(&proposal.evaluation.checks)
        .map_err(|e| SaveError::Other(format!("slip_proposals checks json: {e}")))?;
    let facts = &proposal.facts;
    let transferred_at = facts.transferred_at.map(|t| t.to_rfc3339());
    let amount = match facts.amount_satang {
        None => D1Type::Null,
        Some(satang) => uint_bind("slip_proposals.amount_satang", satang)
            .map_err(|e| SaveError::Other(e.to_string()))?,
    };
    let owned_ref = facts.bank_ref.as_deref().filter(|_| owns_ref);

    let stmt = db
        .prepare(
            "INSERT INTO slip_proposals (event_id, attendee_id, source, model, claimed_ref, \
             bank_ref, amount_satang, transferred_at, receiver_account, checks, verdict, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) \
             ON CONFLICT (event_id, attendee_id) DO UPDATE SET \
             source = excluded.source, model = excluded.model, claimed_ref = excluded.claimed_ref, \
             bank_ref = excluded.bank_ref, amount_satang = excluded.amount_satang, \
             transferred_at = excluded.transferred_at, receiver_account = excluded.receiver_account, \
             checks = excluded.checks, verdict = excluded.verdict, created_at = excluded.created_at",
        )
        .bind_refs(&[
            D1Type::Text(event_id),
            D1Type::Text(attendee_id),
            D1Type::Text(proposal.source.as_str()),
            text_or_null(proposal.model.as_deref()),
            text_or_null(facts.bank_ref.as_deref()),
            text_or_null(owned_ref),
            amount,
            text_or_null(transferred_at.as_deref()),
            text_or_null(facts.receiver_account.as_deref()),
            D1Type::Text(&checks),
            D1Type::Text(proposal.evaluation.verdict.as_str()),
            D1Type::Text(&proposal.created_at),
        ])
        .map_err(|e| SaveError::Other(format!("D1 slip_proposals upsert bind: {e:?}")))?;

    match stmt.run().await {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = format!("{e:?}");
            match msg.contains("UNIQUE constraint failed") && msg.contains("bank_ref") {
                true => Err(SaveError::RefTaken),
                false => Err(SaveError::Other(format!("D1 slip_proposals upsert: {msg}"))),
            }
        }
    }
}

/// Every proposal on `event_id`, keyed by attendee id. A row that no longer
/// parses (an unknown check or verdict) is skipped, not guessed at.
pub async fn by_event(
    db: &D1Database,
    event_id: &str,
) -> Result<HashMap<String, SlipProposal>, String> {
    let stmt = db
        .prepare("SELECT * FROM slip_proposals WHERE event_id = ?1")
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 slip_proposals by_event bind: {e:?}"))?;
    let rows = super::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 slip_proposals by_event: {e}"))?;
    Ok(rows.iter().filter_map(row_to_proposal).collect())
}

fn row_to_proposal(row: &serde_json::Value) -> Option<(String, SlipProposal)> {
    let text = |key: &str| row.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let attendee_id = text("attendee_id")?;
    let source: FactSource = serde_json::from_value(row.get("source")?.clone()).ok()?;
    let verdict: Verdict = serde_json::from_value(row.get("verdict")?.clone()).ok()?;
    let checks: Vec<(Check, Outcome)> = serde_json::from_str(&text("checks")?).ok()?;
    let facts = SlipFacts {
        bank_ref: text("claimed_ref"),
        amount_satang: row
            .get("amount_satang")
            .and_then(|v| v.as_f64())
            .and_then(|f| match f >= 0.0 && f.fract() == 0.0 {
                true => Some(f as u64),
                false => None,
            }),
        transferred_at: text("transferred_at")
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
            .map(|t| t.with_timezone(&chrono::Utc)),
        receiver_account: text("receiver_account"),
    };
    let proposal = SlipProposal {
        source,
        model: text("model"),
        facts,
        evaluation: Evaluation { checks, verdict },
        created_at: text("created_at")?,
    };
    Some((attendee_id, proposal))
}

/// Delete every proposal on `event_id` (runs with the deposit purge).
pub async fn delete_for_event(db: &D1Database, event_id: &str) -> Result<(), String> {
    db.prepare("DELETE FROM slip_proposals WHERE event_id = ?1")
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 slip_proposals delete_for_event bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 slip_proposals delete_for_event: {e:?}"))?;
    Ok(())
}

/// Delete one attendee's proposal (runs with the deposit delete).
pub async fn delete_one(db: &D1Database, event_id: &str, attendee_id: &str) -> Result<(), String> {
    db.prepare("DELETE FROM slip_proposals WHERE event_id = ?1 AND attendee_id = ?2")
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(attendee_id)])
        .map_err(|e| format!("D1 slip_proposals delete_one bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 slip_proposals delete_one: {e:?}"))?;
    Ok(())
}

fn text_or_null(value: Option<&str>) -> D1Type<'_> {
    value.map_or(D1Type::Null, D1Type::Text)
}
