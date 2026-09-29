# 177: Design: public organizer profiles and attribution on events

**Status:** open (design only; filed 2026-09-29 by session `event-checkin-ba` per the GOAT-hardening handoff, which pairs this with the multi-org work).

## Context

Events carry `organizer_emails` and `organization_id`. Attendees see neither
who runs an event nor its other events. The handoff defers public profiles
to the multi-org work, so the design has to settle ownership first.

## Questions

1. **What the profile is keyed on:** an organization (`org_store`), or a
   person? A public page must never expose `organizer_emails`, and the
   "Keep `://` out of API error text" rule applies too.
2. **URL and slug:** `/o/{slug}`; uniqueness and renames.
3. **Content:**
   - name, logo, short bio, links;
   - upcoming and past public events (reuse `list_public_events_raw`,
     filtered by org);
   - the community links that events already carry.
4. **Attribution on `/e/{slug}`:** a "Hosted by …" meta row (it fits the
   P2-b rows).
5. **Editing:** who may edit (org admin role), and the audit entries.
6. **Privacy:** an opt-in to be listed; archived orgs.
