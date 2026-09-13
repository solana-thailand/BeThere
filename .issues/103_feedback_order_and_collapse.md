# 103 — 60% see one question set; the 14% who see four or more are the regulars

**Status:** implemented 2026-09-14, **not deployed**
**Found:** 2026-09-14, asked directly: *"ตอนนี้ที่ถูกที่ควรที่ดีต่อ ux ต้องเป็นยังไงกันแน่"*
**Severity:** medium — it decides the response rate of the cohort that matters most

## The shape of the problem, measured

`feedback_eligible_events` on the production export:

| blocks | people |
|---|---|
| **1** | **123** |
| 2 | 32 |
| 3 | 22 |
| 4+ | **29** |
| 6+ | 12 |
| 11+ | 3 |

**The median experience was already fine.** 123 of 206 see one question set and
are done in fifteen seconds. The problem is entirely the tail — and the tail is
the regulars, the cohort DevRel says the Q4 track is built on. Optimising the
page for the median would have been optimising for the people who were never
going to struggle.

## Four changes

### 1. The programme questions moved above the tail — the biggest one

*"เนื้อหาที่ท่านสนใจหรืออยากให้มีในการจัดงานครั้งต่อไป"* sat **last, under up to
eleven blocks.** DevRel calls it the most valuable answer the survey can get.
Anyone who stopped after two sessions never saw it.

They already knew: *"Response quality on open text collapses when it sits at the
end of a long grid unannounced"* — and worked around it by naming the question
in the covering email. That is a workaround for a page they could not change.
We can change the page.

Order is now: **newest session → programme questions → everything older.** Easy
start, the valuable question inside twenty seconds, the long tail opt-in.

### 2. Only the newest session opens

The rest collapse to a one-line row with poster, name and date. An eleven-block
page becomes one question set and ten rows.

The list already arrives `event_start_ms DESC`, so the one that opens is the
session they remember best. If they answer only that one, it is the most
reliable answer they could have given — recency is the whole argument.

### 3. The header is a button, not a link

It linked to `/e/{slug}`. **Nothing on this form is persisted until submit**, so
one tap on the header discarded a half-filled block. The link was one tap from
losing someone's work, on the element most likely to be tapped.

Recognition is what the poster, name, date and venue are for. The event page was
not worth that risk, so tapping the header now expands or collapses instead.

### 4. Progress, and permission to stop

`ตอบแล้ว 2 จาก 11 งาน — ส่งได้เลยไม่ต้องครบ`, shown only to the 83 people with
more than one session. For the other 123 a counter reading "1 จาก 1" is noise.

A collapsed row that has been answered carries a tick; an untouched one carries
a `+`. Without that, the only way to tell answered from skipped is to open it.

## Deliberately not done

- **Not trimming the list.** DevRel needs the per-event signal; showing only the
  newest session would make the four dimensions incomparable across sessions.
- **No "did you attend?" gate.** BeThere knows. The Google Form has to ask
  because it does not — five required questions that exist only to establish
  something we already store.
- **Nothing required.** The onsite form makes five questions mandatory, which is
  one of the reasons a 34-item form gets abandoned.

## Not verified

The page is auth-gated, so this has been compiled, linted and tested but **not
seen**. The collapse behaviour in particular — what a ten-row tail actually
looks like — needs someone with a session to open it.

## Related

- `.issues/102` — the regression that made the list one block long.
- `.issues/098` — the per-participation-type question sets inside each block.
