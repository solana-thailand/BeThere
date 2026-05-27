# Issue 038: Curriculum Design Vision — Future Reference

> **Status: 🔮 Future Reference** — Not a committed implementation plan. Captured for institutional partnerships and customer discovery.
> **Prerequisite**: Phases 10–12 shipped (mainnet, platform fees, multi-org).
> **Trigger**: University or bootcamp partnership requests OBE/credit/RPL features.

## Context

BeThere already has a **micro-credential system in embryo**:

- **Rust Adventures**: 10 progressive levels teaching Rust (Hello World → Traits)
- **Quiz gating**: Competency verification before NFT claim
- **cNFT badges**: On-chain verifiable attendance proof ($0.001/mint)
- **Rolling deposit credit** (Issue #032): Off-chain credit tracking per attendee
- **Adventure progress**: KV-tracked levels completed, scores, star ratings

These building blocks can be extended into a full curriculum platform for institutions that need OBE compliance, credit banking, and quality assurance. This document captures the vision so it's not lost, but **does not commit to building it** until real demand exists.

---

## Five Pillars

### Pillar 1 — Outcome-Based / Competency-Based Curricula (OBE/CBE)

Define learning outcomes per course/program. Design assessments that measure actual competencies, not just knowledge recall.

**Key concepts:**
- Each learning outcome tagged with Bloom's Taxonomy level (Remember → Create)
- Multi-method assessment per LO (quiz + adventure + portfolio)
- Competency threshold: per-LO minimum + overall composite
- Passing criteria configurable per curriculum

**Data model sketch:**
```rust
struct CurriculumConfig {
    curriculum_id: String,
    title: String,
    total_credits: f32,
    standards: Vec<QaStandard>,          // AUN-QA, TQF, etc.
    learning_outcomes: Vec<LearningOutcome>,
    modules: Vec<Module>,
    assessments: Vec<Assessment>,
    passing_criteria: PassingCriteria,
}

struct LearningOutcome {
    code: String,                         // "LO1"
    description: String,
    bloom_level: BloomLevel,              // Remember|Understand|Apply|Analyze|Evaluate|Create
    competency_criteria: CompetencyCriteria,
    assessed_by: Vec<AssessmentRef>,
}
```

**Effort**: 2-3 weeks for data model + API + admin CRUD.

---

### Pillar 2 — Modular Learning Units (Micro-credentials, Stackable Credentials, Credit Banking)

Each learning unit earns credits that accumulate toward certificates. Credits are transferable.

**Key concepts:**
- Module = adventure level, workshop, short course, or project (self-contained, credit-bearing)
- Credit Bank = per-learner accumulated credits stored in KV (or D1 after Issue #037)
- Stackable Credentials = certificates issued when credit thresholds are met (cNFT minted)
- Credit transfer via API or Verifiable Credentials standard

**Already partially built:**
- Adventure levels = modules (10 exist)
- Rolling deposit credit (#032) = credit tracking pattern
- cNFT badges = credential proof

**Data model sketch:**
```rust
struct Module {
    id: String,
    title: String,
    credits: f32,
    module_type: ModuleType,              // AdventureLevel|Workshop|ShortCourse|Project
    prerequisites: Vec<String>,
    learning_outcomes: Vec<String>,
    estimated_hours: f32,
    delivery_mode: DeliveryMode,          // Online|InPerson|Hybrid|SelfPaced
}

struct CreditBank {
    learner_id: String,
    entries: Vec<CreditEntry>,
    total_credits: f32,
    certificates_earned: Vec<Certificate>,
}

struct Certificate {
    id: String,
    title: String,                         // "Rust Foundation Certificate"
    required_credits: f32,
    required_modules: Vec<String>,
    nft_certificate_tx: Option<String>,    // Solana TX signature
    standard: Option<QaStandard>,
}
```

**Effort**: 2-3 weeks for credit bank + certificate issuance.

---

### Pillar 3 — Learning Programs for Working Adults

Self-paced, problem-based, immediately applicable. Short courses that fit around work schedules.

**Key concepts:**
- Learning pathways (guided sequences with diagnostic pre-assessment)
- Problem-based learning modules (real-world scenarios, not abstract puzzles)
- Short course format (6-20 hours, self-paced or cohort-based)
- Micro-lessons (< 5 min, single concept)

**Enhancement over current adventures:**
| Current | Enhanced |
|---------|----------|
| "Pass values through gates" | "Debug a Solana program that double-borrows an account" |
| "Handle errors to proceed" | "Build a retry mechanism for failed RPC calls" |
| No pacing | Estimated hours + soft milestone deadlines |

**Effort**: 1-2 weeks for course format + pathway routing.

---

### Pillar 4 — Quality Assurance (AUN-QA, TQF, Internal QA)

Curriculum quality assurance meeting international standards required for institutional adoption in Thailand and ASEAN.

**Key concepts:**
- TQF Forms 2–7 generated from curriculum data model
- AUN-QA criterion checklist with scoring
- Internal QA workflow: self-assessment → peer review → committee approval → delivery → evaluation → improvement
- Annual review cycle

**TQF mapping to existing models:**
| TQF Form | Content | BeThere Model |
|----------|---------|---------------|
| Form 2 | Course Specification | `CurriculumConfig` + `Module` |
| Form 3 | Learning Outcomes & Assessment | `LearningOutcome` + `Assessment` |
| Form 4 | Teaching Methods | `DeliveryMode` + `ModuleType` |
| Form 5 | Student Assessment | `Assessment` + `PassingCriteria` |
| Form 6 | Course Evaluation | New: `CourseEvaluation` |
| Form 7 | Course Improvement | New: `ImprovementPlan` |

**Effort**: 2-3 weeks for TQF generation + QA workflow.

---

### Pillar 5 — Recognition of Prior Learning (RPL)

Assess and credit professional experience toward credentials, reducing redundant learning for experienced working adults.

**Key concepts:**
- Portfolio submission (CV, GitHub, certifications, projects)
- Challenge exams (adventure levels under time pressure, or harder quizzes)
- Evidence-based credit award (quantifiable: GitHub stars, cert validity; subjective: assessor review)
- Appeal process with 30-day window

**Data model sketch:**
```rust
struct RplAssessment {
    id: String,
    learner_id: String,
    curriculum_id: String,
    status: RplStatus,                    // Submitted|UnderReview|Approved|PartiallyApproved|Rejected
    evidence: Vec<RplEvidence>,
    challenge_exams: Vec<ChallengeExam>,
    credits_awarded: f32,
    assessed_by: String,
    appeal_deadline: Option<String>,
}

enum RplEvidenceType {
    WorkExperience, Portfolio, Certification,
    GithubProfile, OpenSourceContributions,
    PriorTranscript, InterviewAssessment, ChallengeExam,
}
```

**Effort**: 2-3 weeks for RPL submission + assessor UI + challenge exam integration.

---

## What Already Exists (No Build Needed)

| Curriculum Concept | BeThere Feature | Location |
|---|---|---|
| Learning outcomes | 10 adventure levels with explicit concepts | `.issues/006_rust_adventures.md` |
| Assessments | Quiz-gated claims + adventure puzzle gating | `worker/src/quiz.rs`, `frontend-leptos/src/pages/adventure/` |
| Micro-credentials | cNFT badges per event | `worker/src/solana.rs` |
| Progress tracking | `AdventureProgress` in KV | `domain/src/models/adventure.rs` |
| Competency verification | Sequential gate: check-in → adventure → quiz → NFT | `worker/src/handlers/claim.rs` |
| Credit tracking (deposit) | Rolling deposit credit with `CreditThb`/`CreditUsdc` | `.issues/032_rolling_deposit_credit.md` |
| Modular units | Self-contained adventure levels | `frontend-leptos/src/pages/adventure/levels.rs` |

## What Needs Building (If Triggered)

| Phase | Features | Effort | Trigger |
|---|---|---|---|
| A | Curriculum data model + Module registry + Credit Bank (learning credits) | 2-3 weeks | University asks for credit tracking |
| B | Learning Outcomes + Assessment engine + passing criteria | 2 weeks | Bootcamp wants OBE compliance |
| C | Short course format + self-paced pathways | 1-2 weeks | Working adult program request |
| D | TQF Form generation (Forms 2-7) + QA evaluation workflow | 2-3 weeks | Thai institution partnership |
| E | RPL assessment + challenge exams + evidence review | 2-3 weeks | Professional certification demand |
| F | Stackable credentials + certificate cNFTs + LinkedIn sharing | 1-2 weeks | Learner demand for shareable certs |
| G | QA dashboard + internal audit workflow + AUN-QA alignment | 2 weeks | AUN-QA audit required |

## Architectural Decisions (Pre-decided)

| Decision | Choice | Rationale |
|---|---|---|
| Storage | KV initially, D1 after Issue #037 | Curriculum data is read-heavy. D1 enables SQL queries for analytics. |
| Credits on-chain? | Credit claims off-chain, certificate cNFTs on-chain | Not every credit needs a TX. Only final certificates get cNFTs. |
| TQF output | JSON API → frontend PDF renderer | Don't build PDF server-side. Leptos renders from structured data. |
| RPL evidence | URI references (links, GitHub, IPFS) | Don't store files. Reference and verify on demand. |
| Multilingual | TH + EN for TQF forms, EN for tech content | Thai institutions require Thai documentation. |
| Framework | Extend existing domain models | Add to `domain/src/models/`. No new crate. |

## Risks

| Risk | Mitigation |
|---|---|
| Scope creep into LMS territory | Only build when institutional customer demands it. No speculative features. |
| KV limitations for curriculum queries | Migrate to D1 (Issue #037) first. D1 enables indexed queries. |
| TQF standard changes | Data model is abstract (QaStandard enum). Update mapping, not schema. |
| RPL quality assurance | Require assessor review for subjective evidence. Auto-assess quantifiable only. |
| Academic credibility | Partner with accredited institution. BeThere provides tooling, not accreditation. |

## Related

- `.issues/006_rust_adventures.md` — 10-level Rust curriculum (existing)
- `.issues/032_rolling_deposit_credit.md` — Credit tracking pattern (existing)
- `.issues/037_d1_database_migration.md` — D1 migration enables structured curriculum queries
- `docs/ux_roadmap.md` — LC-1 through LC-4 learning UX items (planned)
- `.issues/031_capstone_project_definition_market_analysis.md` — Target market: universities & bootcamps
