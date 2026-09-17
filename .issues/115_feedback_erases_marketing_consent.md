# 115 — Answering the post-event survey withdraws marketing consent

**Status:** Fixed on `fix/115-feedback-erases-marketing-consent` — not merged, not deployed
**Found:** 2026-09-14, by DevRel, in production (`solana-thailand-devrel-helper/reports/phase-2/CONSENT-GATE.md`)
**Severity:** High — PDPA: the consent record must be accurate. The bug wrote dated withdrawals nobody made, and erased opt-ins that were made.

---

## 1. What happened

The feedback page (`frontend-leptos/src/pages/public/feedback.rs`) posts one
`register-post-event` per event answered. It asks nothing about marketing, so
`consent_marketing` stayed `None` and — being `skip_serializing_if` — never
reached the wire. The worker then:

1. read absent as no — `consent_marketing.unwrap_or(false)` in both
   `upsert_attendee` and `upsert_post_event_attendee`;
2. overwrote the stored answer **and re-dated it** —
   `consent_marketing = excluded.consent_marketing, consent_marketing_at = excluded.consent_marketing_at`;
3. and reset `developer_profiles.consent_outreach` to `"0"` in
   `write_developer_data`, plus stored a `consent_marketing = false` response row.

Two lines above (2), the same statement already protects contact fields with
`COALESCE(NULLIF(excluded.x, ''), attendees.x)`. The field with legal weight
was the one without it.

**Production, 2026-09-17:** 11 people carry dated withdrawals; every one of them
withdrew exactly as many rows as events they answered in the survey.

A second, smaller defect sat beside it: the post-event registration form shipped
the marketing box **pre-ticked** (`signal(true)`), while the main registration
form ships it unticked. A pre-ticked box is not consent (PDPA s.19).

## 2. Fix

`consent_marketing` now has three states end to end.

| client sends | meaning | fresh row | existing row |
|---|---|---|---|
| `Some(true)` | box ticked | `1`, dated | `1`, re-dated |
| `Some(false)` | box shown, unticked | `0`, dated | `0`, re-dated |
| `None` | form did not ask | `0`, **undated** (= never answered) | **unchanged** |

- `worker/src/db/attendees/writes.rs` — `consent_bind()` binds `None` as SQL
  `NULL`; both upserts use `COALESCE(?, 0)` / `CASE WHEN ? IS NULL …` on insert
  and `COALESCE(?, attendees.consent_marketing)` / keep `_at` on conflict.
  Behaviour verified against SQLite for all four transitions.
- `worker/src/handlers/register/{types,contact,post_event,signup}.rs` —
  `DeveloperData.consent_marketing` is `Option<bool>`; `consent_outreach` and the
  response row are written only when the form asked.
- `frontend-leptos` — both forms that show the box send `Some(checked)`; the
  post-event form defaults unticked; `feedback.rs` states `consent_marketing: None`.
- `worker/tests/marketing_consent_guard.rs` — pins all of the above; reverting
  `writes.rs` fails it.

`D1Type::Null` binds work in production (worker 0.8.1): `append_audit` binds it
for `metadata` and `audit_log` holds 89 `NULL` rows, the latest from
2026-09-17. The comment in `db/attendees/management.rs` saying `bind_refs`
rejects `Null` predates that and is out of date — left alone, not in scope.

## 3. What this does NOT do

**The 11 existing withdrawals are left as they are.** Restoring them would write
consent into production on behalf of people who did not give it — the same
error pointed the other way. The right repair is to ask them again. Three of
the original four had never answered the question at all.

## 4. Remaining

- [ ] Review, merge to `develop`, deploy (owner). Back up D1 first.
- [ ] After deploy, confirm with DevRel's `./scripts/recipients.py --audit`: a
      survey submission must no longer add a withdrawal.
- [ ] DevRel's watchdog `tests/test_recipients.py::TestTheBugIsStillThere` reads
      `develop` and will fail on merge — expected; update `CONSENT-GATE.md` then.
