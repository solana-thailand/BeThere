// Synthetic Thai bank-slip screenshots for the `.plans/035` OCR bench.
// Three layouts shaped like KBank / SCB / BBL confirmation screens, every one
// stamped SYNTHETIC TEST SLIP. No real slip, name or account is used. Seeded,
// so the set is the same on every run. Ground truth goes to slips/truth.json.
import puppeteer from "puppeteer-core";
import fs from "node:fs";
const CHROME=process.env.CHROME||"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
fs.mkdirSync("slips",{recursive:true});
const TH_MON=["ม.ค.","ก.พ.","มี.ค.","เม.ย.","พ.ค.","มิ.ย.","ก.ค.","ส.ค.","ก.ย.","ต.ค.","พ.ย.","ธ.ค."];
let seed=7; const rnd=()=>{seed=(seed*1103515245+12345)%2147483648; return seed/2147483648;};
const pick=a=>a[Math.floor(rnd()*a.length)];
const layouts={
 kbank:(t)=>`<div style="background:#f3faf5;padding:48px;font-family:Thonburi;color:#1a1a1a">
  <div style="color:#00a651;font-size:44px;font-weight:bold">โอนเงินสำเร็จ</div>
  <div style="font-size:30px;color:#666">${t.d} ${TH_MON[t.m]} ${t.yy} ${t.hh}:${t.mi} น.</div><hr>
  <div style="font-size:32px">${t.sender}<br><span style="color:#666">ธ.กสิกรไทย xxx-x-x${t.stail}-x</span></div>
  <div style="font-size:60px;margin:20px 0">↓</div>
  <div style="font-size:32px">${t.recv}<br><span style="color:#666">พร้อมเพย์ xxx-xxx-${t.rtail}</span></div><hr>
  <div style="font-size:28px;color:#666">เลขที่รายการ:</div><div style="font-size:30px">${t.ref}</div>
  <div style="font-size:28px;color:#666;margin-top:16px">จำนวน:</div><div style="font-size:52px;font-weight:bold">${t.amt} บาท</div>
  <div style="font-size:28px;color:#666">ค่าธรรมเนียม: 0.00 บาท</div>
  <div style="margin-top:30px;font-size:24px;color:#c00">SYNTHETIC TEST SLIP</div><div style="width:140px;height:140px;background:repeating-linear-gradient(90deg,#000 0 7px,#fff 7px 14px);margin-top:12px"></div></div>`,
 scb:(t)=>`<div style="background:#fff;padding:48px;font-family:Thonburi;color:#222">
  <div style="background:#4e2a84;color:#fff;font-size:40px;padding:20px">โอนเงินสำเร็จ</div>
  <div style="font-size:28px;color:#777;margin-top:16px">${t.d} ${TH_MON[t.m]} ${2500+t.yy} - ${t.hh}:${t.mi}</div>
  <div style="font-size:28px;color:#777">รหัสอ้างอิง: ${t.ref}</div>
  <div style="font-size:30px;margin-top:24px">จาก ${t.sender}<br>xxx-xxx${t.stail}-x</div>
  <div style="font-size:30px;margin-top:24px">ไปยัง ${t.recv}<br>xxx-xxx-${t.rtail}</div>
  <div style="display:flex;justify-content:space-between;font-size:36px;margin-top:40px"><span>จำนวนเงิน</span><b>${t.amt}</b></div>
  <div style="margin-top:30px;font-size:24px;color:#c00">SYNTHETIC TEST SLIP</div></div>`,
 bbl:(t)=>`<div style="background:#eef3fb;padding:48px;font-family:Thonburi;color:#0b2a6b">
  <div style="font-size:40px;font-weight:bold">รายการสำเร็จ</div>
  <div style="font-size:30px;margin-top:20px">จำนวนเงิน (บาท)</div><div style="font-size:56px;font-weight:bold">${t.amt}</div>
  <div style="font-size:28px">ค่าธรรมเนียม (บาท) 0.00</div>
  <div style="font-size:28px;margin-top:20px">วันที่ทำรายการ ${t.d} ${TH_MON[t.m]} ${t.yy}, ${t.hh}:${t.mi}</div>
  <div style="font-size:28px">หมายเลขอ้างอิง ${t.ref}</div>
  <div style="font-size:30px;margin-top:20px">จาก ${t.sender} xxx-x-x${t.stail}-x</div>
  <div style="font-size:30px">ไปที่ ${t.recv} PromptPay xxx-xxx-${t.rtail}</div>
  <div style="margin-top:30px;font-size:24px;color:#c00">SYNTHETIC TEST SLIP</div></div>`,
};
const names=["นาย สมชาย ใจดี","น.ส. พิมพ์ ศรีสุข","นาง นก ทองคำ","MR. JOHN DOE","นาย ก้อง ภูผา"];
const amts=["500.00","300.00","1,000.00","250.00","200.00","1,500.00","99.00"];
const b=await puppeteer.launch({executablePath:CHROME,headless:"new"});
const p=await b.newPage(); await p.setViewport({width:1080,height:1600,deviceScaleFactor:1});
const truth=[];
for(let i=0;i<24;i++){
  const kind=Object.keys(layouts)[i%3];
  const t={d:1+Math.floor(rnd()*28),m:Math.floor(rnd()*12),yy:69,hh:String(Math.floor(rnd()*24)).padStart(2,"0"),mi:String(Math.floor(rnd()*60)).padStart(2,"0"),
    sender:pick(names),recv:pick(names),stail:String(1000+Math.floor(rnd()*9000)),rtail:String(1000+Math.floor(rnd()*9000)),
    ref:String(Math.floor(rnd()*1e15)).padStart(15,"0")+(kind==="kbank"?"BOR"+String(Math.floor(rnd()*1e4)).padStart(4,"0"):""),amt:pick(amts)};
  await p.setContent(`<html><body style="margin:0">${layouts[kind](t)}</body></html>`);
  const el=await p.$("body > div"); const f=`slips/s${String(i).padStart(2,"0")}_${kind}.png`;
  await el.screenshot({path:f}); truth.push({file:f,kind,amount:t.amt.replace(/,/g,""),day:t.d,month:t.m+1,time:`${t.hh}:${t.mi}`,rtail:t.rtail});
}
fs.writeFileSync("slips/truth.json",JSON.stringify(truth,null,1)); await b.close(); console.log("made",truth.length);
