//! A create or duplicate refused for bad input must reach the admin as a 400
//! with its reason. Every `create_event` error used to become a 500 "internal
//! error", which the redactor leaves with no reason at all: duplicating an
//! event with no Sheet ID failed that way on staging (2026-09-27).

use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::CreateEventRequest;
use event_checkin_worker::event_store::{EventWriteError, create_event};

fn request(sheet_id: &str, start_ms: i64, end_ms: i64) -> CreateEventRequest {
    serde_json::from_value(serde_json::json!({
        "name": "Probe",
        "slug": "probe",
        "tagline": "",
        "link": "",
        "event_start_ms": start_ms,
        "event_end_ms": end_ms,
        "time_tba": false,
        "sheet_id": sheet_id,
    }))
    .expect("minimal create request")
}

async fn refusal(req: &CreateEventRequest) -> EventWriteError {
    // No KV, no D1: validation runs before any storage is touched.
    match create_event(None, None, req, "admin@example.com").await {
        Ok(_) => panic!("invalid input must be refused"),
        Err(e) => e,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn missing_sheet_id_is_invalid_input() {
    let e = refusal(&request("", 1_000, 2_000)).await;
    assert_eq!(
        e,
        EventWriteError::Invalid("google sheet_id is required".into())
    );
}

#[tokio::test(flavor = "current_thread")]
async fn end_before_start_is_invalid_input() {
    let e = refusal(&request("sheet", 2_000, 1_000)).await;
    assert!(matches!(e, EventWriteError::Invalid(_)), "{e:?}");
}

#[test]
fn invalid_input_is_a_validation_error_and_storage_is_internal() {
    let invalid: AppError = EventWriteError::Invalid("x".into()).into();
    assert!(matches!(invalid, AppError::Validation(ref m) if m == "x"));
    let storage: AppError = EventWriteError::Storage("x".into()).into();
    assert!(matches!(storage, AppError::Internal(_)));
}

#[test]
fn handlers_keep_the_error_kind() {
    // Both entry points go through `logged`, not a blanket `AppError::Internal`.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers/events");
    for (file, action) in [
        ("create.rs", "create event"),
        ("duplicate.rs", "duplicate event"),
    ] {
        let src = std::fs::read_to_string(dir.join(file)).expect(file);
        assert!(
            src.contains(&format!(".map_err(|e| e.logged(\"{action}\"))?")),
            "{file}: create_event errors must keep their kind"
        );
    }
}
