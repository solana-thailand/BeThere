# 113 — Admin Feedback & Post-Event Survey Dashboard

**Status:** Resolved  
**Found:** 2026-09-14, post-event survey data active in production across 12 events  
**Resolved:** 2026-09-16  
**Severity:** High — Organizers and DevRel previously lacked real-time visibility into post-event survey results in the admin portal, requiring manual SQL / D1 queries to view attendee satisfaction, online attendance behavior, and qualitative feedback.

---

## 1. Problem Context

In Issues #091, #097, #098, #101, #107, #111, and #112, the retrospective feedback campaign was introduced, allowing verified attendees to answer post-event survey questions for events they participated in.

Responses are stored in `registration_responses` with keys scoped as `post.*`:
- `post.satisfaction.content`: Content satisfaction rating ("ไม่พึงพอใจ", "พึงพอใจ", "พึงพอใจมาก")
- `post.satisfaction.venue`: Venue satisfaction rating (onsite only)
- `post.satisfaction.catering`: Catering satisfaction rating (onsite only)
- `post.satisfaction.promotion`: Marketing & promotion satisfaction rating
- `post.online.watched`: Online attendance funnel ("ได้ดูสด", "ดูย้อนหลัง", "ไม่ได้ดู — ติดเวลา", etc.)
- `post.latent_space_continue`: Series continuation interest ("อยากให้จัดต่อ และจะเข้าร่วม", etc.)
- `post.comment`: Per-session feedback comments and observations
- `post.next_topics`: Topic suggestions and requests for future workshops

Over 170+ responses have been submitted across 12 events. However, organizers and admins had no interface inside `/admin` to inspect or download these responses.

---

## 2. Implementation Summary

1. **Domain Models (`domain/src/models/api.rs`):**
   - Added `FeedbackRatingDistribution`, `FeedbackDimensionStats`, `FeedbackOptionCount`, `FeedbackRespondentRow`, and `AdminFeedbackResponse`.
   - Re-exported models cleanly across crates.

2. **Backend Aggregation & API (`worker`):**
   - Implemented `get_admin_feedback` in [`worker/src/db/feedback.rs`](file:///Users/ozone/event-checkin/worker/src/db/feedback.rs):
     - Joins `registration_responses` with `attendees` on `event_id` and `lower(email)` to attach attendee name, check-in status, and participation type.
     - Uses `d1_safe::safe_all_rows` for resilience against nullable columns.
     - Aggregates the 4 dimensions (`post.satisfaction.content`, `venue`, `catering`, `promotion`) with average scores, positive response percentages, and distribution across Thai labels.
     - Aggregates online viewership funnel (`post.online.watched`) and continuation sentiment (`post.latent_space_continue`).
     - Generates RFC4180-compliant CSV export (`feedback-{event_slug}.csv`) embedded directly in response.
   - Added `admin_feedback_handler` in [`worker/src/handlers/feedback.rs`](file:///Users/ozone/event-checkin/worker/src/handlers/feedback.rs).
   - Protected endpoint via `resolve_event_with_access(&state, &claims, query.event_id.as_deref())`.
   - Mounted `GET /admin/feedback` under protected routes in [`worker/src/handlers/mod.rs`](file:///Users/ozone/event-checkin/worker/src/handlers/mod.rs).

3. **Frontend API Client & UI (`frontend-leptos`):**
   - Added `get_admin_feedback` in [`frontend-leptos/src/api/admin.rs`](file:///Users/ozone/event-checkin/frontend-leptos/src/api/admin.rs).
   - Created `<AdminFeedback />` component in [`frontend-leptos/src/pages/admin_feedback.rs`](file:///Users/ozone/event-checkin/frontend-leptos/src/pages/admin_feedback.rs):
     - KPI cards: Total Respondents, Content Satisfaction, Venue Satisfaction, Catering Satisfaction, Promotion Satisfaction.
     - Dimension breakdown cards with multi-colored horizontal distribution bars.
     - Online watch funnel bars (`post.online.watched`).
     - Series continuation sentiment breakdown (`post.latent_space_continue`).
     - Searchable respondent feedback table with participation mode filter (All, Onsite, Online), attendee comments, topic requests, and answered timestamp.
     - One-click native CSV download (`feedback-{slug}.csv`).
   - Wired `AdminSection::Feedback` into [`frontend-leptos/src/pages/admin.rs`](file:///Users/ozone/event-checkin/frontend-leptos/src/pages/admin.rs) sidebar ("Post-Event" group) and render block.

---

## 3. Verification

- **Production D1 Query Performance:**
  - Tested joined feedback query against live production D1 (`bethere-db`): completed in **1.95ms** across joined responses and attendees tables.
- **Compiler Health:**
  - `cargo check --workspace`: PASS (0 errors)
  - `cargo check` (frontend-leptos): PASS (0 errors)
  - `cargo clippy --workspace -- -D warnings`: PASS (0 warnings)
  - `cargo clippy` (frontend-leptos, `-D warnings`): PASS (0 warnings)
