# 043 — PDPA Consent & Data Collection

> **Date**: 2026-05-28
> **Status**: 📋 Planned
> **Priority**: P1 (pre-mainnet — legal compliance)
> **Depends on**: None (can be done anytime)
> **Related**: `.issues/016_attendee_google_auth.md` (attendee identity)

## Summary

Add Thailand PDPA (Personal Data Protection Act, effective June 1, 2022) compliance to BeThere, covering:

1. **Consent checkbox** on registration for collecting personal data
2. **Photo/media consent** for event photography (separate optional consent)
3. **Privacy policy page** (`/privacy`) with data practices disclosure
4. **Data retention policy** — what we store, how long, and how to request deletion

## Problem

BeThere collects significant personal data from attendees:

| Data | Where Stored | Purpose | PDPA Category |
|------|-------------|---------|---------------|
| Name | Google Sheets (B–D) | Registration, check-in badge | Personal |
| Email | Google Sheets (E) | Registration, communication | Personal |
| Phone | Google Sheets (J) | Optional contact | Personal |
| Contact handle (Telegram/Line/etc.) | Google Sheets (K–L) | Staff follow-up | Personal |
| Participation type | Google Sheets (I) | Track assignment | Non-sensitive |
| Wallet address | Google Sheets (T), On-chain | NFT mint, refund | Public (blockchain) |
| Deposit TX signature | Google Sheets (P), On-chain | Payment verification | Public (blockchain) |
| Checked-in timestamp | Google Sheets (R), On-chain | Attendance proof | Public (blockchain) |
| Claim token | Google Sheets (V) | NFT claim access | Security credential |
| Google OAuth token | Worker session | Authentication | Sensitive |
| Payment slip photo | Worker KV (THB flow) | Deposit verification | Sensitive (financial) |

Under Thailand's PDPA:
- **Consent is required** before collecting personal data (Section 19)
- **Purpose limitation** — data can only be used for the stated purpose
- **Photo/video at events** — requires separate explicit consent (Section 20, sensitive data if biometric)
- **Data subject rights** — attendees can request access, correction, deletion of their data
- **Privacy notice** — must be available before or at the time of collection

## Proposed Solution

### Phase A: Consent Checkbox (Registration)

**Status: ✅ Implemented (commit `b2cb1a7`)**

Added mandatory consent checkbox + optional marketing consent to the registration form.

| What | Status |
|------|--------|
| `consent_given` — mandatory data collection consent | ✅ Done |
| `photo_consent_given` — per-event photo/media consent | ✅ Done |
| `consent_marketing` — optional future event marketing consent | ✅ Backend (field added to `RegisterRequest`, D1 migration `0005`) |
| Frontend checkbox for marketing consent | 📋 Next (Phase A frontend) |

### Phase B: Photo/Media Consent (Optional, Per-Event)

**Status: ✅ Implemented (commit `b2cb1a7`)**

| What | Status |
|------|--------|
| `require_photo_consent` event config field | ✅ Done |
| Frontend toggle in organizer event form | ✅ Done |
| Registration form conditional photo consent checkbox | ✅ Done |

### Phase C: Privacy Policy Page

**Status: ✅ Implemented (commit `b2cb1a7`)**

Public `/privacy` page with 11-section PDPA-compliant notice. Frontend Leptos page with full disclosure of data practices.

### Phase D: Data Retention & Deletion API

**Status: ✅ Implemented (commit `c944716` + time-gate update)**

| Area | Change | Status |
|------|--------|--------|
| `worker/src/handlers/privacy.rs` | `POST /api/privacy/delete-request` — self-service PDPA erasure | ✅ |
| `worker/src/db/attendees.rs` | `get_attendees_by_email`, `clear_attendee_pii` | ✅ |
| `worker/src/db/contacts.rs` | `clear_contact_pii` | ✅ |
| `worker/src/db/developers.rs` | `clear_developer_pii`, `delete_developer_responses` | ✅ |
| `worker/src/sheets/write.rs` | `clear_sheet_cells_batch` — Google Sheets batch clear | ✅ |
| `worker/src/audit_store.rs` | `AuditAction::DataDeletionRequested` | ✅ |
| Time-gate: deletion only available after event ends | ✅ PDPA §38 exemption |
| Frontend (Phase D UI) | "Request Data Deletion" section | 📋 Next |

**Time-gate design:**

| Period | Deletion Right | Rationale |
|--------|---------------|----------|
| Before event | ❌ Blocked | Contract performance (PDPA §38) |
| During event | ❌ Blocked | Active operational requirement |
| Post-event | ✅ Allowed | No longer contractually necessary |

Response includes `blocked_events` with `event_id`, `event_name`, `event_end_ms` for each blocked event, allowing the frontend to show "available after [date]".

### Phase E: Marketing Consent & Unsubscribe

**Status: ✅ Backend implemented**

| Area | Change | Status |
|------|--------|--------|
| `worker/migrations/0005_marketing_consent.sql` | `consent_marketing`, `consent_marketing_at` columns | ✅ |
| `worker/src/db/attendees.rs` | `set_marketing_consent()` — batch update by email | ✅ |
| `worker/src/handlers/privacy.rs` | `POST /api/privacy/unsubscribe-marketing` | ✅ |
| `worker/src/handlers/register.rs` | `consent_marketing` field on `RegisterRequest` | ✅ |
| `worker/src/audit_store.rs` | `AuditAction::MarketingUnsubscribed` | ✅ |
| `worker/src/handlers/mod.rs` | Route wired in attendee-authed router | ✅ |
| Frontend marketing consent checkbox | Registration form | 📋 Next |
| Frontend unsubscribe UI | "Unsubscribe from marketing" section | 📋 Next |

**Two consent layers:**

| Consent Type | When Collected | What It Allows | Can Withdraw? |
|-------------|---------------|----------------|---------------|
| **Registration consent** (`consent_given`) | At sign-up (mandatory) | Process event, check-in, deposits/refunds | Only after event ends |
| **Marketing consent** (`consent_marketing`) | At sign-up (optional) | Promote new events, interest-based outreach | ✅ Anytime — `/api/privacy/unsubscribe-marketing` |

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Consent granularity | **Two-tier**: data collection (mandatory) + photo (optional) | PDPA requires specific consent per purpose. Photo consent is separate from registration consent. |
| Photo consent default | **Opt-in** (not checked by default) | PDPA principle: consent must be freely given, not pre-checked. |
| Blockchain immutability | **Disclosed, not solved** | Cannot delete on-chain data. PDPA allows exemptions for technical impossibility (Section 37). Must disclose in privacy policy. |
| Data retention period | **Until event conclusion + 90 days** | Reasonable for refund processing and dispute resolution. Then PII is cleared from sheets. |
| Deletion mechanism | **Self-service + email fallback** | Attendees can request via UI or email. Organizer cooperation needed for sheet data. |
| Consent storage | **Google Sheets column** | Simple, transparent, auditable. Organizer can see who consented. |
| Organizer responsibility | **Shared** | BeThere is a data processor; the organizer is the data controller. Both have obligations. |

## PDPA Compliance Checklist

| Requirement | PDPA Section | Status |
|-------------|-------------|--------|
| Legal basis for collection | §19 | ✅ Phase A (consent checkbox) |
| Privacy notice before collection | §23 | ✅ Phase C (privacy policy) |
| Purpose limitation | §23 | ✅ Already scoped to event operations |
| Data minimization | §23 | ✅ Contact fields are optional |
| Consent for sensitive data (photos) | §20, §26 | ✅ Phase B (photo consent) |
| Right of access | §27 | ✅ Phase D (deletion API) |
| Right to correct | §28 | ✅ Organizer can edit Google Sheets |
| Right to delete | §29 | ✅ Phase D (time-gated deletion API) |
| Right to withdraw consent | §30 | ✅ Phase E (marketing unsubscribe) |
| Data breach notification | §31 | ✅ Cloudflare provides monitoring |
| Data Protection Officer | §32 | 📋 Organizational — not a code change |
| Cross-border transfer | §33 | ✅ Disclosed in privacy policy |
| Data retention limit | §24 | ✅ Phase D (time-gate = retention until event ends) |
| Marketing consent (separate purpose) | §19, §23 | ✅ Phase E (separate opt-in) |

## Google Sheet Column Updates

| Column | Index | Field | Phase |
|--------|-------|-------|-------|
| AE | 30 | `consent_given` | A |
| AF | 31 | `photo_consent` | B |

## Effort Summary

| Phase | Description | Effort | Status |
|-------|-------------|--------|--------|
| A | Consent checkbox | ~3h | ✅ Done |
| B | Photo/media consent | ~3.5h | ✅ Done |
| C | Privacy policy page | ~2.75h | ✅ Done |
| D | Data retention & deletion (time-gated) | ~4h | ✅ Done |
| E | Marketing consent & unsubscribe | ~2h | ✅ Backend done |
| Frontend | Marketing checkbox + unsubscribe UI + deletion UI | ~3h | 📋 Next |
| **Total backend** | | **~15.25h** | |
| **Remaining** | | **~3h frontend** | |

## References

- [Thailand PDPA (Personal Data Protection Act B.E. 2562)](https://www.pdpc.or.th/)
- [PDPA English Summary](https://www.pdpc.or.th/645-2/)
- [PDPA Consent Requirements](https://www.pdpc.or.th/consent/)
- [GDPR vs PDPA Comparison](https://www.dataguidance.com/comparisons/thailand)

## Relationship to Other Docs

| Document | Relationship |
|----------|-------------|
| `README.md` §Google Sheet Layout | Column additions (AE–AF) |
| `DISCUSSION.md` §10 | Attendee journey — consent step needed before deposit |
| `.issues/016_attendee_google_auth.md` | Google OAuth collects email — needs consent linkage |
| `.issues/042_solana_mobile_support.md` | Mobile registration — consent checkbox must be mobile-friendly |
| `docs/security_audit.md` | Data handling is a security concern |
