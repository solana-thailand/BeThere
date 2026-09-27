// Download cost of the `.plans/035` OCR payload, brotli q4 (what Cloudflare
// serves; see scripts/verify/frontend_size_budget.sh) and q11.
import fs from "node:fs";
import z from "node:zlib";
const br = (b, q) => z.brotliCompressSync(b, { params: { [z.constants.BROTLI_PARAM_QUALITY]: q, [z.constants.BROTLI_PARAM_SIZE_HINT]: b.length } }).length;
// The core Chrome actually fetched in the bench (see setup.sh).
const engine = ["tesseract.min.js", "worker.min.js", "tesseract-core-relaxedsimd-lstm.wasm.js"];
const lang = ["lang/eng.traineddata", "lang/tha.traineddata"];
const sum = { engine: 0 };
for (const f of [...engine, ...lang]) {
  const b = fs.readFileSync(f), q4 = br(b, 4);
  console.log(f.padEnd(34), "raw", String(b.length).padStart(9), "br4", String(q4).padStart(9), "br11", String(br(b, 11)).padStart(9));
  if (engine.includes(f)) sum.engine += q4; else sum[f] = q4;
}
console.log("engine br4", sum.engine, "| +tha", sum.engine + sum["lang/tha.traineddata"], "| +eng", sum.engine + sum["lang/eng.traineddata"]);
