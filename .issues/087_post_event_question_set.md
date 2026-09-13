# 087 — Post-event satisfaction questions in the registration form

**Status:** implemented 2026-09-13, not yet deployed — see `.issues/090`
**Requested by:** Solana Thailand DevRel, `reports/phase-2/BETHERE-ASKS.md` item 2
**Severity:** feature — their stated top priority

## Why

DevRel had two Google Forms ready for weeks, with lists cut for 55 onsite and
96 online people. **Zero responses.** Their read: an email three days after an
event does not get answered. Their conclusion is the same insight that made the
deposit hold work — *proximity beats intent*: ask the person who is already on
the page.

`.issues/082` made the Worker accept it. This is the half that asks.

## What

Four optional questions on `/events/{slug}/post-event-register`, using the
`field_key` names DevRel specified:

| key | control |
|---|---|
| `post.satisfaction.overall` | 1–5 |
| `post.nps` | 0–10 |
| `post.would_return` | yes / maybe / no |
| `post.comment` | free text |

They ride the `profile_fields` map that `PostEventRegisterRequest` has accepted
since Plan 008 Phase 3 and that nothing had ever sent — `PostEventRegisterBody`
simply did not carry the field.

## The `post.` prefix is load-bearing

It is not a naming convention. `db::developers::is_event_scoped_field` routes a
`post.` key to `registration_responses` scoped to the event and **skips the
`developer_profiles` upsert entirely** (`.issues/082`). Dropping the prefix
would start writing survey answers onto people's profiles and mark them
`is_profile_field = 1`, which is exactly the column DevRel read from outside to
work out where the existing 2,103 rows came from.

A test asserts every key keeps the prefix.

## Unanswered questions are omitted, not sent empty

An absent key reads as "not answered"; an empty string reads as "answered with
nothing". The Worker drops empty values anyway, so sending them would only add
noise to a table DevRel query directly.

## Verified

The full chain was exercised end to end on deployed staging before the UI
existed, by posting the same map the form now builds:

```
POST /api/public/event/flow-084-lifecycle/register-post-event  → 200
```

```
field_key                       value   is_profile_field
post.nps                        9       0
post.satisfaction.overall       5       0
post.would_return               yes     0
participation_type              retrospective
```

`is_profile_field = 0` and `participation_type = retrospective` — the two
properties DevRel asked us to guarantee.

Frontend: clippy clean on both targets, 4 new unit tests, suite green.

## Not done

- **Question wording is ours, not DevRel's.** The keys are theirs, verbatim from
  their ask; the labels are placeholders that read sensibly. They said they
  would write the questions — swap the label strings when they do.
- The set is compiled in rather than per-event configurable. `form_config`
  exists for registration and could carry a post-event phase later; that is a
  bigger change and nobody needs it yet.
- Nothing surfaces the answers to an organizer. They are queryable in
  `registration_responses`; a dashboard is separate work.

## Related

- `.issues/082` — the `post.` namespace and the bounded field map.
- `.issues/083` — the admin control that opens post-event registration at all.
- `.issues/080` — why the survey still cannot be *sent* by notification.
