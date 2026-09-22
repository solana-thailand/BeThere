# 132 — The ticket page has no day-of announcement

**Status:** built, uncommitted (branch `develop`)
**Opened:** 2026-09-22
**Migration:** `worker/migrations/0049_ticket_announcements.sql` — NOT APPLIED ANYWHERE

## 1. The problem

On the morning of an event the organizer learns things the attendee needs and
the system has nowhere to put them: which door is unlocked, that the basement
car park is full, that the stream went up late, where the slides are. Today the
only levers are `community_links` (a list of labelled URLs, wrong shape for
prose) and editing the event `description` (which is the marketing copy and is
also shown to people who have not registered).

The result is that this information goes out on a channel the product does not
own — a Telegram group most attendees are not in — and the ticket page, which
is the one page every attendee has open on the day, says nothing about it.

## 2. What was built

Free-text announcement shown on the attendee's own ticket page, in **two
variants** chosen by how that person is taking part.

Two columns and not one, because the audiences need opposite things: an
in-person attendee needs travel, parking and door details; an online attendee
needs the livestream link and its timing. Sending both to everyone is how a
ticket page stops being read.

### Data
`events.ticket_note_in_person` and `events.ticket_note_online`, both
`TEXT NOT NULL DEFAULT ''`. Empty string = no card is rendered, which is the
state every existing event starts in. Nothing about today's ticket page changes
until an organizer types something.

### The server picks the variant
`worker/src/handlers/attendee/read.rs:380` emits a single `ticket_note` field
selected by `attendee.is_in_person()`. The other audience's text never goes
over the wire — the client is not asked to choose, and cannot leak the variant
it was not meant to see.

### Rendering is text, never HTML
`frontend-leptos/src/pages/ticket/announcement.rs` splits the note into text
and URL runs and builds real DOM nodes. `inner_html` is never used.

The organizer is trusted; the *storage* is not. This string sits in D1, is
copied by duplicate-event, and could be reached by any future import path.
`inner_html` on a field nobody would think to audit again is how stored XSS
gets in.

Only `http://` and `https://` are linkified — a bare `example.com` stays text,
because guessing a scheme would send an attendee somewhere the organizer never
wrote, and no `javascript:` URL can ever become an anchor. Line breaks are
preserved by CSS `white-space: pre-wrap`, not by generating `<br>`.

7 unit tests cover: `<script>` staying literal, Thai text either side of a URL
not panicking on a byte boundary, trailing sentence punctuation staying out of
the `href`, and a bare hostname staying text.

### The note is bounded, and refused rather than truncated
`domain/src/models/event/ticket_note.rs` — `normalize_ticket_note`, applied on
both the create and update paths, mirroring the existing `normalize_map_url`
convention.

The bound is a **performance** control, not a cosmetic one. Both notes ride in
the event JSON cached in KV, and event reads are KV-first, so that JSON is
fetched on *every* ticket page load. Without a cap, one organizer pasting a
document into the textarea would slow the page for every attendee of that
event. `MAX_TICKET_NOTE_CHARS = 4000` is roughly two pages of prose.

Over-length is **refused with an organizer-facing message**, never silently
truncated — losing the back half of someone's parking instructions without
telling them is worse than making them shorten it.

The limit counts *characters*, not bytes: a Thai announcement is ~3 bytes per
character and must not be refused at a third of the stated length. There is a
test for exactly this.

`\r\n` and bare `\r` are folded to `\n`, because the card renders with
`white-space: pre-wrap` and a stray `\r` from a browser textarea would show as
a blank line the organizer never typed.

The textareas carry `maxlength=MAX_TICKET_NOTE_CHARS` — **the same constant**,
imported from the domain crate rather than repeated as a literal, so the client
hint cannot drift from the server rule. Note that HTML `maxlength` counts
UTF-16 code units, so for an emoji-heavy note the browser stops slightly before
the server would; the client is stricter than the server, never looser, which
is the safe direction.

### Admin
Collapsible "Ticket Announcements" section in `event_form.rs`, two textareas,
above Community & Resources.

### Duplicate carries the notes forward
Matching existing `community_links` / `video_url` behaviour. Right for a
recurring series, where parking information does not change between editions.

## 3. Deliberate decisions worth re-reading later

### 3.1 Non-in-person formats fall to the online note
The client computes `is_online = !is_in_person` (`view_data.rs:136`) and the
server splits on `attendee.is_in_person()`. The two agree exactly, which is the
property that matters. But it means **Retrospective and any future
participation type get the *online* note**, not a third variant and not
silence.

This is intentional — someone watching a recap is closer to an online attendee
than to someone standing at the door — but it is a default, not a decision
anyone made per-format. If a third format ever needs its own text, this is the
line to revisit.

### 3.2 The notes are not public
`PUBLIC_EVENT_COLUMNS` deliberately excludes both columns. The announcement is
for people holding a ticket; door codes and parking instructions should not be
readable from the public event endpoint by anyone who has the slug.

### 3.3 Stale KV entries are safe
Event reads are KV-first (`event_store::get_event`). Both new fields carry
`#[serde(default)]`, so event JSON written before 0049 deserialises as empty
and renders no card, rather than failing the read. The same attribute on the
frontend's `ticket_note` means a new frontend against a not-yet-deployed worker
degrades to no card instead of a parse error.

## 4. State of verification

Measured 2026-09-22, with the feature and the length cap in.

| check | result |
|---|---|
| `cargo fmt --all --check` (workspace) | pass |
| `cargo fmt --all --check` (frontend) | pass, after fixing 2 diffs in new code |
| workspace clippy `--all-targets --all-features -D warnings` | **exit 0** |
| frontend clippy `--all-targets --target wasm32-unknown-unknown -D warnings` | **exit 0** |
| worker `cargo test` | **554 passed, 0 failed** (36 binaries, 1 ignored) |
| domain `cargo test` (incl. 6 new `ticket_note` tests) | pass |
| frontend `cargo test` | **212 passed, 0 failed** (incl. `css_class_audit` 5/5) |
| Python SQL suite (applies all migrations, incl. 0049) | **36 passed** |
| `upsert_event` SQL alignment | 61 columns = 61 placeholders; 40 binds = 40 `?` |
| worker bundle | **1,574,121 bytes gzip = 50.03%** of the 3 MiB ceiling, exit 0 |
| opened in a browser | **verified, see §7** |

The SQL alignment was checked by counting, not by eye. `upsert_event` builds
its statement as a string, so the column list, the `VALUES` placeholders and
the bind array have to agree; if they drift, the announcement is written into
some other column and nothing fails loudly.

## 5. Remaining before this can ship

Everything in §4 and §7 is done. What is left is deployment, which is
owner-gated:

1. Apply 0049 to staging, then production. It is now an **additional pending
   migration** on the production deploy described in `.plans/027` §F.6 (0048 is
   deferred by design; 0049 must be applied).
2. Nothing else. The code is committed on `develop` (`ec59bf1`, `fbf73cb`,
   `96264d9`) and unpushed.

## 6. A trap this work uncovered

`frontend-leptos/tests/css_class_audit.rs` concatenates every `.rs` file under
`src/` and scans character-by-character for `"` — with **no concept of a char
literal**. A `'"'` in a `&[char]` array in this feature opened a string the
scanner never closed, and it silently reported every CSS class in every file
sorted *after* that one as dead.

The file that breaks the scan is never among the ones it names, which is what
makes it expensive. Fixed by using `const TRAILING_PUNCTUATION: &str` instead
of a `&[char]`, and the trap is now documented in that test's module comment.

Worth knowing that the same defect will fire again on any char literal holding
a quote anywhere under `src/`.

## 7. Verified in a browser, not just compiled

Per [[verify-frontend-by-opening-it]] — `cargo check`, `clippy -D warnings` and
an HTTP 200 all pass on a Leptos page that renders nothing, so none of them
count as verification here.

Harness: `wrangler dev --local --port 8788 --persist-to /tmp/bethere-dev-state`,
clean store, all migrations applied (**0049 applied cleanly for the first time
anywhere**; 0048 correctly absent). Seeded event `probe-evt` with two
attendees, `probe-inperson` and `probe-online`, straight into local D1 —
`get_attendee_by_id` is D1-first, so the Sheets fallback never runs and this
made **zero Google calls** despite `.dev.vars` holding live credentials.

Read back from the rendered DOM:

- `.ticket-announcement-card` present, title "From the organizer".
- **The variant split works end to end.** The in-person attendee receives only
  the in-person note; the online attendee receives only the online note, and
  the in-person text appears nowhere in its DOM (checked explicitly).
- **The XSS contract holds in a real browser.** The seeded
  `<script>alert(1)</script>` came back as `innerHTML`
  `&lt;script&gt;alert(1)&lt;/script&gt;`, with **0 `<script>` elements inside
  the card** and no alert fired.
- Thai text renders correctly, computed `white-space` is `pre-wrap` so the
  organizer's line breaks survive, the URL is linkified with `target="_blank"
  rel="noopener noreferrer"`, and the trailing sentence `.` is outside the
  `href`.

Screenshot taken and eyeballed: the card sits above the attendee details,
sharing the Access & Logistics shell as intended.

**This browser pass is also what found `.issues/133`** — the ticket page dies
outright for any event with no venue map link. Unrelated to this feature, and
it would have kept hiding behind a green build.
