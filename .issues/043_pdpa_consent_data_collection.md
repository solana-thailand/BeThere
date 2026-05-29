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

Add a mandatory consent checkbox to the registration form (`registration_form.rs`) before the submit button.

**What the user sees:**

```
☑ I consent to BeThere collecting my name, email, and contact information
  for event registration, check-in, and NFT issuance. I understand my wallet
  address and transaction data will be recorded on the Solana blockchain
  (public, immutable). View Privacy Policy.

[Reserve My Spot]
```

**Implementation:**

| Area | Change | Effort |
|------|--------|--------|
| `domain/src/models/api.rs` | Add `consent_given: Option<bool>` to `RegisterBody` | 0.5h |
| `worker/src/handlers/` | Validate `consent_given == true` in register handler; reject if missing | 0.5h |
| `frontend-leptos/src/pages/public_event/registration_form.rs` | Add consent checkbox UI + validation (like existing `deposit_agreed`) | 1h |
| `frontend-leptos/src/pages/public_event/types.rs` | Add `consent_given` to `RegisterBody` frontend type | 0.25h |
| Google Sheets | Add `consent_given` column (AE, index 30) to attendee sheet | 0.25h |
| `domain/src/models/attendee.rs` | Add `ColumnKey::ConsentGiven` | 0.5h |

**Total: ~3h**

### Phase B: Photo/Media Consent (Optional, Per-Event)

Add an optional photo/media consent toggle to the registration form. This is controlled by the organizer per event (some events photograph, some don't).

**What the user sees (when organizer enables it):**

```
☑ (Optional) I consent to being photographed/filmed during the event.
  Photos may be used for event promotion on social media and marketing materials.

☐ (Optional) I do NOT consent to being photographed. I understand the venue
  may have general event photography and I should inform staff if I wish to be excluded.
```

**Implementation:**

| Area | Change | Effort |
|------|--------|--------|
| `domain/src/models/event.rs` | Add `require_photo_consent: bool` to `EventConfig` | 0.5h |
| `domain/src/models/api.rs` | Add `photo_consent: Option<bool>` to `RegisterBody` | 0.25h |
| `worker/src/handlers/` | Store photo consent in sheet column | 0.5h |
| `frontend-leptos/src/pages/public_event/registration_form.rs` | Conditional photo consent section | 1h |
| `frontend-leptos/src/pages/event_form.rs` | Organizer toggle for "Collect photo consent" | 0.5h |
| Google Sheets | Add `photo_consent` column (AF, index 31) | 0.25h |
| `domain/src/models/attendee.rs` | Add `ColumnKey::PhotoConsent` | 0.5h |

**Total: ~3.5h**

### Phase C: Privacy Policy Page

Create a public `/privacy` page with BeThere's data practices. This is a PDPA requirement.

**Content should cover:**

| Section | What to Include |
|---------|----------------|
| Data Controller | Organization name, contact |
| Data Collected | List all personal data fields collected |
| Purpose | Registration, check-in, NFT issuance, refund |
| Legal Basis | Consent (PDPA Section 19) + Contract performance |
| Blockchain Data | Explicit notice that wallet addresses and transaction data are public and immutable on Solana |
| Photo/Media | How event photos are handled, consent mechanism |
| Data Retention | How long data is kept (Google Sheets + KV + on-chain) |
| Data Sharing | Google (Sheets), Helius (RPC), wallet providers |
| Data Subject Rights | Access, correction, deletion, withdrawal of consent |
| Contact | How to exercise rights (email/Telegram) |
| Cookie Policy | JWT session cookie, Google OAuth |

**Implementation:**

| Area | Change | Effort |
|------|--------|--------|
| `frontend-leptos/src/pages/` | New `privacy.rs` page (~200 lines of content) | 2h |
| `frontend-leptos/src/lib.rs` | Add `/privacy` route | 0.25h |
| `frontend-leptos/src/components.rs` | Footer link to privacy policy | 0.25h |
| Registration form | Link "View Privacy Policy" to `/privacy` | 0.25h |

**Total: ~2.75h**

### Phase D: Data Retention & Deletion API

Allow attendees to request data deletion (PDPA right to erasure). This is harder because:

- **Google Sheets** — organizer controls the sheet, not BeThere
- **On-chain data** — immutable, cannot be deleted
- **Worker KV** — can be deleted

**Pragmatic approach:**

| Data Store | Deletion Approach | PDPA Compliance |
|-----------|------------------|----------------|
| Google Sheets | Mark row as "DELETED", clear PII cells (name, email, phone, contact) | Organizer must cooperate |
| On-chain (Solana) | **Cannot delete** — note this in privacy policy as technical limitation | Disclosed as immutable |
| Worker KV (session, deposits) | Delete KV entries | Full compliance |
| Cloudflare logs | Auto-expire after 72h (Cloudflare default) | Automatic |

**Implementation:**

| Area | Change | Effort |
|------|--------|--------|
| `worker/src/handlers/` | `POST /api/privacy/delete-request` — clears PII from sheet row + KV | 2h |
| `frontend-leptos/src/pages/` | "Request Data Deletion" section on landing page (auth-aware) | 1.5h |
| Privacy policy | Document deletion scope and limitations | 0.5h |

**Total: ~4h**

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
| Legal basis for collection | §19 | 📋 Phase A (consent checkbox) |
| Privacy notice before collection | §23 | 📋 Phase C (privacy policy) |
| Purpose limitation | §23 | ✅ Already scoped to event operations |
| Data minimization | §23 | 🟡 Contact fields are optional; review if all columns needed |
| Consent for sensitive data (photos) | §20, §26 | 📋 Phase B (photo consent) |
| Right of access | §27 | 📋 Phase D (deletion API) |
| Right to correct | §28 | ✅ Organizer can edit Google Sheets |
| Right to delete | §29 | 📋 Phase D (deletion API) |
| Right to withdraw consent | §30 | 📋 Phase A (unchecking = withdraw) |
| Data breach notification | §31 | ✅ Cloudflare provides monitoring |
| Data Protection Officer | §32 | 📋 Organizational — not a code change |
| Cross-border transfer | §33 | 🟡 Google Sheets (US), Solana (decentralized) — disclose in privacy policy |
| Data retention limit | §24 | 📋 Phase D (retention policy) |

## Google Sheet Column Updates

| Column | Index | Field | Phase |
|--------|-------|-------|-------|
| AE | 30 | `consent_given` | A |
| AF | 31 | `photo_consent` | B |

## Effort Summary

| Phase | Description | Effort | Priority |
|-------|-------------|--------|----------|
| A | Consent checkbox | ~3h | **P1** — required before mainnet |
| B | Photo/media consent | ~3.5h | P1 — most Thai events photograph |
| C | Privacy policy page | ~2.75h | **P1** — PDPA requirement |
| D | Data retention & deletion | ~4h | P2 — can ship after mainnet |
| **Total** | | **~13.25h** | |

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
