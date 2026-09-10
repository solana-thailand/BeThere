//! Pure helpers: question ids, blank/default construction, JSON export.

use wasm_bindgen::JsCast;

use crate::api::{QuizConfigAdmin, QuizQuestionAdmin};

/// Generate a unique question ID (q1, q2, ...) that doesn't collide with existing.
pub(super) fn generate_question_id(existing: &[QuizQuestionAdmin]) -> String {
    let max_num = existing
        .iter()
        .filter_map(|q| q.id.strip_prefix('q').and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0);
    format!("q{}", max_num + 1)
}

/// Create a blank question with default options.
pub(super) fn blank_question(id: String) -> QuizQuestionAdmin {
    QuizQuestionAdmin {
        id,
        text: String::new(),
        options: vec![
            "Option A".to_string(),
            "Option B".to_string(),
            "Option C".to_string(),
        ],
        correct_index: 0,
        explanation: None,
        session_id: None,
        session_title: None,
        enabled: true,
    }
}

/// Create a default quiz config with one blank question.
pub(super) fn default_config() -> QuizConfigAdmin {
    QuizConfigAdmin {
        questions: vec![blank_question("q1".to_string())],
        passing_score_percent: 60,
        max_attempts: 3,
        time_limit_seconds: None,
    }
}

/// Format attempt limit display.
pub(super) fn format_attempts(n: u8) -> String {
    match n {
        1 => "1 attempt".to_string(),
        n => format!("{n} attempts"),
    }
}

/// Download a JSON string as a file.
pub(super) fn export_json_download(json_str: &str, filename: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    let blob_parts = js_sys::Array::new();
    blob_parts.push(&js_sys::JsString::from(json_str).into());
    let blob = match web_sys::Blob::new_with_str_sequence(&blob_parts) {
        Ok(b) => b,
        Err(_) => return,
    };

    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(_) => return,
    };

    let a = match document.create_element("a") {
        Ok(el) => el,
        Err(_) => return,
    };
    let _ = a.set_attribute("href", &url);
    let _ = a.set_attribute("download", filename);
    let a_el: &web_sys::HtmlElement = a.unchecked_ref();
    a_el.click();

    web_sys::Url::revoke_object_url(&url).unwrap();
}
