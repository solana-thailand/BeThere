//! D1 queries and data aggregations for event feedback / post-event survey (Issue #113).

use event_checkin_domain::models::api::{
    AdminFeedbackResponse, FeedbackDimensionStats, FeedbackEventIncluded, FeedbackOptionCount,
    FeedbackRatingDistribution, FeedbackRespondentRow,
};
use std::collections::BTreeMap;
use worker::D1Database;
use worker::d1::D1Type;

use crate::db::d1_safe::safe_all_rows;

const CONTENT_KEY: &str = "post.satisfaction.content";
const VENUE_KEY: &str = "post.satisfaction.venue";
const CATERING_KEY: &str = "post.satisfaction.catering";
const PROMOTION_KEY: &str = "post.satisfaction.promotion";
const WATCHED_KEY: &str = "post.online.watched";
const LATENT_SPACE_KEY: &str = "post.latent_space_continue";
const COMMENT_KEY: &str = "post.comment";
const NEXT_TOPICS_KEY: &str = "post.next_topics";

fn escape_csv(s: &str) -> String {
    match s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        true => format!("\"{}\"", s.replace('"', "\"\"")),
        false => s.to_string(),
    }
}

/// Scope definition for feedback queries: single event, series, or all events.
pub struct FeedbackFilterScope<'a> {
    pub target_id: &'a str,
    pub title: &'a str,
    pub series_name: Option<String>,
    pub filter_event_ids: Option<&'a [String]>,
}

pub async fn get_admin_feedback(
    db: &D1Database,
    scope: FeedbackFilterScope<'_>,
) -> Result<AdminFeedbackResponse, String> {
    // If an explicit empty list was passed, return an empty response immediately.
    if let Some(ids) = scope.filter_event_ids
        && ids.is_empty()
    {
        return Ok(AdminFeedbackResponse {
            event_id: scope.target_id.to_string(),
            event_name: scope.title.to_string(),
            series_name: scope.series_name,
            events_included: Vec::new(),
            total_respondents: 0,
            onsite_respondents: 0,
            online_respondents: 0,
            dimensions: Vec::new(),
            online_watched: Vec::new(),
            latent_space_continue: Vec::new(),
            respondents: Vec::new(),
            csv: None,
            filename: None,
        });
    }

    let rows = match scope.filter_event_ids {
        Some(ids) if ids.len() == 1 => {
            let sql = "SELECT \
                r.event_id, \
                r.developer_email, \
                r.field_key, \
                r.field_value, \
                r.answered_at, \
                COALESCE(a.name, '') AS name, \
                COALESCE(a.participation_type, 'in_person') AS participation_type, \
                (CASE WHEN a.checked_in_at IS NOT NULL AND a.checked_in_at <> '' THEN 1 ELSE 0 END) AS is_checked_in, \
                COALESCE(e.name, r.event_id) AS event_name \
                FROM registration_responses r \
                LEFT JOIN attendees a \
                  ON a.event_id = r.event_id \
                 AND LOWER(a.email) = LOWER(r.developer_email) \
                LEFT JOIN events e \
                  ON e.id = r.event_id \
                WHERE r.event_id = ?1 \
                  AND r.field_key LIKE 'post.%' \
                ORDER BY r.answered_at DESC, r.developer_email ASC";
            let stmt = db.prepare(sql);
            let bound = stmt
                .bind_refs(&[D1Type::Text(&ids[0])])
                .map_err(|e| format!("D1 get_admin_feedback bind: {e:?}"))?;
            safe_all_rows(&bound)
                .await
                .map_err(|e| format!("D1 get_admin_feedback safe_all_rows: {e}"))?
        }
        Some(ids) => {
            let placeholders = (1..=ids.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT \
                r.event_id, \
                r.developer_email, \
                r.field_key, \
                r.field_value, \
                r.answered_at, \
                COALESCE(a.name, '') AS name, \
                COALESCE(a.participation_type, 'in_person') AS participation_type, \
                (CASE WHEN a.checked_in_at IS NOT NULL AND a.checked_in_at <> '' THEN 1 ELSE 0 END) AS is_checked_in, \
                COALESCE(e.name, r.event_id) AS event_name \
                FROM registration_responses r \
                LEFT JOIN attendees a \
                  ON a.event_id = r.event_id \
                 AND LOWER(a.email) = LOWER(r.developer_email) \
                LEFT JOIN events e \
                  ON e.id = r.event_id \
                WHERE r.event_id IN ({placeholders}) \
                  AND r.field_key LIKE 'post.%' \
                ORDER BY r.answered_at DESC, r.developer_email ASC"
            );
            let stmt = db.prepare(&sql);
            let binds: Vec<D1Type> = ids.iter().map(|id| D1Type::Text(id.as_str())).collect();
            let bound = stmt
                .bind_refs(&binds)
                .map_err(|e| format!("D1 get_admin_feedback bind: {e:?}"))?;
            safe_all_rows(&bound)
                .await
                .map_err(|e| format!("D1 get_admin_feedback safe_all_rows: {e}"))?
        }
        None => {
            let sql = "SELECT \
                r.event_id, \
                r.developer_email, \
                r.field_key, \
                r.field_value, \
                r.answered_at, \
                COALESCE(a.name, '') AS name, \
                COALESCE(a.participation_type, 'in_person') AS participation_type, \
                (CASE WHEN a.checked_in_at IS NOT NULL AND a.checked_in_at <> '' THEN 1 ELSE 0 END) AS is_checked_in, \
                COALESCE(e.name, r.event_id) AS event_name \
                FROM registration_responses r \
                LEFT JOIN attendees a \
                  ON a.event_id = r.event_id \
                 AND LOWER(a.email) = LOWER(r.developer_email) \
                LEFT JOIN events e \
                  ON e.id = r.event_id \
                WHERE r.field_key LIKE 'post.%' \
                ORDER BY r.answered_at DESC, r.developer_email ASC";
            let stmt = db.prepare(sql);
            safe_all_rows(&stmt)
                .await
                .map_err(|e| format!("D1 get_admin_feedback safe_all_rows: {e}"))?
        }
    };

    // Group by (event_id, email) so multiple event submissions by the same person remain distinct
    let mut respondents_map: BTreeMap<String, FeedbackRespondentRow> = BTreeMap::new();
    let mut events_map: BTreeMap<String, (String, usize)> = BTreeMap::new();

    for row in rows {
        let event_id = row
            .get("event_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        let event_name = row
            .get("event_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        let email = row
            .get("developer_email")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if email.is_empty() || event_id.is_empty() {
            continue;
        }

        let field_key = row
            .get("field_key")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim();
        let field_value = row
            .get("field_value")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        let answered_at = row
            .get("answered_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let name = row
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        let participation_type = row
            .get("participation_type")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        let is_checked_in = row
            .get("is_checked_in")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            == 1;

        let respondent_key = format!("{event_id}:{email}");
        let entry = respondents_map.entry(respondent_key).or_insert_with(|| {
            let display_name = if event_name.is_empty() {
                event_id.clone()
            } else {
                event_name.clone()
            };
            events_map
                .entry(event_id.clone())
                .or_insert((display_name, 0))
                .1 += 1;
            FeedbackRespondentRow {
                event_id: Some(event_id.clone()),
                event_name: Some(if event_name.is_empty() {
                    event_id.clone()
                } else {
                    event_name.clone()
                }),
                email: email.clone(),
                name: if name.is_empty() {
                    email.clone()
                } else {
                    name.clone()
                },
                participation_type: if participation_type.is_empty() {
                    "in_person".to_string()
                } else {
                    participation_type.clone()
                },
                is_checked_in,
                answered_at: answered_at.clone(),
                ..Default::default()
            }
        });

        // Update fields if we didn't have name before
        if entry.name == entry.email && !name.is_empty() {
            entry.name = name;
        }
        if entry.answered_at.is_none() && answered_at.is_some() {
            entry.answered_at = answered_at;
        }

        match field_key {
            CONTENT_KEY => entry.satisfaction_content = Some(field_value),
            VENUE_KEY => entry.satisfaction_venue = Some(field_value),
            CATERING_KEY => entry.satisfaction_catering = Some(field_value),
            PROMOTION_KEY => entry.satisfaction_promotion = Some(field_value),
            WATCHED_KEY => entry.online_watched = Some(field_value),
            LATENT_SPACE_KEY => entry.latent_space_continue = Some(field_value),
            COMMENT_KEY => entry.comment = Some(field_value),
            NEXT_TOPICS_KEY => entry.next_topics = Some(field_value),
            _ => {}
        }
    }

    let mut respondents: Vec<FeedbackRespondentRow> = respondents_map.into_values().collect();
    // Sort respondents by answered_at DESC
    respondents.sort_by(|a, b| b.answered_at.cmp(&a.answered_at));

    let total_respondents = respondents.len();
    let onsite_respondents = respondents
        .iter()
        .filter(|r| {
            let p = r.participation_type.to_lowercase();
            p.contains("in_person") || p.contains("in-person") || p.contains("onsite")
        })
        .count();
    let online_respondents = total_respondents.saturating_sub(onsite_respondents);

    // Build list of events included
    let mut events_included: Vec<FeedbackEventIncluded> = events_map
        .into_iter()
        .map(|(eid, (ename, count))| FeedbackEventIncluded {
            event_id: eid,
            event_name: ename,
            respondent_count: count,
        })
        .collect();
    events_included.sort_by_key(|b| std::cmp::Reverse(b.respondent_count));

    // Build dimensions
    let dimension_defs = [
        (CONTENT_KEY, "ด้านเนื้อหา (Content)"),
        (VENUE_KEY, "ด้านสถานที่ (Venue)"),
        (CATERING_KEY, "ด้านอาหารเครื่องดื่ม (Catering)"),
        (PROMOTION_KEY, "ด้านการประชาสัมพันธ์ (Promotion)"),
    ];

    let mut dimensions = Vec::with_capacity(dimension_defs.len());
    for (dim_key, dim_title) in dimension_defs {
        let mut count_high = 0usize;
        let mut count_med = 0usize;
        let mut count_low = 0usize;
        let mut other_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut total_dim = 0usize;
        let mut score_sum = 0.0f64;

        for r in &respondents {
            let val = match dim_key {
                CONTENT_KEY => r.satisfaction_content.as_deref(),
                VENUE_KEY => r.satisfaction_venue.as_deref(),
                CATERING_KEY => r.satisfaction_catering.as_deref(),
                PROMOTION_KEY => r.satisfaction_promotion.as_deref(),
                _ => None,
            };

            if let Some(v) = val {
                let v = v.trim();
                if v.is_empty() {
                    continue;
                }
                total_dim += 1;
                if v.contains("พึงพอใจมาก") {
                    count_high += 1;
                    score_sum += 3.0;
                } else if v == "พึงพอใจ" || (v.contains("พึงพอใจ") && !v.contains("ไม่"))
                {
                    count_med += 1;
                    score_sum += 2.0;
                } else if v.contains("ไม่พึงพอใจ") {
                    count_low += 1;
                    score_sum += 1.0;
                } else {
                    *other_counts.entry(v.to_string()).or_insert(0) += 1;
                }
            }
        }

        let average_score = if total_dim > 0 {
            (score_sum / total_dim as f64 * 100.0).round() / 100.0
        } else {
            0.0
        };

        let positive_percentage = if total_dim > 0 {
            (((count_high + count_med) as f64 / total_dim as f64) * 1000.0).round() / 10.0
        } else {
            0.0
        };

        let mut distribution = Vec::new();
        if total_dim > 0 {
            distribution.push(FeedbackRatingDistribution {
                label: "พึงพอใจมาก".to_string(),
                count: count_high,
                percentage: ((count_high as f64 / total_dim as f64) * 1000.0).round() / 10.0,
            });
            distribution.push(FeedbackRatingDistribution {
                label: "พึงพอใจ".to_string(),
                count: count_med,
                percentage: ((count_med as f64 / total_dim as f64) * 1000.0).round() / 10.0,
            });
            distribution.push(FeedbackRatingDistribution {
                label: "ไม่พึงพอใจ".to_string(),
                count: count_low,
                percentage: ((count_low as f64 / total_dim as f64) * 1000.0).round() / 10.0,
            });
            for (lbl, c) in other_counts {
                distribution.push(FeedbackRatingDistribution {
                    label: lbl,
                    count: c,
                    percentage: ((c as f64 / total_dim as f64) * 1000.0).round() / 10.0,
                });
            }
        }

        dimensions.push(FeedbackDimensionStats {
            key: dim_key.to_string(),
            title: dim_title.to_string(),
            total_answers: total_dim,
            average_score,
            positive_percentage,
            distribution,
        });
    }

    // Build online watched options breakdown
    let mut watched_map: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_watched = 0usize;
    for r in &respondents {
        if let Some(ref w) = r.online_watched {
            let w = w.trim();
            if !w.is_empty() {
                *watched_map.entry(w.to_string()).or_insert(0) += 1;
                total_watched += 1;
            }
        }
    }
    let mut online_watched: Vec<FeedbackOptionCount> = watched_map
        .into_iter()
        .map(|(opt, cnt)| FeedbackOptionCount {
            percentage: if total_watched > 0 {
                ((cnt as f64 / total_watched as f64) * 1000.0).round() / 10.0
            } else {
                0.0
            },
            option: opt,
            count: cnt,
        })
        .collect();
    online_watched.sort_by_key(|b| std::cmp::Reverse(b.count));

    // Build latent space continuation breakdown
    let mut continue_map: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_continue = 0usize;
    for r in &respondents {
        if let Some(ref c) = r.latent_space_continue {
            let c = c.trim();
            if !c.is_empty() {
                *continue_map.entry(c.to_string()).or_insert(0) += 1;
                total_continue += 1;
            }
        }
    }
    let mut latent_space_continue: Vec<FeedbackOptionCount> = continue_map
        .into_iter()
        .map(|(opt, cnt)| FeedbackOptionCount {
            percentage: if total_continue > 0 {
                ((cnt as f64 / total_continue as f64) * 1000.0).round() / 10.0
            } else {
                0.0
            },
            option: opt,
            count: cnt,
        })
        .collect();
    latent_space_continue.sort_by_key(|b| std::cmp::Reverse(b.count));

    // Build CSV
    let has_multiple_events = events_included.len() > 1;
    let mut csv_lines = Vec::with_capacity(respondents.len() + 1);
    if has_multiple_events {
        csv_lines.push("Event,Email,Name,Participation,Checked In,Answered At,Content Satisfaction,Venue Satisfaction,Catering Satisfaction,Promotion Satisfaction,Online Watched,Series Continuation,Comment,Next Topics".to_string());
    } else {
        csv_lines.push("Email,Name,Participation,Checked In,Answered At,Content Satisfaction,Venue Satisfaction,Catering Satisfaction,Promotion Satisfaction,Online Watched,Series Continuation,Comment,Next Topics".to_string());
    }

    for r in &respondents {
        let base_line = format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}",
            escape_csv(&r.email),
            escape_csv(&r.name),
            escape_csv(&r.participation_type),
            if r.is_checked_in { "Yes" } else { "No" },
            escape_csv(r.answered_at.as_deref().unwrap_or("")),
            escape_csv(r.satisfaction_content.as_deref().unwrap_or("")),
            escape_csv(r.satisfaction_venue.as_deref().unwrap_or("")),
            escape_csv(r.satisfaction_catering.as_deref().unwrap_or("")),
            escape_csv(r.satisfaction_promotion.as_deref().unwrap_or("")),
            escape_csv(r.online_watched.as_deref().unwrap_or("")),
            escape_csv(r.latent_space_continue.as_deref().unwrap_or("")),
            escape_csv(r.comment.as_deref().unwrap_or("")),
            escape_csv(r.next_topics.as_deref().unwrap_or("")),
        );
        if has_multiple_events {
            let ev_display = r
                .event_name
                .as_deref()
                .or(r.event_id.as_deref())
                .unwrap_or("");
            csv_lines.push(format!("{},{}", escape_csv(ev_display), base_line));
        } else {
            csv_lines.push(base_line);
        }
    }
    let csv_content = csv_lines.join("\r\n");
    let safe_target = scope.target_id.replace(['/', '\\', ' ', ':'], "-");
    let filename = format!("feedback-{safe_target}.csv");

    Ok(AdminFeedbackResponse {
        event_id: scope.target_id.to_string(),
        event_name: scope.title.to_string(),
        series_name: scope.series_name,
        events_included,
        total_respondents,
        onsite_respondents,
        online_respondents,
        dimensions,
        online_watched,
        latent_space_continue,
        respondents,
        csv: Some(csv_content),
        filename: Some(filename),
    })
}
