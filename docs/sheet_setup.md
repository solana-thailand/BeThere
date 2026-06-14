# Google Sheet Provisioning — Make-a-Copy Flow

> **Goal:** cut the per-event sheet-setup friction from ~5 manual steps to ~3
> by publishing one master template that organizers copy.

BeThere uses Google Sheets as the attendee data store. Each event needs a
spreadsheet with a **33-column `Attendees` tab** + a `staff` tab, shared with
the **service account** so the worker can read/write it.

The script `scripts/create_sheet_template.gs` builds that structure. But
running it per event still means: create blank sheet → open Apps Script →
paste script → set property → run → share service account → copy `sheet_id`.
This document removes most of those steps via a **one-time master** that
organizers copy.

---

## Honest constraint (read first)

**Google does not carry editors/sharing over to a "Make a Copy".**

A copy of the master inherits the **structure** (columns, formatting,
dropdowns, conditional formatting) but **not** the editors. So each organizer
must still **re-share their copy** with the BeThere service account after
copying. This single step cannot be eliminated by the copy approach.

The only way to remove it entirely is **worker auto-provisioning** — which
requires adding the `drive.file` OAuth scope to the service account JWT
(currently it only has `spreadsheets`). See [§ Long-term fix](#long-term-fix).

Net result: **5 steps → 3 steps** today, fully one-click later.

---

## One-time: build the master (admin, ~5 min)

Done **once** by whoever administers BeThere.

1. **Create a blank spreadsheet**
   - <https://sheets.new> → rename it e.g. `BeThere Event Template (MASTER)`

2. **Open the Apps Script editor**
   - `Extensions → Apps Script`

3. **Paste the template generator**
   - Contents of `scripts/create_sheet_template.gs`

4. **Set the service account email**
   - `Project Settings → Script Properties → Add`
   - Key: `SERVICE_ACCOUNT_EMAIL`
   - Value: the `client_email` from your BeThere service account JSON
     (also bound as a Cloudflare secret on the worker)

5. **Run `createBeThereTemplate`**
   - Authorize the script when prompted
   - It builds the 33-column `Attendees` tab + `staff` tab, grants the
     service account editor access, and shows an alert containing:
     - **Spreadsheet ID** — the master's ID
     - **Make-a-copy link** — `https://docs.google.com/spreadsheets/d/{MASTER_ID}/copy`

6. **Share the master as view-only**
   - `Share → General access → Anyone with the link → Viewer`
   - (Viewers can copy unless you explicitly disable it)

7. **Save the make-a-copy link** — this is what you give to every organizer.

The master is now ready. **Never register attendees into the master** — it is
a template only.

---

## Per-event: organizer copies it (~30 sec)

Done by each event organizer when they create a new event in BeThere.

1. **Open the make-a-copy link**
   - `https://docs.google.com/spreadsheets/d/{MASTER_ID}/copy`
   - Click **Make a Copy** → a new sheet appears in their own Drive, with the
     full 33-column structure already in place (no Apps Script needed).

2. **Re-share with the service account**
   - In the copy: `Share` → paste the **service account email** → **Editor**
   - This step is mandatory — see [§ Honest constraint](#honest-constraint-read-first).

3. **Copy the `sheet_id` into BeThere**
   - The `sheet_id` is the long string in the URL between `/d/` and `/edit`:
     `https://docs.google.com/spreadsheets/d/`**`THIS_PART`**`/edit`
   - Paste it into the **Sheet ID** field of the BeThere "Create Event" form.

Done. No blank-sheet creation, no Apps Script, no column setup.

### Optional — re-run the template script on the copy

If an organizer wants to re-verify the structure (e.g. after a BeThere schema
upgrade), they can open Apps Script on their copy, set the
`SERVICE_ACCOUNT_EMAIL` property (Script Properties don't copy either), and
re-run `createBeThereTemplate`. It is idempotent — it will not delete data.

---

## Where the pieces live

| Artifact | Path | Purpose |
|---|---|---|
| Template generator | `scripts/create_sheet_template.gs` | Builds the 33-col + staff structure; now also prints `sheet_id` + copy link |
| Sheet config (code) | `domain/src/config/types.rs` — `SheetsConfig` | `sheet_id`, `sheet_name`, `staff_sheet_name` per event |
| Auth scope | `domain/src/models/auth.rs` — `ServiceAccountClaim::new` | Currently `spreadsheets` only (no Drive scope) |
| Event creation API | `domain/src/models/event.rs` — `CreateEventRequest` | Requires `sheet_id` |

---

## Long-term fix

**Auto-provision from the worker** — make event creation fully one-click.

1. Add the `https://www.googleapis.com/auth/drive.file` scope to
   `ServiceAccountClaim::new` in `domain/src/models/auth.rs` (alongside the
   existing `spreadsheets` scope — use a space-separated multi-scope string).
2. Add a `POST /api/admin/events/{id}/provision-sheet` worker handler that:
   - Calls the Drive API to **create** a new spreadsheet
   - Seeds it with the 33-column headers (a small subset of what
     `create_sheet_template.gs` does, ported to Sheets API `batchUpdate`)
   - Shares it with the creating organizer (their email from the auth claim)
3. Wire the "Create Event" form to call it automatically when no `sheet_id`
   is supplied.

**Effort:** ~1 day. The auth + Sheets write path already exist; this adds a
Drive scope + one create call + header seeding.

---

## Troubleshooting

| Symptom | Cause / Fix |
|---|---|
| Worker returns 403 on attendee read | Copy wasn't shared with the service account → re-do [§ Per-event step 2](#per-event-organizer-copies-it-30-sec) |
| Columns misaligned / dropdowns missing | Organizer edited the structure → re-run `createBeThereTemplate` on the copy (optional step above) |
| `sheet_id` rejected by Create Event form | You copied the wrong URL segment — it's between `/d/` and `/edit`, not the full URL |
| Master copy link 404s | Master was deleted or sharing was revoked → rebuild via [§ One-time](#one-time-build-the-master-admin-5-min) |