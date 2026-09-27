# 001 — In-browser slip OCR: tesseract.js 7 payload and accuracy on synthetic Thai slips
Status: valid
Rung: download cost and field accuracy of tesseract.js for the W1 0 THB path, before any product code (035 §2; 033 W1)
Session: event-checkin-4d, 1790533326
Commit: f5daa71f
Gate: `bash scripts/ocr_bench/setup.sh /tmp/ocr_bench` exit 0
Lanes: single
Load: MacBook M5 Pro, AC power, headless Chrome (stable, 28 Sep 2026), other agent sessions idle; files served from 127.0.0.1

## Method

`scripts/ocr_bench/setup.sh` installs tesseract.js 7.0.0 (core 7.0.0) and
puppeteer-core into a scratch dir, fetches `tessdata_fast` `eng` and `tha`, and
renders 24 synthetic slips. They use three layouts shaped like KBank, SCB and
BBL confirmation screens, and each one is stamped SYNTHETIC TEST SLIP. It then
runs `run_ocr.mjs`: one worker per cell, PSM 11 (sparse text), for each
language (`tha`, `eng`, `tha+eng`) × posture:

- `clean`: the rendered 1080 px screenshot.
- `photo`: half scale, 2° tilt, 0.8 px blur, JPEG q45.

`harness.html`'s `extract()` reads the amount, the time and the Thai date.
`rtail` only asks whether the receiver's 4-digit tail appears anywhere in the
OCR text; no tail extractor was written. **WRONG** counts amounts that were
read but differ from the truth. A miss (null) is not WRONG.

Sizes come from `sizes.mjs`, using node zlib brotli at q4 (what Cloudflare
serves, per `scripts/verify/frontend_size_budget.sh`) and at q11.

## Result

Payload, fetched lazily and never on first load:

| Part | raw | br q4 | br q11 |
|---|---:|---:|---:|
| `tesseract.min.js` | 62,961 | 9,763 | 8,242 |
| `worker.min.js` | 111,307 | 31,770 | 27,550 |
| `tesseract-core-relaxedsimd-lstm.wasm.js` | 3,905,767 | 1,374,067 | 1,123,820 |
| **engine total** | | **1,415,600** | |
| `tha.traineddata` (fast) | 1,072,600 | 896,837 | 885,638 |
| `eng.traineddata` (fast) | 4,113,088 | 1,909,751 | 1,633,293 |
| **engine + tha** | | **2,312,437** | |

Accuracy, n = 24 per cell, PSM 11:

| Cell | amount | WRONG | time | date | rtail in text | median / max ms |
|---|---:|---:|---:|---:|---:|---|
| tha / clean | 24 | 0 | 24 | 18 | 24 | 245 / 329 |
| tha / photo | 24 | 0 | 24 | 8 | 24 | 171 / 229 |
| eng / clean | 24 | 0 | 24 | 0 | 20 | 105 / 203 |
| eng / photo | 24 | 0 | 24 | 0 | 24 | 68 / 104 |
| tha+eng / clean | 24 | 0 | 24 | 18 | 21 | 194 / 253 |
| tha+eng / photo | 24 | 0 | 24 | 8 | 24 | 163 / 196 |

Worker init is 60–90 ms with the files already local. On a real network the
download above dominates.

## Notes

- **The gate can fail.** With the first amount regex (`\b\d{1,3}(?:,\d{3})*\.\d{2}\b`)
  at PSM 3, `tha/photo` read "1,500.00" as "1.500.00" and took "500.00": 3
  WRONG, exit 1. The shipped regex refuses a figure that starts or ends inside
  a number, and it folds a misread "1.500.00" back to 1500.00.
- **PSM matters more than language.** At PSM 3 (the default), 7 of 8 BBL-style
  slips lost the large bold amount line entirely (tha/clean 17/24). PSM 11
  read all of them.
- **Two traps found:**
  - Tesseract emits sara am decomposed (U+0E4D U+0E32), so a label regex
    written with ำ never matches until the text is folded.
  - `corePath` pointing at a directory loads the base64 `*.wasm.js`. The
    split `*.js` + `*.wasm` build (1,039,308 B br4 for the wasm) aborted in
    the worker with "Invalid URL", so it is not what this record measures.
- **Not measured:** real slips, any phone, and any bank layout beyond these
  three.
  - Synthetic, uniformly rendered text is a best case for Tesseract, so the
    accuracy here is an upper bound.
  - A 6× CDP CPU throttle did not reach the Web Worker (timings unchanged), so
    there is no phone number.
