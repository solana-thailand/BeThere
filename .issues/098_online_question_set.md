# 098 — The question DevRel most wants answered is in neither form

**Status:** implemented 2026-09-14, **not deployed**
**Found:** 2026-09-14, reading the online Google Form's own definition
**Severity:** medium — it decides whether 337 online registrations produce a
number or an explanation

## What the two forms actually ask

Both were read out of their live `FB_PUBLIC_LOAD_DATA_`, not from a description.

| | onsite form | online form |
|---|---|---|
| satisfaction | **four named dimensions** — เนื้อหา / สถานที่ / อาหารเครื่องดื่ม / ประชาสัมพันธ์, each ไม่พึงพอใจ–พึงพอใจ–พึงพอใจมาก | **one unlabelled item**, scale 1–3 |
| per event | "ท่านได้เข้าร่วมครั้งที่ N หรือไม่" gate, then satisfaction, then ข้อเสนอแนะ | identical |
| closing | เนื้อหาที่อยากให้มีครั้งต่อไป · Latent Space จัดต่อไหม | identical |

The difference is one line long and the reason is obvious once stated: someone
who watched a livestream has no opinion on the room or the coffee. Asking anyway
produces a number that looks like data.

## The gap

**"ทำไมลงทะเบียนแล้วไม่ได้ดู" is not a field in either form.** It is a sentence
in the covering email (`survey-emails.md`):

> คำถามที่สำคัญที่สุดสำหรับเรา: อะไรที่ทำให้คุณ "ไม่ได้ดู" ทั้งที่ลงทะเบียนไว้
> — เวลา หัวข้อ ภาษา หรือไม่รู้ว่าไลฟ์แล้ว คำตอบนั้นมีค่ากับเรามาก

So the question DevRel calls the most useful thing the survey can find was only
answerable as free text in a comment box, by people who read to the bottom of an
email — and that email was never sent.

The gap it measures is real and they quantified it themselves: **337 online
registrations against 7,553 archive views across 15 videos.** Their words: *"the
gap between registering and watching is the most useful thing this survey can
find, and no dashboard will ever tell us."*

## What changed

`dimensions_for(participation_type)` — content and promotion for everyone;
venue and catering onsite only. Every block still carries all four signals; the
function decides which are asked **and** which are submitted, so an online
answer can never carry a venue rating it was never shown.

`post.online.watched` — asked of online attendees only, and **first**, because
for someone who did not watch it is the only question they can answer honestly:

```
ได้ดูสด · ดูย้อนหลัง · ไม่ได้ดู — ติดเวลา ·
ไม่ได้ดู — หัวข้อไม่ตรงที่สนใจ · ไม่ได้ดู — ภาษา · ไม่ได้ดู — ไม่รู้ว่าไลฟ์แล้ว
```

The four "ไม่ได้ดู" reasons are DevRel's own list, turned from prose into
options so the answers are countable instead of needing to be read.

`InboxNotification` gained `participation_type`. The view always selected it; it
was only ever used to decide whether a deposit was outstanding.

## Deliberately not done

- **The 1–3 scale is not copied.** The online form's unlabelled numeric scale
  produces answers that cannot be compared with the onsite form's named ones.
  Using the same three labels for both makes content and promotion comparable
  across participation types, which the Google Forms never were.
- **No "did you attend?" gate**, on either type. BeThere knows who checked in
  and who registered; re-asking is the five required questions the Google Form
  has to ask because it has no idea who is filling it in.

## Verified

Worker lib suite 288 passing, both clippy gates clean at `-D warnings`.

**Not seen rendered** — the page is auth-gated, and the online branch needs a
session whose attendee row is `participation_type = 'online'`. The organizer's
rows are in-person, so even a screenshot from him will not exercise it.

## Related

- `.issues/097` — the gate that lets these people in at all.
- `.issues/091`, `.issues/094` — the page and its recognition block.
