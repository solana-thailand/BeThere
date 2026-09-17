# 111 — A closed tab threw the whole form away

**Status:** implemented 2026-09-14, **not deployed**
**Found:** 2026-09-14, flagged as the remaining risk in `.issues/110`
**Severity:** medium — it costs exactly the answers that are hardest to get

## What

Nothing on `/feedback` was persisted until submit. A closed tab, a flat battery,
a mistaken back-swipe, a session expiring mid-form — any of them discarded
everything typed, with no warning and no way back.

Harmless for the 123 people with one session. **29 have four or more and three
have eleven**, and those are the regulars — the people whose answers the Q4 case
rests on, doing the longest version of the task.

## Why localStorage rather than a save endpoint

The alternative was submitting each block as it is completed, which removes the
problem by removing the concept of an unsent answer. It was rejected:

- It changes what "ส่งความเห็น" means. A reader who rates one dimension and
  then reconsiders would have already published the first answer.
- The submission path writes an `attendees` row and a contact upsert per call.
  Firing it on every partial edit multiplies writes and audit noise for
  something the reader has not finished saying.
- `.issues/110` had just made the submit button mean something. Making it
  decorative the same week would be churn.

A draft is honest about its own status: *ยังไม่ได้ส่ง*. It is a safety net, not
sync — it does not follow the reader to another device, and it says so by
existing only on the browser that typed it.

## How it behaves

- **One writer.** An `Effect` over the block signals, not a save call hung off
  every radio and textarea. Per-handler saves mean the next question added to
  the page silently is not saved; an effect cannot be forgotten.
- **A server answer beats a draft.** If a block is `answered` the draft for it is
  ignored and never restored — they submitted since typing it, and showing the
  old text back would look like the submission had been undone.
- **Cleared per block as each POST lands**, not wholesale at the end. Clearing
  everything after a partial failure would discard the answers for the one block
  whose submission failed — the case where the draft is the only copy left.
- **The reader is told.** *กู้คำตอบที่คุณกรอกค้างไว้กลับมาแล้ว — ยังไม่ได้ส่ง*,
  only when something was actually recovered. A net nobody knows about does not
  reduce the anxiety it exists for, and answers appearing unannounced read as
  the page deciding things on your behalf.
- Empty blocks are not written, so the key stays small and a reader who typed
  nothing leaves no trace.

## What is stored, and the honest caveat

Satisfaction selections and free-text comments, under
`bethere.feedback.draft.v1`, on the reader's own browser. It is their own
unsent text and it is removed as soon as the server has it — but it is
unencrypted local storage on a possibly shared machine, which is worth stating
rather than discovering. The app already keeps a session token there.

## Not done

- **No expiry.** A draft for an event whose form later closes will sit there.
  Small, and cleaning it needs a rule about how long an unsent answer is worth
  keeping, which nobody has been asked.
- **No cross-device recovery.** Out of scope by design; that is sync.

## Related

- `.issues/110` — where this was recorded as the remaining risk.
- `.issues/107` — the server-side answered state this defers to.
