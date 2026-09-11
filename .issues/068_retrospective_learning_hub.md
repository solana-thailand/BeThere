# 068 — Retrospective event learning hub

**Status:** Planned  
**Priority:** P1 growth after Core Readiness P0 gates  
**Created:** 2026-09-11

## Product decision

Allow people who missed a past event to register retrospectively and access its
recording and published resources. Treat this as a learning and community entry
point, while preserving the historical truth that retrospective learners did
not attend live.

The satisfaction email can include two independent calls to action:

1. Attendees: give feedback about the event they joined.
2. Non-attendees or recipients who missed it: view the public recap and register
   for retrospective access.

Never mark retrospective registration as check-in, attendance, deposit
completion, quiz completion, or proof-of-attendance NFT eligibility.

Campaigns are not an MVP dependency. Production has no configured campaigns;
campaign creation and administration need a separate usability phase before
series-based learning navigation becomes a default organizer workflow.

## Priority rationale

This is P1 because it can extend the value of every event and create a clear
route from email back into BeThere. The production mint, quiz configuration,
and live lifecycle gates in Issue 066 remain P0 because failures there can
block existing attendees or create ambiguous asset and money state.

## Free-first rollout

### Phase 0 — email and existing pages

- [ ] Add one feedback URL and one canonical past-event URL to the campaign
      template; keep unsubscribe and consent rules intact.
- [ ] Use campaign attribution parameters without putting email, attendee IDs,
      or claim tokens in URLs.
- [x] Reuse the event's existing HTTPS recording URL on published public recap
      pages through the shared ticket video component; recap text remains the
      initial free resource surface.
- [ ] Ensure the past-event page has a useful public recap when registration is
      unavailable or content is private.

### Phase 1 — retrospective registration MVP

- [ ] Add an explicit `retrospective` enrollment kind rather than reusing
      `online`, walk-in, or checked-in state.
- [ ] Let organizers enable retrospective enrollment per event and set content
      visibility to `public`, `registered`, or `attended`.
- [ ] Extend the existing recap surface from recording + recap text into an
      ordered module list for slides, links, source code, and downloads; do not
      create a parallel learning route.
- [x] Start with URL-backed video and resources. Existing event `video_url` and
      typed `community_links` records power the published recap without a new
      table or admin CRUD flow. Do not introduce paid video hosting or
      transcoding until usage and cost justify it.
- [ ] Give signed-in learners a resumable “My learning” entry point.
- [ ] Record consent, enrollment timestamp, content version, and campaign
      attribution separately from live registration analytics.

### Phase 2 — learning experience

- [ ] Add chapters/modules, completion progress, transcripts/captions, resource
      metadata, and accessible keyboard navigation.
- [ ] Reuse the existing quiz engine as optional learning assessment, with a
      separate policy from the proof-of-attendance claim gate.
- [ ] Let organizers publish updates without silently changing completion or
      quiz results already earned against an older content version.
- [ ] Measure registration conversion, lesson starts, meaningful video
      progress, resource opens, completion, and next-event registration.

## Domain boundaries

Use separate concepts even if they share an account and UI:

| Concept | Meaning | Grants attendance NFT |
|---|---|---|
| Live registration | Intends to join the scheduled event | No |
| Check-in | Verified participation at the event | Eligible under event rules |
| Retrospective enrollment | Accesses learning content after the event | No |
| Learning completion | Completed published modules or assessment | No; a future learning credential must be a distinct asset |

Deposits must never be required for retrospective enrollment. Past-event
enrollment must not consume historical capacity or change attendance and
no-show metrics.

## Security, privacy, and sustainability

- Authorize registered or attended content server-side; hiding a URL in the UI
  is insufficient.
- Store only resource metadata and approved URLs in D1. Keep secrets, private
  source URLs, and provider tokens out of public API responses.
- Sanitize organizer-authored titles/descriptions and restrict resource URL
  schemes to HTTPS.
- Use existing Cloudflare and external video/resource hosting first. Add R2
  storage only with explicit size limits, lifecycle rules, and cost monitoring.
- Avoid autoplay and heavy embeds on initial load. Use a poster/link or lazy
  embed to protect Core Web Vitals and mobile data usage.
- Preserve email consent, unsubscribe, suppression, and campaign audit trails.

## Acceptance journeys

1. A prior attendee opens the satisfaction email, submits feedback, and returns
   to the correct event recap.
2. A person who missed the event opens the same campaign, enrolls
   retrospectively, and resumes the first module on another device.
3. Public content is readable without registration; registered content requires
   ownership; attended content requires verified check-in.
4. A retrospective learner completes a quiz but remains excluded from
   proof-of-attendance NFT eligibility and live attendance metrics.
5. An organizer unpublishes a broken/private resource and it stops being served
   without deleting historical enrollment or progress.

## Evidence required

- Domain and API tests proving retrospective enrollment cannot mutate live
  registration, capacity, deposit, check-in, or claim eligibility.
- Browser E2E for public, registered, attended, and unauthorized content.
- Email-link test covering attribution, consent, unsubscribe, and absence of PII
  in URLs.
- Mobile/accessibility review and cold-load performance measurements with a
  representative video and resource set.

## Related work

- Issue 058: post-event summary and no-show/online reporting.
- Issue 060: attendee event-series navigation.
- Issue 066: core services production readiness.
- Issue 067: attendee UX/UI refresh.
