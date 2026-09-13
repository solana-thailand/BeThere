//! Attendee-facing preview rendering.

use leptos::prelude::*;

use crate::api::{QuizConfigAdmin, QuizQuestionAdmin};

use super::helpers::format_attempts;

/// Questions grouped by session for preview rendering:
/// `(session_title, [(global_question_index, question)])`.
type SessionGroups<'a> = Vec<(Option<String>, Vec<(usize, &'a QuizQuestionAdmin)>)>;

/// Render the quiz in preview mode (as attendees see it).
pub(super) fn render_preview(config: &QuizConfigAdmin) -> AnyView {
    let questions = config.questions.clone();
    let passing = config.passing_score_percent;
    let max_attempts = config.max_attempts;
    let time_limit = config.time_limit_seconds;
    let total = questions.len();

    // Group questions by session, preserving encounter order.
    // Questions without a session_id are grouped under the “General” bucket.
    let mut session_order: Vec<(Option<String>, Option<String>)> = Vec::new(); // (session_id, session_title)
    for q in &questions {
        let key = (q.session_id.clone(), q.session_title.clone());
        if !session_order.iter().any(|s| s.0 == key.0) {
            session_order.push(key);
        }
    }

    let session_groups: SessionGroups = session_order
        .iter()
        .map(|(sid, stitle)| {
            let group_qs: Vec<(usize, &QuizQuestionAdmin)> = questions
                .iter()
                .enumerate()
                .filter(|(_, q)| q.session_id == *sid)
                .collect();
            (stitle.clone(), group_qs)
        })
        .collect();

    let _has_sessions = session_groups.len() > 1
        || session_groups
            .first()
            .map(|(t, _)| t.is_some())
            .unwrap_or(false);

    let time_display = time_limit.map(|t| {
        if t >= 60 {
            format!("{}m {}s", t / 60, t % 60)
        } else {
            format!("{t}s")
        }
    });

    view! {
        <div class="quiz-preview">
            // Info bar
            <div class="card quiz-preview-info">
                <div class="quiz-preview-stat">
                    <span class="quiz-preview-stat-value">{total}</span>
                    <span class="quiz-preview-stat-label">"Questions"</span>
                </div>
                <div class="quiz-preview-stat">
                    <span class="quiz-preview-stat-value">{format!("{passing}%")}</span>
                    <span class="quiz-preview-stat-label">"To Pass"</span>
                </div>
                <div class="quiz-preview-stat">
                    <span class="quiz-preview-stat-value">{max_attempts}</span>
                    <span class="quiz-preview-stat-label">{format_attempts(max_attempts)}</span>
                </div>
                {match time_display {
                    Some(td) => view! {
                        <div class="quiz-preview-stat">
                            <span class="quiz-preview-stat-value">{td}</span>
                            <span class="quiz-preview-stat-label">"Time Limit"</span>
                        </div>
                    }.into_any(),
                    None => view! { <div></div> }.into_any(),
                }}
            </div>

            // Questions grouped by session
            <div class="quiz-preview-questions">
                {session_groups.iter().map(|(session_title, group_qs)| {
                    let title_view = match session_title {
                        Some(t) => view! {
                            <div class="quiz-preview-session-header">
                                <h4 class="quiz-preview-session-title">{t.clone()}</h4>
                            </div>
                        }.into_any(),
                        None => view! { <div></div> }.into_any(),
                    };

                    let q_views = group_qs.iter().map(|(global_idx, q)| {
                        let question_text = q.text.clone();
                        let options = q.options.clone();
                        let qnum = global_idx + 1;

                        view! {
                            <div class="card quiz-preview-question">
                                <div class="quiz-preview-question-text">
                                    {format!("{}. {question_text}", qnum)}
                                </div>
                                <div class="quiz-preview-options">
                                    {options.iter().enumerate().map(|(oi, opt)| {
                                        let letter = (b'A' + oi as u8) as char;
                                        view! {
                                            <div class="quiz-preview-option">
                                                <span class="quiz-preview-option-letter">
                                                    {format!("{letter}.")}
                                                </span>
                                                <span>{opt.clone()}</span>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>
                        }
                    }).collect_view();

                    view! {
                        {title_view}
                        {q_views}
                    }.into_any()
                }).collect_view()}
            </div>

            <div class="quiz-preview-note">
                "This is how attendees will see the quiz. Correct answers are hidden."
            </div>
        </div>
    }
    .into_any()
}
