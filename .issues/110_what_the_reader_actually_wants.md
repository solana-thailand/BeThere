# 110 — The page asked a favour without saying what the favour buys

**Status:** implemented 2026-09-14, **not deployed**
**Found:** 2026-09-14, asked to think about what the reader wants and feels
**Severity:** medium — it is response rate, which is the only output the page has

## Who arrives, and in what state

Someone who got an email. They are doing the programme a favour and have no
stake in the result. Three questions run under everything they do on the page:

1. **How long is this?**
2. **Do I remember well enough to answer honestly?**
3. **Does my answer go anywhere?**

The page answered (1) badly, (2) well, and (3) not at all.

### (3) was the real gap

The stake existed — in DevRel's covering email:

> ตอนนี้เรากำลังสรุปเฟสแรกเพื่อเสนอ Solana Foundation ว่าจะทำอะไรต่อ
> และสิ่งเดียวที่เรายังไม่มีคือเสียงของคุณ

Anyone who clicked the link left it behind. The page then opened with mechanics
— two minutes, skip anything, anonymous — and never said why any of it mattered.
That is the same defect as the closing question sitting under eleven blocks in
`.issues/103`: the most motivating thing present, positioned where it does no
work.

The reason is now the first thing under the heading, above the mechanics.

### (1) "2 นาที" was a promise the page could not keep

Two minutes, next to eleven sessions. Anyone counting rows knows the number is
wrong, and a page that opens with a claim you can immediately disprove has spent
its credibility before the first question.

Priced per unit the reader actually commits to instead: *หนึ่งงานใช้เวลาไม่ถึงนาที*.
True, checkable, and it makes stopping after one feel like completing something.

## Submit accepted an empty form

> ถ้าจะกดปุ่มส่งความเห็นได้ ควรจะมีกรอกอะไรสักอย่าง อย่างน้อย 1 event หรือเปล่า

It did. The button ran eleven POSTs, each skipping its empty payload, and
rendered **"บันทึกความเห็นของคุณแล้ว 0 งาน"** — a thank-you for nothing, and a
state the reader could reach without ever understanding they had done nothing.

Now disabled until at least one unanswered session has been rated, with a line
saying so. A disabled button on its own is a puzzle; the hint is the half that
makes it a rule.

## The thank-you screen was a full stop

> ส่ง feedback เสร็จแล้ว ให้เห็น upcoming event ต่อด้วยดีไหม แบบเชิญชวนต่อ

Yes, and for a better reason than filling space: **someone who has just done the
programme a favour is the warmest audience the next event will ever have.**
Ending on "thank you" spends that and returns nothing.

The screen now carries the next public event — name, date, venue, linked — above
the continue and home buttons.

## "Past Events" pointed at an empty page

`GET /api/public/events/past` filters `recap_published = 1`. That is 0 on every
one of the sixteen events, and DevRel has explicitly asked us not to flip it:
the recap half is not ready and the genesis site is the public archive.

So the header linked to a page that has never had anything on it and will not
until a decision that is not ours. Removed from both the desktop and mobile nav.
The route still exists for when the recaps do.

## What was considered and not done

- **Saving answers as you go.** Nothing is persisted until submit, so a closed
  tab loses everything — a real risk across eleven blocks. It needs either
  local storage with a merge story or a per-block save endpoint, and both are
  bigger than this change. The sticky submit reduces the exposure; it does not
  remove it.
- **Asking fewer questions of people with many sessions.** Tempting, and it
  would destroy the per-event comparison DevRel reports on.
- **A completion reward.** Argued and rejected in `.issues/091`: a reward
  attached to a satisfaction score biases the score, and these numbers go to the
  Foundation.

## Related

- `.issues/103` — the ordering work this extends.
- `.issues/108` — the polish pass before it.
