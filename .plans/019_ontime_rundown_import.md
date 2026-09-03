# Plan 019 — Ontime Rundown → Public Agenda (Import)

> **Status:** Spec. Phase 1 is implementable today against shipped code; Phase 2 is
> gated on one product decision (§6.1).
> **Motivation:** organizers already author a per-session rundown in Ontime for
> every event. BeThere's public event page shows none of it, so attendees cannot
> see the schedule and the data gets re-typed by hand.
> **Related:** `.plans/018` (Colosseum readiness — a published agenda is the kind
> of thing "a product, not a demo" means), and `ontime-rs`
> (<https://github.com/ozoneRatchapon/ontime-rs>).

---

## 0. Orientation

**What this is.** A spec for importing an Ontime rundown into BeThere and
publishing it as a session list on the public event page.

**What it is not.** An integration with `ontime-rs` the *program*. See §2 — the
boundary is deliberately data-only, and that constraint is legal, not technical.

**Already shipped, and this plan builds on it:** `66737b0` made
`EventConfig.description` organizer-editable. The whole path (D1 column, worker
create/update, public API, "About this Event" card with `white-space: pre-line`)
was already live; only the form field was missing. Phase 1 needs no new schema.

---

## 1. The source format — measured, not assumed

Two Ontime CSV shapes exist and they are **not** the same. Getting this wrong is
the most likely way to build the wrong parser.

### 1.1 Upstream Ontime export — what BeThere actually has

`ontime/solana-dev-thailand-26apr.csv` and `ontime/road-to-mainnet-3-bangkok.csv`
are real exports, committed to this repo:

```
Time start,Link start,Duration,Cue,Title,Skip,Note,Colour,End action,Timer type,Time warning,Time danger,Presenter
09:30,false,00:30,1,Registration,false,Registration desk opens,grey,load-next,count-down,00:05:00,00:01:00,
10:10,false,00:50,3,Rust AI and Gaming Ep. 2,false,Deep dive session,blue,load-next,count-down,00:05:00,00:02:00,Katopz
```

13 columns, header row present. Only five are public content:

| Column | Use | Notes |
|---|---|---|
| `Time start` | display time | `HH:MM`, **venue wall-clock** — see §5.1 |
| `Duration` | session length | `HH:MM` |
| `Cue` | ordering | integer, 1-based |
| `Title` | session name | the payload |
| `Presenter` | speaker | **often empty** (Registration, Networking, Group Photo) |

`Skip` is a **filter**, not content (§5.2). `Note`, `Colour`, `End action`,
`Timer type`, `Time warning`, `Time danger` are run-of-show operator fields and
must not be published by default (§5.3).

### 1.2 `ontime-rs`'s own CSV — deliberately not the target

`packages/ontime-core/src/parser.rs::parse_csv` reads a much simpler positional
shape (`cue, title, duration_sec`, no header, `#` comments, duration defaulting
to 300s). It drops `Presenter` and `Time start` entirely, so it cannot express a
public agenda. **Import the upstream 13-column export, not this.**

> **Note back to `ontime-rs`:** that parser splits on `','` with no quoted-field
> handling, while real upstream exports do quote fields (see the `Note` column in
> `solana-dev-thailand-26apr.csv`). No field in the two sample files happens to
> contain a comma, so nothing breaks *today* — but a title like
> `"Rust, Anchor and You"` would silently split into two columns. Worth a real
> CSV reader upstream.

### 1.3 `ontime-rs` backup JSON — the richer future option

`POST /data/db/download` returns the whole `Project` (minus `passwordHash`) as
camelCase JSON, with entries shaped
`EventItem { id, cue, title, duration, subtitle, presenter, note }`. Richer than
CSV and already structured — a good Phase 2+ source **if** ontime-rs is deployed
alongside. It still has no per-session URL field (§4.2).

---

## 2. Why data, and not code

`ontime-rs` is **GPLv3**, inherited from upstream Ontime. BeThere currently has
**no LICENSE file at all**.

The usual "GPL is fine for a web service, hosting isn't distribution" reasoning
**does not apply here**: BeThere ships `event-checkin-frontend_bg.wasm` to every
visitor's browser, which is conveying a binary. GPL'd Rust compiled into that
wasm would make the frontend binary a derivative work, obliging GPLv3 release of
its complete corresponding source.

Therefore:

- ✅ **Import an exported file** (CSV/JSON). Data is not a derivative work of the
  program that produced it. No linking, no license contact.
- ✅ **Call a deployed `ontime-rs` over HTTP** if live sync is ever wanted —
  separate programs at arm's length.
- ❌ **Do not** vendor `ontime-core` into this repo or add it as a Cargo
  dependency. That is the one move that pulls GPL into the wasm.

*(Engineering read, not legal advice — but §6.3 should be settled by someone who
can give the latter.)*

---

## 3. Phase 1 — CSV → `description` (no schema change)

Ships on top of `66737b0`. Converts a rundown into the agenda text the organizer
would otherwise type by hand.

**Where it runs:** admin-side, in the event form. An "Import Ontime CSV" control
next to the Description field parses the file **in the browser** and fills the
textarea. Nothing is uploaded; nothing is stored beyond the resulting text, which
the organizer can edit before saving.

**Transform:** filter `Skip == true`, sort by `Cue`, emit one line per session:

```
09:30  Registration
10:00  Opening by Solana Developer Thailand & Solana Thailand DAO
10:10  Rust AI and Gaming Ep. 2 — Katopz
11:00  Group Photo Session
11:10  Hands-on: Solana Account Model & Building Your First NFT — Golf
```

Presenter appended as `— {presenter}` only when non-empty. `white-space: pre-line`
already renders this correctly on the public page.

**Acceptance — implemented `0d07da6`, verified 2026-08-23**

Parser: `domain/src/models/rundown.rs` (`parse_ontime_csv`, `to_agenda_text`).
UI: file input beside the Description field in `event_form.rs`.
Browser verification ran against the staging deployment in a scripted Chromium
session, using the committed April export as the input file.

- [x] 3.1 `ontime/solana-dev-thailand-26apr.csv` parses to 8 rows — asserted in
      tests and observed in the browser (`line count = 8`).
- [x] 3.2 `Skip == true` rows excluded; matching is case-insensitive.
- [x] 3.3 Empty `Presenter` yields no trailing dash — no line ends in `—`.
- [x] 3.4 Quoted fields parse as one field, including embedded commas, escaped
      `""`, and newlines inside quotes.
- [x] 3.5 Ordered by `Cue` numerically (`10` after `9`); missing `Cue` falls back
      to row order.
- [x] 3.6 A malformed file leaves the textarea untouched and toasts
      *"Could not import rundown: no 'Title' column found — export the rundown
      from Ontime as CSV"* — observed in the browser, textarea still `""` after.
- [x] 3.7 26 tests in `domain/tests/rundown_import.rs`, fixtured on **both**
      committed CSVs via `include_str!`.

**Two things the browser run caught that the spec had not anticipated:**

1. **The presenter is often already inside the title.** The April export's row 2
   is titled *"Opening by Solana Developer Thailand & Solana Thailand DAO"* with
   the presenter *"Solana Developer Thailand & Solana Thailand DAO"*, so the
   naive `title — presenter` render repeated it verbatim. `to_agenda_text` now
   suppresses a presenter already named in the title (case-insensitive), with a
   regression test pinned to that real row.
2. **A file input does not re-fire `change` for the same file.** After a failed
   import the organizer would re-pick the same corrected file and nothing would
   happen. The handler now clears `input.value` after reading.

**Actual cost:** a few hours, no migration, no new endpoint, no new dependency —
the CSV reader is hand-rolled for this shape.

---

## 4. Phase 2 — structured sessions

Only worth doing once Phase 1 proves organizers actually publish agendas.

### 4.1 Schema

```sql
CREATE TABLE event_sessions (
  id           TEXT PRIMARY KEY,
  event_id     TEXT NOT NULL,
  cue          INTEGER NOT NULL,
  title        TEXT NOT NULL,
  presenter    TEXT NOT NULL DEFAULT '',
  start_local  TEXT NOT NULL DEFAULT '',  -- 'HH:MM' venue wall-clock, see §5.1
  duration_min INTEGER NOT NULL DEFAULT 0,
  slide_url    TEXT NOT NULL DEFAULT '',
  asset_key    TEXT NOT NULL DEFAULT ''   -- R2 key, see §4.2
);
```

### 4.2 Slides and files

This is the original question that started this plan, and the answer is that the
machinery already exists. `worker/src/storage.rs` has a key-scheme convention
(`slip_key`, `poster_key`, `badge_key`, `export_key`) over generic
`put_bytes`/`get_bytes`, and `POST /events/{id}/poster` is a working per-event R2
upload. Adding `session_asset_key(event_id, session_id, ext)` and a sibling
endpoint is a small, well-precedented change.

Note that **no Ontime format carries a per-session URL** — upstream CSV has no
such column, and `ontime-rs`'s `public_url` lives on `ProjectData` (the whole
project), not on `EventItem`. So `slide_url` is BeThere-side data, keyed by
`cue`, and survives re-import. Keeping it on this side avoids maintaining a fork
of ontime-rs.

### 4.3 Re-import

Rundowns change up to the morning of the event. Re-import must be **idempotent on
`(event_id, cue)`**: update title/presenter/time, preserve `slide_url` and
`asset_key`. A naive delete-and-reinsert would silently drop every uploaded deck.

---

## 5. The four things that will bite

### 5.1 `Time start` is wall-clock, not an instant — do not convert it
BeThere renders `event_start_ms` in the **viewer's** timezone
(`get_timezone_offset()` / `toLocaleString`), which is correct for an instant. An
agenda's `09:30` is the venue's wall-clock time and must be shown **as written**.
Treating it as UTC would display a Bangkok 09:30 slot as 02:30 to a European
reader. Store the string; do not parse it into an epoch. If a real instant is
ever needed, it requires an explicit venue timezone, which BeThere does not
currently store.

### 5.2 `Skip` must be honoured
Ontime uses `Skip` for slots that exist in the run-of-show but should not run.
Publishing them shows attendees sessions that will not happen.

### 5.3 `Note` is internal by default
`Note` holds operator cues ("Registration desk opens", "Deep dive session").
Harmless in the two samples, but it is an operator field and may contain anything
(staffing, contingencies, personal remarks). Do not publish it unless the
organizer explicitly opts in per event.

### 5.4 The CSV needs a real reader
The format quotes fields. A `split(',')` parser is a latent break the first time
a title or note contains a comma — the exact fragility flagged upstream in §1.2.
Do not repeat it here.

---

## 6. Open decisions

- [ ] **6.1 Is a session a first-class entity?** Phase 2 only pays for itself if
      sessions later gain per-session check-in, speaker pages, or "which talks did
      I attend". If the agenda is only ever *read*, Phase 1's text is the correct
      end state and Phase 2 should be declined.
- [ ] **6.2 Are decks public or attendee-gated?** The poster path is public, the
      slip path is attendee-scoped; slides could reasonably be either. Decides
      whether `session_asset_key` objects are served openly or behind auth.
- [ ] **6.3 BeThere's licence.** Unset today, which means all-rights-reserved —
      so nobody can legally use or contribute, and the GPL-adjacency question in
      §2 cannot be answered. Prerequisite for publishing anything here.

---

## 7. Definition of Done

- [x] Source format established from real committed exports, not assumed (§1).
- [x] Arm's-length boundary justified and its failure mode named (§2).
- [x] The four latent bugs identified before implementation (§5).
- [x] Phase 1 implemented (`0d07da6`) and its acceptance list green (§3),
      verified in a browser against staging rather than by code trace.
- [ ] §6 decisions taken.
