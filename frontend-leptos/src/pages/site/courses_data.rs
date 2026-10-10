//! The courses from the worker (.plans/045 R4.4): active campaigns and
//! their public episodes (`GET /api/public/courses[/{id}]`). Public reads:
//! no sign-in, never a redirect to /login.

use event_checkin_domain::models::course::{CourseDetail, CourseSummary};

async fn get<T: serde::de::DeserializeOwned + Default>(path: &str) -> Option<T> {
    let resp = crate::api::fetch::get(path, &[]).await.ok()?;
    if resp.status() != 200 {
        return None;
    }
    let body = crate::api::fetch::response_json::<crate::api::ApiResponse<T>>(&resp)
        .await
        .ok()?;
    body.data
}

/// Every course, or none when the read fails.
pub async fn courses() -> Vec<CourseSummary> {
    get("/api/public/courses").await.unwrap_or_default()
}

/// One course page, or `None` for no such course (or a failed read).
pub async fn course(id: &str) -> Option<CourseDetail> {
    get(&format!("/api/public/courses/{id}")).await
}
