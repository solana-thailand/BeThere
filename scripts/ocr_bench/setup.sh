#!/usr/bin/env bash
# `.plans/035` in-browser slip OCR bench: set up a scratch dir and run it.
#
#   bash scripts/ocr_bench/setup.sh [workdir]     # default /tmp/ocr_bench
#
# Installs tesseract.js + puppeteer-core into the workdir (never into the
# repo), fetches tessdata_fast eng + tha, renders the synthetic slips, prints
# the payload sizes and runs the OCR matrix. Needs node and Google Chrome
# (override the path with CHROME=...). Exit 1 on any wrong amount read.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="${1:-/tmp/ocr_bench}"
tessdata="https://github.com/tesseract-ocr/tessdata_fast/raw/main"

mkdir -p "$work/lang"
cp "$here"/harness.html "$here"/make_slips.mjs "$here"/run_ocr.mjs "$here"/sizes.mjs "$work/"
cd "$work"

[ -f package.json ] || npm init -y >/dev/null
npm install --silent tesseract.js@7.0.0 puppeteer-core@24

core="node_modules/tesseract.js-core"
[ -d "node_modules/tesseract.js/node_modules/tesseract.js-core" ] && core="node_modules/tesseract.js/node_modules/tesseract.js-core"
cp node_modules/tesseract.js/dist/tesseract.min.js node_modules/tesseract.js/dist/worker.min.js .
# corePath is a directory, so tesseract.js picks the fastest *.wasm.js the
# browser supports (relaxed SIMD in current Chrome). Those embed the wasm as
# base64; the split *.js + *.wasm build aborted in the worker with "Invalid
# URL" under tesseract.js 7 (28 Sep 2026), so it is not what gets measured.
cp "$core"/tesseract-core-*lstm.wasm.js .

for l in eng tha; do
  [ -s "lang/$l.traineddata" ] || curl -sfL -o "lang/$l.traineddata" "$tessdata/$l.traineddata"
done

node make_slips.mjs
node sizes.mjs
node run_ocr.mjs
