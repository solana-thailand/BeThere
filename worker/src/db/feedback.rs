//! D1 queries and data aggregations for event feedback / post-event survey (Issue #113).

use std::collections::BTreeMap;
use event_checkin_domain::models::api::{
    AdminFeedbackResponse, FeedbackDimensionStats, FeedbackOptionCount,
    FeedbackRatingDistribution, FeedbackRespondentRow,
};
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

pub async fn get_admin_feedback(
    db: &D1Database,
    event_id: &str,
    event_name: &str,
) -> Result<AdminFeedbackResponse, String> {
    let sql = "SELECT \
        r.developer_email, \
        r.field_key, \
        r.field_value, \
        r.answered_at, \
        COALESCE(a.name, '') AS name, \
        COALESCE(a.participation_type, 'in_person') AS participation_type, \
        (CASE WHEN a.checked_in_at IS NOT NULL AND a.checked_in_at <> '' THEN 1 ELSE 0 END) AS is_checked_in \
        FROM registration_responses r \
        LEFT JOIN attendees a \
          ON a.event_id = r.event_id \
         AND LOWER(a.email) = LOWER(r.developer_email) \
        WHERE r.event_id = ?1 \
          AND r.field_key LIKE 'post.%' \
        ORDER BY r.answered_at DESC, r.developer_email ASC";

    let stmt = db.prepare(sql);
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 get_admin_feedback bind: {e:?}"))?;

    let rows = safe_all_rows(&bound)
        .await
        .map_err(|e| format!("D1 get_admin_feedback safe_all_rows: {e}"))?;

    // Group by email
    let mut respondents_map: BTreeMap<String, FeedbackRespondentRow> = BTreeMap::new();

    for row in rows {
        let email = row
            .get("developer_email")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if email.is_empty() {
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
            .unwrap_or(0) == 1;

        let entry = respondents_map.entry(email.clone()).or_insert_with(|| {
            FeedbackRespondentRow {
                email: email.clone(),
                name: if name.is_empty() { email.clone() } else { name.clone() },
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
                } else if v == "พึงพอใจ" || (v.contains("พึงพอใจ") && !v.contains("ไม่")) {
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
    let mut csv_lines = Vec::with_capacity(respondents.len() + 1);
    csv_lines.push("Email,Name,Participation,Checked In,Answered At,Content Satisfaction,Venue Satisfaction,Catering Satisfaction,Promotion Satisfaction,Online Watched,Series Continuation,Comment,Next Topics".to_string());

    for r in &respondents {
        let line = format!(
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
        csv_lines.push(line);
    }
    let csv_content = csv_lines.join("\r\n");
    let safe_event_slug = event_id.replace(['/', '\\', ' ', ':'], "-");
    let filename = format!("feedback-{safe_event_slug}.csv");

    Ok(AdminFeedbackResponse {
        event_id: event_id.to_string(),
        event_name: event_name.to_string(),
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
