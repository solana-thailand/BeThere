# Handover #073: Deposit Clarity + 413 Handling + Admin UI Simplification

## What Happened

Continued from session thread "Commit deposit fix online revert error 413". Three batches of work:

### Batch 1: Deposit Clarity + Deadline Reclaim + 413 Error Handling
**Commit**: `b593cd5` — 7 files, +245/-14

| File | Change |
|------|--------|
| `frontend-leptos/src/pages/ticket.rs` | Deposit-required banner, deadline-expired reclaim notice, auto-switched-to-online notice (in-person & online views) |
| `frontend-leptos/src/pages/deposit.rs` | Step-by-step payment instructions, file size hint (3MB), restricted file input to JPEG/PNG/WebP, 413 error handling |
| `frontend-leptos/js/file_upload.js` | Client-side file limit 5MB→3MB with clearer error |
| `worker/src/handlers/deposit/thb.rs` | Server-side data URL limit 7MB→5MB, improved error message |
| `worker/src/handlers/attendee.rs` | Added deposit_enabled/deposit_deadline_hours/deposit_amount_thb/deadline_expired/in_person_available/event_slug to ticket API |
| `frontend-leptos/src/api/types.rs` | 7 new AttendeeData fields |
| `worker/src/handlers/register.rs` | Minor cleanup |

### Batch 2: Admin Attendee List Redesign
**Commit**: `ef2aa89` — 2 files, +218/-156

| File | Change |
|------|--------|
| `frontend-leptos/src/pages/admin.rs` | Restructured attendee items from horizontal `[checkbox][info][status+buttons]` to two-row card: row 1 = checkbox + name + badges, row 2 = email/ticket/meta + action buttons |
| `frontend-leptos/style.css` | New card layout classes, removed old `.attendee-info`/`.attendee-status`/`.admin-ticket-row`/`.admin-time-ago`, mobile stacking breakpoint at 480px |

### Batch 3: App-Wide "Less is More" Audit + Issue Planning
**No code changes** — full audit of all frontend pages for UI complexity.

Created `.issues/033_less_is_more_ui_simplification.md` with prioritized 3-phase plan:
- **Phase 1** (attendee-facing): Deposit 2-step wizard, Ticket 1 status slot, Claim Success simplification
- **Phase 2** (staff/admin): Events form extraction, Escrow shared component, Scanner settings gear
- **Phase 3** (polish): Cancel page cleanup, wallet connect unification

## Where Is the Plan/Code/Test

- Issue: `.issues/033_less_is_more_ui_simplification.md`
- Code: `frontend-leptos/src/pages/admin.rs`, `frontend-leptos/style.css`
- UX Roadmap: `docs/ux_roadmap.md` (existing, Issue 033 cross-referenced)

## Reflection — Struggling / Solved

- **Solved**: Admin attendee names invisible — root cause was horizontal flex with 5+ action buttons squeezing the name to zero width. Fix: two-row card layout where name is the hero element.
- **Solved**: 413 error on deposit slip upload — base64 overhead makes 5MB image → 6.7MB JSON body. Reduced client limit to 3MB, server to 5MB.
- **Insight**: "Less is more" philosophy (from "What Elite Software Engineers Do Differently" video) applied across the app. Audit revealed deposit page is the worst offender (12 buttons at once for non-technical users).

## Remain Work

- [ ] Phase 1A: Deposit page → 2-step wizard (highest impact on conversion)
- [ ] Phase 1B: Ticket Page → unify 5 deposit banners → 1 status slot
- [ ] Phase 1C: Claim Success → simplify to NFT + "View NFT" + done
- [ ] Phase 2A: Events Page → extract form component (2,572 → ≤1024 lines)
- [ ] Phase 2B: Admin Escrow → shared step component, remove inline styles
- [ ] Phase 2C: Scanner → settings gear icon
- [ ] Frontend build needed before deploy

## Issues Ref

- `.issues/033_less_is_more_ui_simplification.md`

## How to Dev/Test

1. `cd frontend-leptos && bash build.sh` — builds WASM + trunk
2. `cd worker && npx wrangler dev` — local dev
3. Test admin: login as admin → Attendance → In-Person → verify attendee names visible, cards render correctly
4. Test deposit: upload >3MB image → verify friendly error message
5. Test ticket: visit `/ticket/{id}?event_id=xxx` → verify deposit notices show correctly
