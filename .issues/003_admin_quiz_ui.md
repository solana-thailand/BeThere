# Issue 003: Admin Quiz UI [PLAN]

## Problem

Quiz questions are currently set via `wrangler kv key put` CLI commands (see Issue 002 setup instructions). Organizers must:
- Manually craft a JSON file with the exact schema
- Use the Wrangler CLI to upload it
- Have no visual way to preview, edit, or manage questions

This blocks non-technical organizers from self-serving quiz setup.

## Solution

Build a visual quiz editor in the admin dashboard that lets organizers:
1. Load the current quiz config (with correct answers)
2. Add, edit, remove, and reorder questions
3. Set passing score, max attempts, and optional timer
4. Preview the quiz as attendees see it
5. Save changes to KV via the existing `POST /api/admin/quiz` endpoint

### Flow

```
Admin Dashboard → Sidebar "Quiz" tab → Quiz Editor
  → Load current config from GET /api/admin/quiz
  → Edit questions (text, options, correct answer, explanation)
  → Configure settings (passing %, max attempts, timer)
  → Preview (shows quiz without correct answers)
  → Save → POST /api/admin/quiz
  → Toast confirmation
```

## Architecture

### Backend (already done)

- `GET /api/admin/quiz` — returns full config with correct answers (protected)
- `POST /api/admin/quiz` — saves full config (protected, already existed)

### Frontend

#### API Layer (`api.rs`)

Types added:
- `QuizQuestionAdmin` — question with `correct_index` and `explanation`
- `QuizConfigAdmin` — full config for save
- `AdminQuizData` — response from GET
- `AdminQuizSaveData` — response from POST

Functions added:
- `get_admin_quiz()` — GET /api/admin/quiz
- `put_admin_quiz(config)` — POST /api/admin/quiz

#### Admin Page Changes (`admin.rs`)

Add a new sidebar section "Quiz" with quiz editor content:
- **AdminSection enum**: `Attendance` | `Quiz` — replaces the implicit tab-only model
- Sidebar shows "Attendance" section (In-Person/Online) and "Quiz" section
- Clicking "Quiz" switches the main content area to the quiz editor

#### Quiz Editor UI

The quiz editor component renders:

1. **Settings card**: passing_score_percent slider/input, max_attempts input, time_limit_seconds optional
2. **Questions list**: each question is an editable card with:
   - Question text (textarea)
   - Options list (editable text inputs, radio buttons for correct answer)
   - Add/remove option buttons
   - Explanation (optional textarea)
   - Delete question button
   - Drag handle for reordering (or up/down buttons)
3. **Add question button**: adds a new blank question
4. **Preview toggle**: shows the quiz as attendees see it
5. **Save button**: calls PUT /api/admin/quiz, shows toast

### State Management

```
Signals:
  - quiz_config: Option<QuizConfigAdmin> — loaded from API
  - quiz_loading: bool
  - quiz_saving: bool
  - quiz_preview: bool — toggle preview mode
  - quiz_dirty: bool — unsaved changes indicator
```

### UI/UX Details

- **Auto-save indication**: Show "Unsaved changes" badge when config differs from loaded state
- **Validation before save**: Same rules as backend (min 1 question, min 2 options, valid correct_index, 1-100 passing score, max_attempts >= 1, unique IDs)
- **Empty state**: When no quiz configured, show "Create Your Quiz" prompt
- **Question IDs**: Auto-generated as `q1`, `q2`, etc. when adding new questions
- **Correct answer**: Radio buttons or dropdown next to each option

## Implementation Phases

### Phase 1 — Backend API ✅ COMPLETE
- [x] Add `GET /api/admin/quiz` handler
- [x] Register route in handlers/mod.rs
- [x] Verify worker builds

### Phase 2 — Frontend API Layer ✅ COMPLETE (done this session)
- [x] Add admin quiz types to api.rs
- [x] Add `get_admin_quiz()` and `put_admin_quiz()` functions
- [x] Add `api_post_json` helper
- [x] Verify frontend compiles

### Phase 3 — Quiz Editor UI ✅ COMPLETE
- [x] Add `AdminSection` enum to admin.rs
- [x] Refactor sidebar to show Attendance + Quiz sections
- [x] Build quiz editor component (`quiz_editor.rs` — 830 lines)
- [x] Add/edit/remove question functionality (up/down/reorder/delete)
- [x] Preview mode (attendee view, correct answers hidden)
- [x] Save with validation + toast
- [x] Empty state for unconfigured quiz ("Create Quiz" prompt)
- [x] CSS styles for quiz editor (~460 lines)
- [x] Clippy-clean, trunk build verified

### Phase 4 — Polish & Deploy
- [ ] Mobile responsive quiz editor
- [ ] Keyboard shortcuts (optional)
- [ ] Test full flow: load → edit → save → verify via claim page
- [ ] Deploy to production
- [ ] Delete feature branch

## Quiz Editor Mockup

```
┌─────────────────────────────────────────────┐
│ Quiz Editor                    [Preview] [Save] │
├─────────────────────────────────────────────┤
│                                             │
│ ┌─ Settings ──────────────────────────────┐ │
│ │ Passing Score: [====60====]%            │ │
│ │ Max Attempts:  [3]                      │ │
│ │ Time Limit:    [  300] seconds (opt)    │ │
│ └─────────────────────────────────────────┘ │
│                                             │
│ ┌─ Question 1 ───────────────── [↑][↓][✕]┐ │
│ │ Text: [What is Solana?______________]  │ │
│ │                                        │ │
│ │ ○ Option A: [A high-performance L1___] │ │
│ │ ● Option B: [A database______________] │ │
│ │ ○ Option C: [A website_______________] │ │
│ │   [+ Add Option]                       │ │
│ │                                        │ │
│ │ Explanation: [Solana is a fast L1...]  │ │
│ └────────────────────────────────────────┘ │
│                                             │
│ ┌─ Question 2 ───────────────── [↑][↓][✕]┐ │
│ │ ...                                    │ │
│ └────────────────────────────────────────┘ │
│                                             │
│ [+ Add Question]                            │
│                                             │
└─────────────────────────────────────────────┘
```

## Open Questions

- **Question reordering**: Simple up/down buttons vs drag-and-drop? (Up/down is simpler and doesn't need a DnD library)
- **Import/Export**: Allow importing quiz from JSON file? (Nice-to-have, defer)
- **Question bank**: Pre-built questions for common event types? (Defer to multi-event support)

## Risks

| Risk | Mitigation |
|------|------------|
| Large quiz configs overflow KV value size | KV max value is 25MB; quiz configs are typically <10KB |
| Concurrent edits by multiple admins | Last-write-wins is acceptable for single-event quiz |
| Frontend WASM bundle size increase | Quiz editor adds ~5-10KB compressed, acceptable |

## Refs

- Issue 002: Quiz-Gated Claim Flow (backend + claim page quiz UI)
- `worker/src/handlers/quiz.rs` — quiz handlers
- `worker/src/quiz.rs` — KV quiz storage
- `frontend-leptos/src/pages/admin.rs` — admin dashboard
- `frontend-leptos/src/api.rs` — frontend API layer
- `domain/src/models/api.rs` — shared quiz types
