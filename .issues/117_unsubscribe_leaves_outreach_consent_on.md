# 117 — Marketing unsubscribe leaves `developer_profiles.consent_outreach` on

**Status:** Fixed on `fix/116-unsubscribe-email-case` (2026-09-17); not deployed. One prod row needs an owner decision (below).
**Found:** 2026-09-17, while fixing `.issues/116`
**Severity:** Medium (PDPA s.19 withdrawal incomplete)

## What

The single marketing checkbox on registration is written to two stores
(`handlers/register/contact.rs` step 1b):

- `attendees.consent_marketing` (per event row)
- `developer_profiles.consent_outreach` (per person)

The organizers' contacts view and CSV export (`db/contacts.rs`,
`handlers/contacts.rs`) read `consent_outreach`. But
`POST /api/privacy/unsubscribe-marketing` ("Unsubscribe from Marketing" on
`/data-privacy`) only cleared the attendee rows. A person who withdrew stayed
listed as `consent_outreach = 1` in the export, and the endpoint still
reported `"completed"`. This is the same duplicated-state-transition pattern
seen before: the guard landed on one writer and the sibling store kept the
old state.

## Fix

- `db::developers::withdraw_outreach_consent`:
  `UPDATE developer_profiles SET consent_outreach = 0, updated_at = datetime('now')
  WHERE consent_outreach = 1 AND LOWER(email) = LOWER(?1)`.
  The `consent_outreach = 1` term lets the partial index
  `idx_dev_profiles_consent` serve the query (verified with `EXPLAIN QUERY PLAN`)
  instead of scanning the table for `LOWER(email)`. It is also a no-op for rows
  already withdrawn, so their `updated_at` is not bumped.
- `unsubscribe_marketing` calls it after the attendee write and **fails closed**:
  either write erroring returns 500, never a partial "completed". Audit text,
  log and response gain `profiles_updated`. The frontend response struct ignores
  unknown fields.
- Guard: `worker/tests/marketing_consent_guard.rs`
  `withdrawal_clears_the_developer_profile_too`. Mutation-checked three ways:
  removing the call, swallowing its error (`.unwrap_or(0)`), and a
  case-sensitive profile match each fail it.

## Prod data — owner decision, not repaired

A read-only count on 2026-09-17 found **1** profile with `consent_outreach = 1`
whose attendee rows are *all* `consent_marketing = 0`. That is consistent with
a past unsubscribe that missed the profile. It is equally consistent with a
deliberate opt-in on the profile page (`dev_profile.rs` has its own toggle),
which is a valid, separate answer. The stores alone cannot tell the two apart.
Check the `MarketingUnsubscribed` audit entries for that address before
changing anything. Do not bulk-reset (compare handover 137 §4.8).

## Not in scope

- Whether the profile toggle and the marketing checkbox should stay one
  consent or become two named purposes (a product/PDPA wording decision).
