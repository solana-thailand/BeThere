# Issue 002: Quiz-Gated Claim Flow

> **Status: ✅ COMPLETE** — Deployed to production (`680bfb89`). Quiz flow tested end-to-end: check-in → quiz (5 questions, 60% pass, 3 attempts) → wallet claim → mint. Bug fixes: KV boolean corruption, serde default, submit counter reactivity, spinner transform conflict.

## Problem

The current check-in flow only proves an attendee *showed up* — they could be scanned and leave 30 seconds later. With the deposit-refund model (Issue 001), organizers need proof that attendees actually **engaged with the content**, not just physically entered the venue.

## Solution

Require attendees to complete a short quiz/evaluation after check-in before they can claim their refund + badge. Questions are written by the organizer and test comprehension of event content (talks, panels, workshops).

### Flow

```
Current:  Scan QR → Checked In → Claim Badge
Proposed: Scan QR → Checked In → Complete Quiz → Claim Refund + Badge
```

The quiz appears on the claim page after the attendee opens their claim link. They must answer all questions and meet the passing threshold to unlock the claim button.

## Architecture

### Storage: Cloudflare KV

Quiz questions are stored in a Cloudflare KV namespace called `QUIZ`. This is the lightest-weight option — no schema migrations, no D1 setup, per-event quiz config via API or wrangler.

**Why KV over Google Sheets tab or env vars?**
- KV: Simple JSON, programmatic writes (admin API), instant reads, free tier covers thousands of events
- Google Sheets tab: Overkill for a flat list of questions, slow batchget API, column-based format is awkward for nested data
- Env vars: Requires `wrangler secret put` per change, no admin UI, can't be changed at runtime

### Data Model

```
KV Key: "questions"
KV Value: QuizConfig {
  questions: Vec<QuizQuestion>,
  passing_score_percent: u8,  // e.g. 70 = 70% correct required
  max_attempts: u8,           // e.g. 3 attempts allowed
  time_limit_seconds: Option<u16>,  // optional per-attempt timer
}

QuizQuestion {
  id: String,           // e.g. "q1", "q2"
  text: String,         // "What consensus mechanism does Solana use?"
  options: Vec<String>, // ["Proof of Work", "Proof of History", "Proof of Stake", "PBFT"]
  correct_index: u8,    // 1 (index into options, NOT sent to frontend)
  explanation: Option<String>,  // shown after submission
}

KV Key: "progress:{claim_token}"
KV Value: QuizProgress {
  claim_token: String,
  attempts: u8,
  best_score_percent: u8,
  passed: bool,
  passed_at: Option<String>,   // ISO 8601 timestamp
  answers: Vec<QuizAttempt>,   // history of attempts
}

QuizAttempt {
  attempt_number: u8,
  answers: Vec<u8>,      // selected option index per question
  score_percent: u8,
  submitted_at: String,
}
```

### API Endpoints

**New endpoints (public — no auth):**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/quiz` | Get quiz questions (options only, no correct answers) |
| POST | `/api/quiz/{token}/submit` | Submit answers, get score + pass/fail |
| GET | `/api/quiz/{token}/status` | Get current quiz progress (attempts, passed) |

**Modified endpoints:**

| Method | Path | Change |
|--------|------|--------|
| GET | `/api/claim/{token}` | Add `quiz_status` field (not_started / in_progress / passed) |
| POST | `/api/claim/{token}` | Gate: reject if quiz not passed |

### Request/Response Types

```rust
// GET /api/quiz
QuizQuestionsResponse {
    questions: Vec<QuizQuestionPublic>,
    passing_score_percent: u8,
    max_attempts: u8,
    time_limit_seconds: Option<u16>,
}

QuizQuestionPublic {
    id: String,
    text: String,
    options: Vec<String>,
    // NOTE: no correct_index — server validates
}

// POST /api/quiz/{token}/submit
QuizSubmitRequest {
    answers: Vec<QuizAnswer>,  // one per question
}

QuizAnswer {
    question_id: String,
    selected_index: u8,
}

QuizSubmitResponse {
    attempt_number: u8,
    score_percent: u8,
    passed: bool,
    correct_count: usize,
    total_questions: usize,
    remaining_attempts: u8,
    explanations: Vec<QuestionExplanation>,
}

QuestionExplanation {
    question_id: String,
    correct: bool,
    explanation: Option<String>,
}

// GET /api/quiz/{token}/status
QuizStatusResponse {
    attempts: u8,
    max_attempts: u8,
    best_score_percent: u8,
    passed: bool,
    passing_threshold_percent: u8,
}

// Modified: GET /api/claim/{token} adds:
ClaimLookupResponse {
    // ... existing fields ...
    quiz_status: QuizStatus,  // new
}

enum QuizStatus {
    NotRequired,     // no quiz configured for this event
    NotStarted,      // quiz exists, attendee hasn't attempted
    InProgress,      // quiz exists, attendee attempted but hasn't passed
    Passed,          // quiz passed, claim unlocked
}
```

### Anti-Cheat Measures

1. **Server-side validation** — correct answers never leave the server
2. **Shuffled option order** — frontend randomizes display order, sends back the selected option text (not index). Server matches text to determine correctness.
3. **Max attempts** — configurable per event (default: 3)
4. **Rate limiting** — 1 submission per 10 seconds per token
5. **No answer reveal until submission** — explanations shown after each attempt, but not which specific option is correct

### Frontend Changes (claim.rs)

The claim page gets a new state between "Ready" and the mint button:

```
ClaimState::Ready(data) {
    if quiz_status == NotRequired || Passed {
        // Show wallet input + mint button (current flow)
    } else {
        // Show quiz UI
        // On pass → transition to wallet input + mint
    }
}
```

**Quiz UI components:**
- Question card with numbered options (radio buttons)
- Progress indicator (question X of Y)
- Timer (if configured)
- Submit button
- Results card with score, pass/fail, explanations
- Retry button (if attempts remaining)

**Claim flow with quiz:**
1. Attendee opens claim link → sees welcome card (checked in ✓)
2. Quiz section appears below → "Complete the quiz to unlock your badge"
3. Answer all questions → submit
4. If passed → wallet input appears + "Claim Refund + Badge" button
5. If failed → show score + explanations + retry (if attempts left)

### Admin Endpoint (protected — staff auth)

| Method | Path | Description |
|--------|------|-------------|
| PUT | `/api/admin/quiz` | Create/update quiz questions |
| GET | `/api/admin/quiz/stats` | Quiz completion stats across all attendees |

This lets organizers set up the quiz before the event via an admin UI or curl.

## Wrangler Config Changes

```toml
# wrangler.toml — add KV namespace binding
[[kv_namespaces]]
binding = "QUIZ"
id = "<namespace_id>"        # created via: wrangler kv namespace create QUIZ
preview_id = "<preview_id>"  # created via: wrangler kv namespace create QUIZ --preview
```

## Implementation Phases

### Phase 1 — Backend Quiz API ✅ COMPLETE
- [x] Create KV namespace, add binding to `wrangler.toml`
- [x] Add quiz types to `domain/src/models/api.rs`
- [x] Create `worker/src/handlers/quiz.rs` with GET/POST/STATUS endpoints
- [x] Create `worker/src/quiz.rs` module for KV read/write logic
- [x] Modify `worker/src/handlers/claim.rs` — gate claim on quiz status
- [x] Modify `worker/src/state.rs` — add KV binding
- [x] Add admin endpoint for quiz setup (PUT /api/admin/quiz)

### Phase 2 — Frontend Quiz UI ✅ COMPLETE
- [x] Add quiz API functions to `frontend-leptos/src/api.rs`
- [x] Add `QuizQuestion`, `QuizSubmit`, `QuizStatus` types
- [x] Create quiz components in claim.rs:
  - `QuizView` — extracted component for quiz question flow
  - `QuizSubmittedView` — extracted component for results + retry/claim
  - `build_quiz_questions` — pre-rendered question cards
  - `build_quiz_explanations` — pre-rendered answer review
  - `build_quiz_action` — pre-rendered retry/claim/exhausted actions
- [x] Modify `Claim` component — add quiz state machine (Quiz, QuizSubmitted states)
- [x] CSS for quiz UI (`.claim-quiz-*` classes in `style.css`)
- [x] Mobile responsive quiz CSS (359px + 480px breakpoints)
- [ ] Timer component (if time_limit configured) — deferred

### Phase 3 — Integration & Polish ✅ COMPLETE
- [x] End-to-end test: check-in → claim link → quiz → mint
- [x] Admin curl workflow for quiz setup (see below)
- [ ] Quiz stats on admin dashboard — deferred
- [x] Edge cases: quiz config changed mid-event, no quiz configured (graceful fallback)
- [x] Mobile responsiveness for quiz cards
- [x] Landing page updated — "Complete the Quiz" step added

### Phase 4 — Production ✅ COMPLETE
- [x] Create production KV namespace (`npx wrangler kv namespace create QUIZ`)
- [x] Create preview KV namespace (`npx wrangler kv namespace create QUIZ --preview`)
- [x] Replace placeholder IDs in `wrangler.toml`
- [x] Set quiz questions for first event (use curl command below)
- [x] Test on staging with real attendee flow
- [x] Deploy

**Total estimate: 6-9 days**

## Example Quiz (Solana x AI Builders Event)

```json
{
  "questions": [
    {
      "id": "q1",
      "text": "What consensus mechanism does Solana use to achieve high throughput?",
      "options": [
        "Proof of Work",
        "Proof of History",
        "Delegated Proof of Stake",
        "Practical Byzantine Fault Tolerance"
      ],
      "correct_index": 1,
      "explanation": "Solana uses Proof of History (PoH) as a cryptographic clock to order transactions, combined with Tower BFT consensus."
    },
    {
      "id": "q2",
      "text": "What is the primary advantage of compressed NFTs on Solana?",
      "options": [
        "They are encrypted for privacy",
        "They cost significantly less to mint at scale",
        "They can only be held by validators",
        "They use a different blockchain"
      ],
      "correct_index": 1,
      "explanation": "Compressed NFTs use Merkle trees to reduce storage costs by ~100-500x, making large-scale distribution practical."
    },
    {
      "id": "q3",
      "text": "Which programming language is used to write Solana smart contracts (programs)?",
      "options": [
        "JavaScript",
        "Python",
        "Rust",
        "Go"
      ],
      "correct_index": 2,
      "explanation": "Solana programs are primarily written in Rust, compiled to BPF bytecode, and deployed on-chain."
    },
    {
      "id": "q4",
      "text": "What does an AI agent need to interact with a Solana program?",
      "options": [
        "A web browser",
        "An IDL (Interface Description Language) and RPC connection",
        "A physical hardware wallet",
        "Permission from the program author"
      ],
      "correct_index": 1,
      "explanation": "AI agents use the program's IDL to serialize/deserialize instructions and communicate via RPC."
    },
    {
      "id": "q5",
      "text": "What is the minimum deposit requirement concept in BeThere?",
      "options": [
        "Attendees pay for their NFT",
        "Attendees lock a deposit, get it back if they show up",
        "Organizers pay to create events",
        "Staff pay to use the scanner"
      ],
      "correct_index": 1,
      "explanation": "BeThere's deposit model: attendees commit money upfront, get refunded when they show up and complete the quiz. No-shows forfeit their deposit."
    }
  ],
  "passing_score_percent": 60,
  "max_attempts": 3,
  "time_limit_seconds": null
}
```

## Quiz Setup Instructions

### 1. Create KV Namespaces

```bash
# Production namespace
npx wrangler kv namespace create QUIZ
# Copy the returned id into wrangler.toml

# Preview namespace (for wrangler dev)
npx wrangler kv namespace create QUIZ --preview
# Copy the returned preview_id into wrangler.toml
```

### 2. Set Quiz Questions (Admin)

```bash
# Replace <JWT> with a valid staff JWT token
# Replace <BASE_URL> with your worker URL (e.g. http://localhost:8787 for local dev)

curl -X PUT "<BASE_URL>/api/admin/quiz" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <JWT>" \
  -d '{
    "questions": [
      {
        "id": "q1",
        "text": "What consensus mechanism does Solana use to achieve high throughput?",
        "options": [
          "Proof of Work",
          "Proof of History",
          "Delegated Proof of Stake",
          "Practical Byzantine Fault Tolerance"
        ],
        "correct_index": 1,
        "explanation": "Solana uses Proof of History (PoH) as a cryptographic clock to order transactions, combined with Tower BFT consensus."
      },
      {
        "id": "q2",
        "text": "What is the primary advantage of compressed NFTs on Solana?",
        "options": [
          "They are encrypted for privacy",
          "They cost significantly less to mint at scale",
          "They can only be held by validators",
          "They use a different blockchain"
        ],
        "correct_index": 1,
        "explanation": "Compressed NFTs use Merkle trees to reduce storage costs by ~100-500x, making large-scale distribution practical."
      },
      {
        "id": "q3",
        "text": "Which programming language is used to write Solana smart contracts (programs)?",
        "options": [
          "JavaScript",
          "Python",
          "Rust",
          "Go"
        ],
        "correct_index": 2,
        "explanation": "Solana programs are primarily written in Rust, compiled to BPF bytecode, and deployed on-chain."
      },
      {
        "id": "q4",
        "text": "What does an AI agent need to interact with a Solana program?",
        "options": [
          "A web browser",
          "An IDL (Interface Description Language) and RPC connection",
          "A physical hardware wallet",
          "Permission from the program author"
        ],
        "correct_index": 1,
        "explanation": "AI agents use the program's IDL to serialize/deserialize instructions and communicate via RPC."
      },
      {
        "id": "q5",
        "text": "What is the minimum deposit requirement concept in BeThere?",
        "options": [
          "Attendees pay for their NFT",
          "Attendees lock a deposit, get it back if they show up",
          "Organizers pay to create events",
          "Staff pay to use the scanner"
        ],
        "correct_index": 1,
        "explanation": "BeThere's deposit model: attendees commit money upfront, get refunded when they show up and complete the quiz. No-shows forfeit their deposit."
      }
    ],
    "passing_score_percent": 60,
    "max_attempts": 3,
    "time_limit_seconds": null
  }'
```

### 3. Verify Quiz is Set

```bash
# Public — no auth needed
curl "<BASE_URL>/api/quiz"
```

### 4. Test Submit (requires valid claim token from a checked-in attendee)

```bash
curl -X POST "<BASE_URL>/api/quiz/<CLAIM_TOKEN>/submit" \
  -H "Content-Type: application/json" \
  -d '{
    "answers": [
      { "question_id": "q1", "selected_text": "Proof of History" },
      { "question_id": "q2", "selected_text": "They cost significantly less to mint at scale" },
      { "question_id": "q3", "selected_text": "Rust" },
      { "question_id": "q4", "selected_text": "An IDL (Interface Description Language) and RPC connection" },
      { "question_id": "q5", "selected_text": "Attendees lock a deposit, get it back if they show up" }
    ]
  }'
```

## Open Questions

| Question | Options | Default |
|----------|---------|---------|
| Should quiz be mandatory or optional per event? | Configurable via quiz existence in KV | Optional (no KV entry = no quiz) |
| Should questions be shuffled per attempt? | Yes (anti-cheat) vs No (simpler) | Yes — shuffle question order |
| Should options be shuffled per question? | Yes (anti-cheat) vs No (simpler) | Yes — shuffle option order, server matches by text |
| Retake policy: same questions or new subset? | Same questions vs random subset | Same questions (easier to implement) |
| What if attendee exhausts all attempts? | Lock forever vs admin override | Admin override via API + organizer contact |
| Quiz available before event starts? | Yes (prep) vs No (must be at event) | Only after check-in (enforced server-side) |

## Risks

| Risk | Mitigation |
|------|-----------|
| Attendee can't pass quiz (too hard) | Configurable passing score, max attempts, explanations after each attempt |
| Quiz answers leaked / shared among attendees | Shuffle order, time limit, server-side validation |
| KV read latency on claim page | KV has ~ms read latency globally, cache in worker memory for event duration |
| Organizer writes bad questions | Admin UI with preview, test attempt before event |
| Quiz gate blocks legit claims (organizer forgot to set quiz) | If no quiz in KV, quiz_status = NotRequired, claim works normally |

## Refs

- Claim flow: `frontend-leptos/src/pages/claim.rs`
- Claim API: `worker/src/handlers/claim.rs`
- Domain types: `domain/src/models/api.rs`
- Worker state: `worker/src/state.rs`
- Wrangler config: `worker/wrangler.toml`
- Deposit/refund spec: `.issues/001_deposit_commitment_refund.md`
