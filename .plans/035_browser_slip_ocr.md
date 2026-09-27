# Plan 035: slip OCR in the organizer's browser (W1 0 THB path)

**Status:** open. Measured 28 Sep 2026 by `event-checkin-4d`; no product code
has been written yet. This carries out the `.plans/033` §4 note "The 0 THB path: OCR in
the browser".
**Numbers:** `.benchmarks/001` (tesseract.js 7, 24 synthetic slips × 6 cells,
payload at brotli q4). Bench: `scripts/ocr_bench/`.
**Go/no-go:** §4. A set of real slips must show 0 wrong amounts before any of
§3 ships.

## 1. The decision: OCR runs on the organizer's device, not the attendee's

Plan 033 said "on the attendee's device". The measurement and the trust model
both say otherwise.

1. **Trust.** Whatever the attendee's browser sends is a claim. The QR path
   gets away with this because the server can check the claim: the CRC and
   the reference's uniqueness are re-verified in the worker. An OCR amount
   can't be checked the same way. Checking it means reading the image, which
   is the CPU the free plan doesn't have (`.issues/134`). A doctored slip plus
   `ocr: {amount: 500}` would pass the amount check. The organizer's own
   browser is not the adversary. It reads the image we stored, as it is
   stored.
2. **Payload.** Engine plus Thai data is **2,312,437 B brotli**
   (`.benchmarks/001`). That is more than the whole first load (baseline
   1,765,890). Attendees would pay for it on venue mobile data. Organizers
   use a laptop, and only on the Deposits screen.
3. **Privacy is unchanged.** The admin browser already downloads the slip
   image to display it. No third party is involved, and nothing new leaves
   our storage.

Cost: a proposal gets its OCR fields when an organizer opens the card, not at
upload. In shadow mode that is enough, because the number the plan publishes
compares the proposal with the organizer's decision, and the reading comes
first.

## 2. What was measured (`.benchmarks/001`)

- **Amount: 24/24 in every cell, 0 WRONG at PSM 11.** At PSM 3 (the default),
  the big bold amount on the BBL-style layout is dropped. With the naive regex
  there were 3 WRONG ("1.500.00" read as 500.00); the bench gate catches that.
- **Time: 24/24 everywhere.**
- **Thai date:** needs `tha` data. It reads 18/24 clean and 8/24 as a photo,
  so it is an optional field and never a gate.
- **Receiver tail:** present in the text 20–24/24. No extractor has been
  written or measured yet.
- **Speed** (M5, headless Chrome): with `tha`, median 171–245 ms and max
  329 ms per slip. No phone timing was taken, and none is needed for §1.
- **Language:** `tha` alone. `eng` reads no date and costs 1.0 MB more.
  `tha+eng` gains nothing.

**Not measured:** real slips, other bank apps (KTB, Krungsri, TTB, GSB,
TrueMoney, and so on), photographed paper, dark mode. Synthetic text is a
best case, so treat 24/24 as an upper bound.

## 3. Build (step 1 done; steps 2–5 after §4 passes)

Steps, in order. Each step is its own commit.

1. **Parser in `domain`, not JS.**
   - Port `extract()` from `scripts/ocr_bench/harness.html` to
     `domain/src/slip_ocr.rs`, and port the bench cases to tests.
   - Add the misreads the bench found: "1.500.00", decomposed ำ, a fee line,
     "0.00", and a figure inside a longer digit run.
   - Reuse `slip_proposal::parse_thb_satang`.
   - The **server** runs it. The browser sends the raw OCR text (capped at
     4 KiB), and the worker parses it. This is the same rule as `slip_qr`: one
     parser, and the server decides. The parse is string work in the
     microsecond range.
2. **Storage in a new table, not a CHECK rebuild.**
   - Create `slip_ocr_readings(event_id, attendee_id, amount_satang,
     transferred_at, receiver_tail, engine, read_by, created_at)`, with
     PRIMARY KEY (event_id, attendee_id).
   - Delete readings together with the deposit, the same way as
     `slip_proposals` (90-day retention).
   - Why a new table: widening `slip_proposals.source` means a SQLite table
     rebuild. A new table can't break an existing writer (memory
     `migration-constraints-break-existing-writers`).
3. **Re-evaluate through the single writer.**
   - The authed endpoint `POST /api/admin/deposits/{event}/{attendee}/ocr`
     stores the reading.
   - It then re-runs the checker through the same `slip_agent` function that
     `propose_after_upload` uses. The facts are merged: the QR's reference plus
     the OCR's amount, time and tail.
   - No second verdict writer (memory `duplicated-state-transition-paths`).
   - Scope every lookup by `event_id` (memory `attendee-id-is-global`).
   - Auto-accept stays off (033 W1 step 5).
4. **Frontend.**
   - Self-host the files under `/ocr/` using versioned names, cached
     immutable, and not referenced by `index.html`. The budget gate must stay
     unchanged; confirm that with `frontend_size_budget.sh`.
   - Add `js/slip_ocr.js`, loaded on demand from the Deposits card through a
     "Read slip" action, one reading at a time.
   - Worker options: `workerBlobURL: false`, because CSP has no `worker-src`
     and `blob:` would fall back to `script-src` and be refused. Also set
     `cacheMethod: "none"`: the SW or the HTTP cache holds the data, and
     tesseract's IndexedDB copy would duplicate it.
   - Pin the files' hashes in a test, the way `vendored_jsqr_integrity.rs`
     pins jsQR.
   - Add the licences (Apache-2.0 for both engine and data) to
     `THIRD_PARTY_LICENSES.md`, then run `third_party_licenses.sh`.
5. **Verify** on staging by opening the page (`docs/web-verification-runbook.md`):
   read one slip, see the merged proposal line, and confirm that the first
   load did not grow.

## 4. Go/no-go: real slips (needs the owner)

- The slips must be the **owner's own** transfers, one per bank app they can
  reach (at least 5 apps). These are PDPA-clean. No attendee slips are used
  without the §4 Q3 approval in plan 033.
- Add them to the bench as a local-only directory that is never committed,
  with truth written by hand.
- **Pass:** 0 WRONG amounts. Misses are allowed and are reported as a rate.
- **Fail:** stay QR-only (plan 033 fallback). Record the result as the
  next `.benchmarks` record, marked a negative result.
- Possible cheap fixes before giving up:
  - crop to the amount block;
  - a digits-only second pass (`tessedit_char_whitelist`);
  - the split core (saves ~310 KB, but it aborted under tesseract.js 7;
    see `.benchmarks/001` notes).

## 5. Timing against plan 033

- W1 goes to prod in shadow mode on **Fri 3 Oct**, QR path only. This plan
  does not block that.
- §3 step 1 (the parser) was safe to build before §4, and it is done.
  Step 2 waits with the rest. A migration that nobody writes to would still
  have to be applied before the 3 Oct prod deploy, since `deploy.sh` doesn't
  apply migrations. If §4 fails, it would be dead schema.
- Plan 033's "no new features after 9 Oct" still holds. If §4 has not passed
  by **Wed 8 Oct**, OCR moves past the submission and the W1 story is the QR
  path plus the checker.

## 6. Progress log

### 28 Sep: §3.1 parser in `domain`

- `domain/src/slip_ocr.rs` provides `facts_from_ocr_text` and
  `MAX_OCR_TEXT_BYTES`. It reads the amount and the transfer time (Bangkok →
  UTC, BE year). `bank_ref` and `receiver_account` are always `None`.
- No `regex` dependency, because the crate ships to wasm32.
- It adds one guard the bench's JS extractor lacked: only `:` counts as a
  time separator, so an amount such as "20.00" can't be read as 20:00.
- Tests: `domain/tests/slip_ocr.rs` (13). Fixtures are raw tesseract text from
  the bench slips, plus the misreads.
- Mutation-checked: 6 of 6 mutants fail a test:
  - dropping the sara-am fold;
  - `.` allowed as a time separator;
  - label and fee rules ignored;
  - an ambiguous figure taken anyway;
  - any lead-group length accepted;
  - a time taken from another line.
- Nothing calls the parser yet. §3.2 waits for §4 (see §5).
