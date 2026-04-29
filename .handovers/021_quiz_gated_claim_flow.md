# Handover 021: Quiz-Gated Claim Flow (Issue 002)

## What Happened

Implemented **Issue 002: Quiz-Gated Claim Flow** — a feature requiring event attendees to complete a short quiz after check-in before they can claim their NFT badge and deposit refund. The rationale is to prove attendees actually engaged with event content, not just physically showed up.

**Flow change:**

```
Before:  Scan QR → Checked In → Claim Badge
After:   Scan QR → Checked In → Complete Quiz → Claim Refund + Badge
```

The session started with the frontend blocked by a **Leptos `view!` macro unclosed delimiter error** in `claim.rs`. The backend was already fully implemented. The primary work was fixing the frontend build, adding quiz CSS, updating the landing page, and preparing deployment documentation.

## Changes Made

### 1. Backend Quiz API (`worker/src/handlers/quiz.rs`, `worker/src/quiz.rs`)

New files implementing the full quiz backend:

| File | Purpose |
|------|---------|
| `worker/src/handlers/quiz.rs` | HTTP handlers for quiz endpoints |
| `worker/src/quiz.rs` | Quiz business logic (validation, scoring, KV interaction) |

**API Endpoints:**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/quiz` | Get quiz questions (options only, no correct answers) |
| `POST` | `/api/quiz/{token}/submit` | Submit answers, get score + pass/fail + explanations |
| `GET` | `/api/quiz/{token}/status` | Get quiz progress (attempts, best score, passed) |
| `PUT` | `/api/admin/quiz` | Create/update quiz (staff auth required) |

### 2. Frontend Quiz UI (`frontend-leptos/src/pages/claim.rs`)

**The blocking issue — Leptos `view!` macro:**

The `view!` proc macro cannot handle deeply nested structures inside `match` arms within reactive closures (`{move || { match state.get() { ... }}}`). Specifically:

- `view!` blocks inside `{if ... { } else { }}` conditionals
- Nested `view!` inside `.map()` closures
- `class=move || { if ... { "..." } else { "..." } }` patterns

All cause the macro to miscount braces.

**Solution — two-layer extraction:**

1. **Component extraction** — `QuizView` and `QuizSubmittedView` are separate `#[component]` functions, each with their own simpler `view!` block. The main `Claim` component's `view!` now just has `<QuizView .../>` and `<QuizSubmittedView .../>` in the match arms.

2. **Pre-rendering inside components** — Within `QuizSubmittedView`, conditional views (`result_icon`, `retry_info_view`, `explanations_view`, `action_view`) are computed as `AnyView` variables *before* the `view!` macro invocation, then inserted with `{variable_name}`.

**ClaimState extended:**

```rust
enum ClaimState {
    // ... existing variants ...
    Quiz(ClaimLookupData, QuizQuestionsData),
    QuizSubmitted(ClaimLookupData, QuizQuestionsData, QuizSubmitData),
}
```

**Bug fixes found during implementation:**
- Added missing `.clone()` in `build_quiz_questions` on:click closure
- Added missing `.clone()` in `build_quiz_action` Minting state transition
- Added missing `quiz_kv: None` in test `AppState` (`worker/src/auth.rs`)

### 3. API Types (`domain/src/models/api.rs`, `frontend-leptos/src/api.rs`)

New types for quiz data:

| Type | Purpose |
|------|---------|
| `QuizQuestion` | Single question with id, text, options, explanation |
| `QuizQuestionsData` | Full quiz: questions + passing_score_percent + max_attempts |
| `QuizSubmitData` | Submit request: answers map + result + explanations |
| `QuizStatus` | Enum: NotRequired, NotStarted, InProgress, Passed |
| `QuizAnswers` | Type alias: `HashMap<String, String>` (question_id → option text) |

### 4. Claim Handler Update (`worker/src/handlers/claim.rs`)

Modified `GET /api/claim/{token}` to check quiz status:
- If quiz is configured in KV → returns `quiz_status: NotStarted/InProgress/Passed`
- If no quiz configured → returns `quiz_status: NotRequired` (backward compatible)
- KV binding is `Option<KvStore>` — Worker starts fine without it

### 5. Worker State & Wiring (`worker/src/state.rs`, `worker/src/lib.rs`)

- Added `quiz_kv: Option<KvStore>` to `AppState`
- Added `env.kv("QUIZ")` binding in `AppState::new()`
- Added `quiz` module to handlers

### 6. CSS Styling (`frontend-leptos/style.css`)

~250 lines of new CSS for 28 `.claim-quiz-*` classes:

| Category | Classes |
|----------|---------|
| Intro card | `.claim-quiz-intro`, `.claim-quiz-intro h3`, `.claim-quiz-intro p` |
| Questions | `.claim-quiz-question`, `.claim-quiz-question-number`, `.claim-quiz-question-text` |
| Options | `.claim-quiz-option`, `.claim-quiz-option.selected`, `.claim-quiz-option.correct`, `.claim-quiz-option.wrong` |
| Results | `.claim-quiz-result`, `.claim-quiz-result.passed`, `.claim-quiz-result.failed` |
| Explanations | `.claim-quiz-explanation`, `.claim-quiz-explanation.correct`, `.claim-quiz-explanation.wrong` |
| Retry/exhausted | `.claim-quiz-retry`, `.claim-quiz-exhausted` |
| Progress | `.claim-quiz-progress` |

Mobile responsive overrides at 359px and 480px breakpoints.

### 7. Landing Page Update (`frontend-leptos/src/pages/landing.rs`)

- "How it works" expanded from 3 → 4 steps (added "Complete the Quiz")
- Hero description updated
- Attendee section updated

### 8. Wrangler Config (`worker/wrangler.toml`)

Added KV namespace binding with placeholder IDs:

```toml
[[kv_namespaces]]
binding = "QUIZ"
id = "<QUIZ_KV_NAMESPACE_ID>"
preview_id = "<QUIZ_KV_PREVIEW_NAMESPACE_ID>"
```

## Build Status

| Component | Status |
|-----------|--------|
| `event-checkin-domain` | ✅ 0 errors, 0 warnings |
| `event-checkin-worker` | ✅ 0 errors, 0 warnings |
| `event-checkin-frontend` | ✅ 0 errors, 0 warnings |

## Files Modified

| File | Change |
|------|--------|
| `domain/src/models/api.rs` | +156 lines — quiz types |
| `frontend-leptos/src/api.rs` | +210 lines — quiz API functions |
| `frontend-leptos/src/pages/claim.rs` | +583 lines — QuizView + QuizSubmittedView components |
| `frontend-leptos/src/pages/landing.rs` | +28 lines — quiz step in how-it-works |
| `frontend-leptos/style.css` | +323 lines — quiz CSS + responsive |
| `worker/src/auth.rs` | +1 line — quiz_kv: None in test state |
| `worker/src/handlers/claim.rs` | +49 lines — quiz status in claim lookup |
| `worker/src/handlers/mod.rs` | +7 lines — quiz handler wiring |
| `worker/src/lib.rs` | +1 line — quiz module |
| `worker/src/state.rs` | +9 lines — quiz KV binding |
| `worker/wrangler.toml` | +9 lines — KV namespace config |
| `worker/src/handlers/quiz.rs` | **NEW** — quiz HTTP handlers |
| `worker/src/quiz.rs` | **NEW** — quiz business logic |
| `.issues/002_quiz_gated_claim.md` | **NEW** — issue documentation |

## Architecture Decisions

### Storage: Cloudflare KV
- JSON read/write, no schema, free tier sufficient
- KV binding is optional (`Option<KvStore>`) — Worker starts fine without it
- Quiz feature gracefully disabled when no quiz is configured

### Answer Matching: Server-Side by Text
- Frontend can shuffle option order freely
- Server compares selected text against correct option text
- Correct answers never sent to frontend — prevents answer leaking via network inspector

### Backward Compatibility
- No quiz configured → `quiz_status: NotRequired`, claim works normally
- Existing events without quiz are completely unaffected

### Quiz State Machine
```
NotRequired → NotStarted → InProgress → Passed
                                         ↓ (if failed, back to InProgress)
```

## Anti-Cheat Measures
1. **Correct answers never sent to frontend** — only explanations shown after submission
2. **Server-side scoring** — client cannot tamper with results
3. **Max attempts** — configurable per quiz (default 3)
4. **Attempt tracking in KV** — per-token attempt counter persists across page refreshes

## Deployment Steps (Remaining)

### 1. Create KV Namespaces
```bash
npx wrangler kv namespace create QUIZ
npx wrangler kv namespace create QUIZ --preview
```

### 2. Replace Placeholder IDs in `wrangler.toml`
Replace `<QUIZ_KV_NAMESPACE_ID>` and `<QUIZ_KV_PREVIEW_NAMESPACE_ID>` with returned IDs.

### 3. Set Quiz Questions (Admin)
```bash
curl -X PUT https://bethere.solana-thailand.workers.dev/api/admin/quiz \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <admin-jwt>" \
  -d @quiz.json
```

Example quiz JSON documented in `.issues/002_quiz_gated_claim.md`.

### 4. End-to-End Test
Full flow: check-in → claim link → quiz → mint.

## Testing Notes

### Manual Test Checklist
- [ ] Event without quiz configured → claim works normally (NotRequired)
- [ ] Quiz configured → claim shows quiz intro
- [ ] Select answers → submit → see results with explanations
- [ ] Pass quiz → transition to claim/mint
- [ ] Fail quiz → retry with attempt counter
- [ ] Exhaust all attempts → blocked from claiming
- [ ] Mobile: 320px, 375px, 414px widths
- [ ] Quiz config changed mid-event → existing progress preserved
- [ ] Quiz deleted mid-event → falls back to NotRequired

### Edge Cases
- Quiz config changed while attendee is mid-quiz
- Quiz deleted while attendee has attempts but not passed
- Attendee refreshes page during quiz (answers should persist client-side via signal state)
- Multiple tabs with same claim token

## Reflections

### What Struggled
- **Leptos `view!` macro** — deeply nested reactive closures with conditional views cause the proc macro to lose track of braces. Required significant refactoring to extract components.
- **Pre-rendering pattern** — had to learn that computing `AnyView` variables *before* the `view!` invocation is the reliable approach for conditional content.

### What Solved It
- Component extraction (separate `#[component]` functions)
- Pre-rendering conditional views as `AnyView` variables outside `view!`
- Avoiding `view!` blocks inside `match`/`if` expressions within the outer `view!`

## Refs

- Issue: `.issues/002_quiz_gated_claim.md`
- Cloudflare KV docs: https://developers.cloudflare.com/kv/
- Leptos 0.8 component model: https://leptos-rs.github.io/leptos/