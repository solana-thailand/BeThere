// Runs the `.plans/035` OCR matrix in headless Chrome: language data x image
// posture, one worker per cell, the page segmentation mode from PSM (default
// 11, sparse text). Prints one line per cell and writes results.json.
// Exit 1 when any cell reads a WRONG amount: a miss is allowed, a false read
// is not, so this is also the record's correctness gate.
import puppeteer from "puppeteer-core";
import http from "node:http";
import fs from "node:fs";
import path from "node:path";

const CHROME = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const PSM = process.env.PSM || "11";
const LANGS = (process.env.LANGS || "tha,eng,tha+eng").split(",");
const MIME = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".png": "image/png" };

const server = http.createServer((req, res) => {
  const file = path.join(process.cwd(), decodeURIComponent(new URL(req.url, "http://x").pathname));
  if (!file.startsWith(process.cwd()) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404).end(); return; }
  res.writeHead(200, { "content-type": MIME[path.extname(file)] || "application/octet-stream" });
  fs.createReadStream(file).pipe(res);
}).listen(0, "127.0.0.1");
await new Promise(r => server.once("listening", r));
const base = `http://127.0.0.1:${server.address().port}`;

const truth = JSON.parse(fs.readFileSync("slips/truth.json"));
const b = await puppeteer.launch({ executablePath: CHROME, headless: "new" });
const p = await b.newPage();
p.on("pageerror", e => console.error("pageerror:", e.message));
await p.goto(`${base}/harness.html`);

const out = {};
let wrong = 0;
for (const lang of LANGS) for (const mode of ["clean", "photo"]) {
  const r = await p.evaluate((l, m, t, s) => window.run(l, m, t, s), lang, mode, truth, PSM);
  const n = r.rows.length, c = k => r.rows.filter(x => x[k]).length;
  const ms = r.rows.map(x => x.ms).sort((a, b) => a - b);
  const key = `${lang}/${mode}`;
  out[key] = r;
  wrong += c("amount_wrong");
  console.log(`${key.padEnd(14)} init ${r.init}ms  median ${ms[n >> 1]}ms max ${ms[n - 1]}ms | amount ${c("amount")}/${n} WRONG ${c("amount_wrong")} | time ${c("time")}/${n} | date ${c("date")}/${n} | rtail ${c("rtail")}/${n}`);
}
fs.writeFileSync("results.json", JSON.stringify(out, null, 1));
await b.close();
server.close();
if (wrong > 0) { console.error(`❌ ${wrong} wrong amount read(s)`); process.exit(1); }
console.log("✅ no wrong amount reads");
