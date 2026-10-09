#!/usr/bin/env node
// The /sandbox flow end to end on devnet, through the real API and the same
// burner module the page uses (.plans/042 0.4):
//   create event -> faucet -> deposit -> check-in -> (event ends) -> refund -> return USDC
//
// Usage: node scripts/e2e/sandbox_devnet.mjs [base_url]
//   base_url defaults to http://localhost:8788; pass the staging URL after a deploy.
// Spends from the sandbox faucet (1 test USDC, returned at the end, and 0.003 SOL)
// and about 0.0043 SOL of organizer rent. Exit 0 only if every step confirmed.

import {
  base58Encode,
  createBurner,
  sendAndConfirm,
  signTransaction,
} from "../../frontend-leptos/js/sandbox_burner.js";

const base = (process.argv[2] || "http://localhost:8788").replace(/\/$/, "");
const started = Date.now();
const log = (step, detail) =>
  console.log(`[${((Date.now() - started) / 1000).toFixed(1)}s] ${step}: ${detail}`);

async function api(method, path, body) {
  const response = await fetch(`${base}/api${path}`, {
    method,
    headers: body ? { "Content-Type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  const json = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(`${method} ${path}: HTTP ${response.status} ${JSON.stringify(json)}`);
  return json.data ?? json;
}

async function rpc(url, method, params) {
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
  });
  return (await response.json()).result;
}

async function usdc(rpcUrl, owner, mint) {
  const result = await rpc(rpcUrl, "getTokenAccountsByOwner", [
    owner,
    { mint },
    { encoding: "jsonParsed", commitment: "confirmed" },
  ]);
  const account = result?.value?.[0];
  return account ? Number(account.account.data.parsed.info.tokenAmount.uiAmount) : 0;
}

const config = await api("GET", "/sandbox/config");
if (!config.enabled) throw new Error("sandbox is off on this worker");
const rpcUrl = config.browser_rpc;
log("config", `organizer ${config.organizer}, faucet ${config.faucet}`);

const burner = await createBurner();
const wallet = base58Encode(burner.publicKey);
log("burner", wallet);
const faucetBefore = await usdc(rpcUrl, config.faucet, config.usdc_mint);

const event = await api("POST", "/sandbox/events");
log("event", `${event.event_id} ends ${new Date(event.event_end * 1000).toISOString()} (${event.signature})`);

const grant = await api("POST", "/sandbox/faucet", { wallet });
log("faucet", grant.signature);
log("burner usdc", await usdc(rpcUrl, wallet, config.usdc_mint));

const deposit = await api("POST", "/sandbox/deposit-tx", { event_id: event.event_id, wallet });
log("deposit", await sendAndConfirm(rpcUrl, await signTransaction(deposit.transaction_b64, burner)));

const checkIn = await api("POST", "/sandbox/check-in", { event_id: event.event_id, wallet });
log("check-in", checkIn.signature);

// The program opens refunds at event_end by the cluster clock; wait past it.
const waitMs = event.event_end * 1000 + 8000 - Date.now();
if (waitMs > 0) {
  log("wait", `${Math.ceil(waitMs / 1000)} s for the event to end`);
  await new Promise((resolve) => setTimeout(resolve, waitMs));
}

const refund = await api("POST", "/sandbox/refund-tx", { event_id: event.event_id, wallet });
log("refund", await sendAndConfirm(rpcUrl, await signTransaction(refund.transaction_b64, burner)));
log("burner usdc", await usdc(rpcUrl, wallet, config.usdc_mint));

const back = await api("POST", "/sandbox/return-tx", { wallet });
log("return", await sendAndConfirm(rpcUrl, await signTransaction(back.transaction_b64, burner)));

const faucetAfter = await usdc(rpcUrl, config.faucet, config.usdc_mint);
log("faucet usdc", `${faucetBefore} -> ${faucetAfter}`);
if (faucetAfter !== faucetBefore) throw new Error("the faucet did not get its test USDC back");
log("done", "every step confirmed");
