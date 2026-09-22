//! Age guard for the notification queue (`.issues/128`).
//!
//! A message that has sat in the outbox past its kind's useful life is not a
//! late message, it is a wrong one — and a queue that has never drained holds
//! enough of them that the first working cron becomes a mass mailing. Measured
//! on production on 2026-09-21: 268 pending rows, of which `cancel.sql` retires
//! 25, leaving 243 that would have gone out, ~187 of them surveys for events
//! that ended weeks earlier.
//!
//! The ages live in `policy::max_age_secs`; this module only splices them into
//! the statements and decides what a run does with the result.

use super::policy::{MAX_AGE_TOKEN, StalenessMode, max_age_case_sql};
use crate::db::d1_safe::safe_all_rows;
use serde_json::Value;
use worker::D1Database;

/// `sql/claim.sql` with the per-kind age table spliced in.
pub(super) fn claim_sql() -> String {
    render(include_str!("sql/claim.sql"), "n.kind")
}

fn render(template: &str, column: &str) -> String {
    template.replace(MAX_AGE_TOKEN, &max_age_case_sql(column))
}

/// Retire or count the stale rows, before the claim loop runs.
///
/// Ordering matters: `cancel` has to happen before any claiming so a run's
/// budget is spent on messages worth sending, and `report` has to happen at all
/// so the numbers exist somewhere other than a table nobody reads.
pub(super) async fn sweep(db: &D1Database, mode: StalenessMode) -> Result<(), String> {
    let (sql, retired) = match mode {
        StalenessMode::Off => return Ok(()),
        StalenessMode::Report => (include_str!("sql/stale_report.sql"), false),
        StalenessMode::Cancel => (include_str!("sql/stale_cancel.sql"), true),
    };
    let statement = db.prepare(render(sql, "kind"));
    let rows = safe_all_rows(&statement).await?;
    if rows.is_empty() {
        return Ok(());
    }
    for (kind, count, oldest_due_at) in summarise(&rows) {
        tracing::warn!(
            kind = %kind,
            count,
            oldest_due_at,
            retired,
            "notification queue holds messages past their useful life"
        );
    }
    Ok(())
}

/// Collapse the returned rows into one line per kind, oldest `due_at` first.
///
/// Aggregated here rather than with `GROUP BY` so the counting path is shared
/// with `cancel` mode, whose numbers come from `RETURNING` — an aggregate over
/// the table afterwards would be a second read of something that just changed.
fn summarise(rows: &[Value]) -> Vec<(String, usize, i64)> {
    let mut summary: Vec<(String, usize, i64)> = Vec::new();
    for row in rows {
        let kind = row["kind"].as_str().unwrap_or("unknown").to_string();
        let due_at = row["due_at"].as_i64().unwrap_or_default();
        match summary.iter_mut().find(|(seen, _, _)| *seen == kind) {
            Some((_, count, oldest)) => {
                *count += 1;
                *oldest = (*oldest).min(due_at);
            }
            None => summary.push((kind, 1, due_at)),
        }
    }
    summary.sort_by_key(|(_, _, oldest)| *oldest);
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The SQL with its `--` comments removed. The headers deliberately quote
    /// the placeholders and parameters they explain, which a naive text search
    /// over the whole file would then count as occurrences.
    fn statement_only(sql: &str) -> String {
        sql.lines()
            .filter(|line| !line.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `stale_report.sql` and `stale_cancel.sql` decide the same question in
    /// two statements, so `report` mode is only an honest preview of `cancel`
    /// while their predicates are identical. Compare the text rather than
    /// trusting the comment on each.
    #[test]
    fn report_and_cancel_agree_on_what_is_stale() {
        let predicate = |sql: &str| {
            let (_, rest) = sql.split_once("WHERE ").expect("no WHERE clause");
            rest.split("RETURNING")
                .next()
                .and_then(|s| s.split("ORDER BY").next())
                .expect("empty predicate")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(
            predicate(include_str!("sql/stale_report.sql")),
            predicate(include_str!("sql/stale_cancel.sql")),
            "report mode no longer previews what cancel mode would do"
        );
    }

    /// The token has to be gone by the time the statement reaches D1: a leftover
    /// `{{max_age_secs}}` is a syntax error on the cron, where nothing is
    /// watching, and the claim loop would simply stop taking jobs.
    #[test]
    fn rendering_leaves_no_token_and_binds_the_right_column() {
        let claim = claim_sql();
        assert!(!claim.contains(MAX_AGE_TOKEN));
        assert!(claim.contains("CASE n.kind WHEN "));
        assert!(claim.contains("WHEN 'survey' THEN 259200"));
        // The claim statement reads the mode; the sweep statements do not, so a
        // stray `?1` there would be an unbound parameter at runtime. Count in
        // the executable text only — the header comments name `?1` too.
        assert_eq!(statement_only(&claim).matches("?1").count(), 1);
        for sql in [
            include_str!("sql/stale_report.sql"),
            include_str!("sql/stale_cancel.sql"),
        ] {
            let rendered = render(sql, "kind");
            assert!(!rendered.contains(MAX_AGE_TOKEN));
            assert!(rendered.contains("CASE kind WHEN 'registration' THEN 1209600"));
            assert!(rendered.contains("WHEN 'survey' THEN 259200 ELSE 0 END"));
            assert!(
                !statement_only(&rendered).contains('?'),
                "sweep statements take no parameters"
            );
        }
    }

    /// `off` is compared as a string inside the claim predicate, so the literal
    /// in SQL and the one `StalenessMode::as_str` emits must be the same word.
    #[test]
    fn the_claim_predicate_recognises_the_off_literal() {
        assert!(claim_sql().contains(&format!("?1 = '{}'", StalenessMode::Off.as_str())));
        // Only `off` short-circuits the guard; every other mode must fail that
        // comparison, or `report`/`cancel` would silently let stale rows claim.
        for mode in [StalenessMode::Report, StalenessMode::Cancel] {
            assert_ne!(mode.as_str(), StalenessMode::Off.as_str());
        }
    }

    #[test]
    fn a_summary_counts_by_kind_and_keeps_the_oldest_due() {
        let rows = vec![
            json!({"kind": "survey", "due_at": 300}),
            json!({"kind": "registration", "due_at": 100}),
            json!({"kind": "survey", "due_at": 200}),
            json!({"kind": "survey", "due_at": 400}),
        ];
        assert_eq!(
            summarise(&rows),
            vec![
                ("registration".to_string(), 1, 100),
                ("survey".to_string(), 3, 200),
            ]
        );
    }

    /// A row whose `kind` this build cannot parse still has to be counted:
    /// `max_age_case_sql` makes it stale on arrival, so it is exactly the kind
    /// of row an operator needs to see named in the log.
    #[test]
    fn an_unreadable_row_is_counted_rather_than_dropped() {
        let rows = vec![json!({"kind": null, "due_at": null})];
        assert_eq!(summarise(&rows), vec![("unknown".to_string(), 1, 0)]);
    }

    #[test]
    fn an_empty_queue_summarises_to_nothing() {
        assert!(summarise(&[]).is_empty());
    }
}
