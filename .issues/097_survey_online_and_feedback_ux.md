# 097 — Online attendees could never be asked, and the form they land on staircased

**Status:** fixed 2026-09-14; migration 0037 + backfill **not applied to prod**
**Found:** 2026-09-14, from the organizer's screenshot of the live page
**Severity:** high for the gate (335 people unreachable), medium for the layout

## Part 1 — "จะเปิดสิทธิ์ให้คนลงทะเบียนออนไลน์ตอบแบบสอบถามได้ยังไง"

The survey gate was `checked_in_at IS NOT NULL`. Nobody watching a livestream is
scanned at a door. On the twelve open events:

| participation_type | registered | checked in |
|---|---|---|
| `online` | **337** | **2** |
| `in_person` | 120 | 88 |

335 people excluded by a condition they had no way to satisfy. The two online
check-ins are almost certainly staff testing.

**The rule to use was already written down.** `BETHERE-ASKS.md`, from DevRel:

> Counting rule, stated so it is not re-derived differently: **onsite by
> check-in, online by registration.**

That is how their Phase 1 attendance figures are counted and already submitted
to the Foundation. Any other rule here produces survey responses that cannot be
compared to the numbers they published.

Migration **0037** encodes exactly that, in the trigger and the view:

```sql
a.participation_type='online'
  OR (a.checked_in_at IS NOT NULL AND a.checked_in_at<>'')
```

**What stays out, deliberately:**

- **`in_person` who never checked in** — 32 people who reserved a seat and did
  not come. "How was the event?" is the wrong question for them, and their
  answer would be about an event they did not see.
- **`retrospective`** — someone who found the form afterwards. They did not
  attend; letting them rate the event puts invented numbers in the same column
  as real ones.

Reach: **90 rows / 55 people → 425 / 206.**

Rehearsed against the prod export through 0035 → 0036 → 0037 plus all three
backfills: `survey | pending | 425`, inbox 425 / 206. New test walks five cases
(online no-show, online watched, onsite came, onsite no-show, retrospective) and
asserts the last two stay out. A/B: reverting the predicate to the check-in-only
form fails it on `participation='online', checked_in=False`; restored to green.

**Worth knowing before this ships.** DevRel's most valuable online question is
*"if you registered and did not watch, what got in the way"* — 337 registered
against 7,553 archive views. The four satisfaction dimensions do not ask it. The
form will now reach the right people with slightly the wrong questions; adding
an online-specific block is the obvious follow-up and is not in this change.

## Part 2 — UX verdict on the live page

Four defects, all visible in the screenshot, none of which I could see before
because the page is auth-gated and I had only compiled it.

### 1. The cards walked down the page in a staircase

Each block was narrower than the one above and shifted right. Cause:

```css
.layout-col-center { display: flex; flex-direction: column; align-items: center; }
```

`align-items: center` sizes every child to **its own content** and centres it
independently, so a block with a longer event name came out wider. The page now
uses its own `.fb-page`; only the page is centred, and sections are full width.

### 2. The closing question's options ran together on one line

`อยากให้จัดต่อ และจะเข้าร่วม○อยากให้จัดต่อ แต่จะดูย้อนหลังเอา○เฉย ๆ○…` — bare
`<label><input type=radio></label>` with no styling, inline, no gaps, and nothing
near a 44px tap target.

They are full Thai sentences, so they now stack full width as `.fb-option` rows
rather than borrowing the three-across segmented control the satisfaction rows
use — that would wrap each sentence into an unreadable column.

### 3. The poster was too small to do its job

56px. These posters are dark, dense and mostly black; at that size the title
band is unreadable, which defeats the entire point of putting it there
(`.issues/094`). Now 72px.

### 4. "+ เพิ่มข้อเสนอแนะ" read as disabled

`--text-secondary` on a dark card. It is the one optional action on each block;
it is now accent-coloured and medium weight.

### Not changed, and why

- **Four blocks is correct**, now five — the screenshot predates the 0036
  backfill. Production is `survey | pending | 90`, inbox 90 rows / 55 people,
  verified after the owner applied it.
- **`Intro to Vibing on Solana` shows the BeThere badge**, not a poster, because
  that event has no `poster_url`. Six events are in that state — an upload, not
  a code change.
- **No progress indicator** ("งานที่ 2 จาก 5"). Worth adding once a person can
  have five blocks; left out rather than guessed at.

## Related

- `.issues/095` — the `checked_in` half of the same gate.
- `.issues/094` — why the poster is there at all.
- `.issues/091` — the campaign.
