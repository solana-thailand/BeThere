# Issue 034: Quiz CRUD, Enable/Disable & Bulk Import

> **Status: 📋 PLANNED**

## Problem

Quiz questions are stored as a single JSON blob in Cloudflare KV. The admin editor loads/saves the entire blob atomically. This creates several pain points:

1. **No enable/disable per question** — must delete to remove, can't temporarily disable
2. **No bulk import** — organizer must hand-enter every question via the editor UI
3. **No export/backup** — can't extract questions for sharing or reuse between events
4. **No individual question API** — read-modify-write is client-side only (load all → edit → save all)

The immediate trigger: organizers want to generate 20 questions with AI and import them quickly, and want to toggle questions on/off per event without deleting them.

## Solution

### Level 1 (High Priority)

#### 1.1 `enabled` field on `QuizQuestion`

Add `enabled: bool` (default `true`) to `QuizQuestion` across all layers:
- `domain/src/models/api.rs` — `QuizQuestion`, `QuizQuestionPublic`
- `worker/src/quiz.rs` — filter disabled questions in `to_public_questions()`
- `frontend-leptos/src/api/admin.rs` — `QuizQuestionAdmin`
- `frontend-leptos/src/api/claim.rs` — `QuizQuestionPublic`

**Behavior:**
- Disabled questions are **hidden from attendees** — filtered out in `to_public_questions()`
- Disabled questions are **visible in the admin editor** — shown greyed out with a toggle
- Scoring only counts enabled questions (if 20 questions, 5 disabled → scored on 15)
- Default `true` — backward compatible

#### 1.2 Enable/Disable toggle in quiz editor

Per-question toggle switch in `quiz_editor.rs`:
- Toggle switch in the question card header (next to move/delete controls)
- Visual state: disabled questions have reduced opacity + "Disabled" badge
- When toggling, the question stays in its position in the array

#### 1.3 Import JSON button in quiz editor

"Import" button in the editor header:
- Opens a modal with a `<textarea>` for pasting JSON
- Accepts two formats:
  - Full `QuizConfig` (with `passing_score_percent`, `max_attempts`, etc.)
  - Array of questions `[ { id, text, options, correct_index, ... } ]`
- **Merge mode:** adds imported questions to existing (doesn't replace)
- **Replace mode:** replaces all questions (keeps settings)
- Validates before importing (min 2 options, non-empty text, valid correct_index)
- Shows count: "Imported 10 questions. Total: 20."

### Level 2 (Medium Priority)

#### 2.1 Individual question CRUD API endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/admin/quiz/questions` | Add a single question |
| `PUT` | `/api/admin/quiz/questions/{id}` | Update a single question |
| `DELETE` | `/api/admin/quiz/questions/{id}` | Delete a single question |
| `PATCH` | `/api/admin/quiz/questions/{id}/toggle` | Enable/disable toggle |

All endpoints read-modify-write the KV blob internally. Client sees individual CRUD.

#### 2.2 Export JSON button in editor

- "Export" button downloads current quiz as `.json` file
- Useful for backup, sharing between events, or version control

### Level 3 (Low Priority)

#### 3.1 Question versioning / audit trail

- Track who changed what and when (reuse audit store pattern from `worker/src/audit_store.rs`)
- Show last-modified timestamp per question

## Storage Decision

**Keep Cloudflare KV** — do not switch to Google Sheets.

Reasons:
- KV reads are ~1ms (edge-cached) vs ~200ms+ for Sheets API
- Questions have variable-length options (2-8 per question) — natural in JSON, awkward in fixed spreadsheet columns
- Session grouping (`session_id`/`session_title`) is native in JSON
- KV is already set up and working
- Sheets is better for attendee lists (tabular data) but wrong shape for quiz questions (nested, variable-length)

## Data Model Changes

```
QuizQuestion {
    id: String,
    text: String,
    options: Vec<String>,
    correct_index: u8,
    explanation: Option<String>,
    session_id: Option<String>,
    session_title: Option<String>,
    enabled: bool,              // ← NEW, default true
}

QuizQuestionAdmin {
    id: String,
    text: String,
    options: Vec<String>,
    correct_index: u8,
    explanation: Option<String>,
    session_id: Option<String>,
    session_title: Option<String>,
    enabled: bool,              // ← NEW, default true
}

QuizQuestionPublic {
    id: String,
    text: String,
    options: Vec<String>,
    session_id: Option<String>,
    session_title: Option<String>,
    // enabled: NOT included — filtered out server-side
}
```

## Implementation Phases

### Phase 1 — Enable/Disable + Import (~1.5 hours)
- [ ] Add `enabled: bool` to domain types
- [ ] Filter disabled questions in `to_public_questions()`
- [ ] Add toggle in quiz editor UI
- [ ] Add Import JSON modal (merge + replace modes)
- [ ] Update `.issues/002_quiz_gated_claim.md` data model section

### Phase 2 — Individual CRUD API (~2 hours)
- [ ] `POST /api/admin/quiz/questions`
- [ ] `PUT /api/admin/quiz/questions/{id}`
- [ ] `DELETE /api/admin/quiz/questions/{id}`
- [ ] `PATCH /api/admin/quiz/questions/{id}/toggle`

### Phase 3 — Export + Polish (~30 min)
- [ ] Export JSON button
- [ ] Disabled question visual polish (opacity, badge)

## Backward Compatibility

- `enabled` defaults to `true` via `#[serde(default = "default_true")]`
- Old quiz configs without `enabled` field → all questions enabled → no behavior change
- `to_public_questions()` already existed — just adds a `.filter()` before mapping

## Files to Modify

| File | Change |
|------|--------|
| `domain/src/models/api.rs` | Add `enabled: bool` to `QuizQuestion` |
| `worker/src/quiz.rs` | Filter disabled in `to_public_questions()` |
| `frontend-leptos/src/api/admin.rs` | Add `enabled: bool` to `QuizQuestionAdmin` |
| `frontend-leptos/src/api/claim.rs` | No change (public type doesn't get `enabled`) |
| `frontend-leptos/src/pages/quiz_editor.rs` | Toggle switch + Import modal |
| `frontend-leptos/style.css` | Disabled question styles |

## Refs

- Issue 002: `.issues/002_quiz_gated_claim.md`
- Handover 021: `.handovers/021_quiz_gated_claim_flow.md`
- Quiz backend: `worker/src/quiz.rs`, `worker/src/handlers/quiz.rs`
- Quiz editor: `frontend-leptos/src/pages/quiz_editor.rs`
- Claim quiz view: `frontend-leptos/src/pages/claim.rs`
