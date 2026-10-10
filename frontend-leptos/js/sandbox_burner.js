/**
 * Devnet burner wallet for `/sandbox` (.plans/042 0.4).
 *
 * A throwaway Ed25519 key made in this browser with WebCrypto, kept in
 * localStorage, used only on devnet. It signs the transactions the Worker
 * builds (deposit, refund, returning the test USDC) and sends them to the
 * public devnet RPC itself, so the Worker never relays a visitor transaction.
 *
 * No web3.js: a legacy transaction is `[sig count][64-byte slots][message]`,
 * and signing is one Ed25519 signature over the message, written into the
 * slot of the key's index among the required signers.
 *
 * The pure helpers take their storage and fetch, so a Node script can drive
 * the same code end to end (Node 20+ has WebCrypto Ed25519).
 *
 * Imported via `#[wasm_bindgen(module = "/js/sandbox_burner.js")]`.
 */

const STORAGE_KEY = "bethere.sandbox.burner.v1";
const ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const CONFIRM_POLLS = 40;
const CONFIRM_POLL_MS = 1000;

export function base58Encode(bytes) {
  const digits = [];
  for (const byte of bytes) {
    let carry = byte;
    for (let i = 0; i < digits.length; i++) {
      carry += digits[i] << 8;
      digits[i] = carry % 58;
      carry = (carry / 58) | 0;
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = (carry / 58) | 0;
    }
  }
  let out = "";
  for (const byte of bytes) {
    if (byte !== 0) break;
    out += "1";
  }
  for (let i = digits.length - 1; i >= 0; i--) out += ALPHABET[digits[i]];
  return out;
}

function toB64(bytes) {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

function fromB64(text) {
  const s = atob(text);
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

/** A fresh burner: `{ privateKey: CryptoKey, publicKey: Uint8Array(32), pkcs8: Uint8Array }`. */
export async function createBurner(subtle = globalThis.crypto.subtle) {
  const pair = await subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"]);
  const publicKey = new Uint8Array(await subtle.exportKey("raw", pair.publicKey));
  const pkcs8 = new Uint8Array(await subtle.exportKey("pkcs8", pair.privateKey));
  return { privateKey: pair.privateKey, publicKey, pkcs8 };
}

async function importBurner(saved, subtle = globalThis.crypto.subtle) {
  const pkcs8 = fromB64(saved.pkcs8);
  const privateKey = await subtle.importKey("pkcs8", pkcs8, { name: "Ed25519" }, true, ["sign"]);
  return { privateKey, publicKey: fromB64(saved.publicKey), pkcs8 };
}

/** The stored burner, or a new one saved to `storage`. */
export async function loadOrCreateBurner(storage = globalThis.localStorage) {
  const raw = storage.getItem(STORAGE_KEY);
  if (raw) {
    try {
      return await importBurner(JSON.parse(raw));
    } catch (_) {
      storage.removeItem(STORAGE_KEY);
    }
  }
  const burner = await createBurner();
  storage.setItem(
    STORAGE_KEY,
    JSON.stringify({ pkcs8: toB64(burner.pkcs8), publicKey: toB64(burner.publicKey) }),
  );
  return burner;
}

/** The slot `publicKey` signs in, among the required signers of `tx`. */
export function signerSlot(tx, publicKey) {
  const signatures = tx[0];
  if (!signatures || signatures >= 0x80) throw new Error("malformed transaction");
  const message = tx.subarray(1 + 64 * signatures);
  const required = message[0];
  const keyCount = message[3];
  if (required !== signatures || keyCount >= 0x80 || keyCount < required) {
    throw new Error("malformed transaction");
  }
  for (let i = 0; i < required; i++) {
    const key = message.subarray(4 + 32 * i, 4 + 32 * (i + 1));
    if (key.every((b, j) => b === publicKey[j])) return i;
  }
  throw new Error("this wallet is not a signer of the transaction");
}

/** Sign base64 `txB64` with `burner`; returns the signed bytes. */
export async function signTransaction(txB64, burner, subtle = globalThis.crypto.subtle) {
  const tx = fromB64(txB64);
  const slot = signerSlot(tx, burner.publicKey);
  const message = tx.subarray(1 + 64 * tx[0]);
  const signature = new Uint8Array(await subtle.sign({ name: "Ed25519" }, burner.privateKey, message));
  tx.set(signature, 1 + 64 * slot);
  return tx;
}

/** Attempts per RPC call: the public devnet RPC answers 429 under load, and
 *  a room on one Wi-Fi shares one IP (a 429 hit a staging run on 2026-10-10). */
const RPC_ATTEMPTS = 5;

/** Wait before attempt `n` (1-based): Retry-After when the server sends it,
 *  else 1 s, 2 s, 4 s, 8 s with up to 30 % jitter so a room does not retry in step. */
export function retryDelayMs(n, retryAfter) {
  const header = Number(retryAfter);
  if (Number.isFinite(header) && header > 0) return Math.min(header, 15) * 1000;
  const base = 1000 * 2 ** (n - 1);
  return Math.round(base * (1 + Math.random() * 0.3));
}

/** One JSON-RPC call, retried on 429, 5xx and network errors. Resending a
 *  signed transaction is safe: its signature is its identity. */
async function rpc(rpcUrl, method, params, fetchFn, attempts = RPC_ATTEMPTS) {
  let last;
  for (let n = 1; n <= attempts; n++) {
    let response;
    try {
      response = await fetchFn(rpcUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: "sandbox", method, params }),
      });
    } catch (e) {
      last = new Error(`${method}: ${e && e.message ? e.message : e}`);
      if (n < attempts) await sleep(retryDelayMs(n));
      continue;
    }
    if (response.status === 429 || response.status >= 500) {
      last = new Error(`${method}: HTTP ${response.status}`);
      if (n < attempts) {
        const header = response.headers && response.headers.get ? response.headers.get("Retry-After") : null;
        await sleep(retryDelayMs(n, header));
      }
      continue;
    }
    if (!response.ok) throw new Error(`${method}: HTTP ${response.status}`);
    const json = await response.json();
    if (json.error) throw new Error(`${method}: ${json.error.message || JSON.stringify(json.error)}`);
    return json.result;
  }
  throw last;
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** Send signed bytes and wait for `confirmed`; returns the signature. */
export async function sendAndConfirm(rpcUrl, tx, fetchFn = globalThis.fetch.bind(globalThis)) {
  const signature = await rpc(
    rpcUrl,
    "sendTransaction",
    [toB64(tx), { encoding: "base64", preflightCommitment: "confirmed" }],
    fetchFn,
  );
  for (let i = 0; i < CONFIRM_POLLS; i++) {
    await sleep(CONFIRM_POLL_MS);
    const result = await rpc(rpcUrl, "getSignatureStatuses", [[signature]], fetchFn);
    const status = result && result.value && result.value[0];
    if (!status) continue;
    if (status.err) throw new Error(`transaction failed: ${JSON.stringify(status.err)}`);
    if (status.confirmationStatus === "confirmed" || status.confirmationStatus === "finalized") {
      return signature;
    }
  }
  throw new Error(`not confirmed after ${CONFIRM_POLLS} s: ${signature}`);
}

/** Wait until `signature` (sent by a real wallet) is `confirmed`; returns it. */
export async function confirmSignature(rpcUrl, signature, fetchFn = globalThis.fetch.bind(globalThis)) {
  for (let i = 0; i < CONFIRM_POLLS; i++) {
    await sleep(CONFIRM_POLL_MS);
    const result = await rpc(rpcUrl, "getSignatureStatuses", [[signature]], fetchFn);
    const status = result && result.value && result.value[0];
    if (!status) continue;
    if (status.err) throw new Error(`transaction failed: ${JSON.stringify(status.err)}`);
    if (status.confirmationStatus === "confirmed" || status.confirmationStatus === "finalized") {
      return signature;
    }
  }
  throw new Error(`not confirmed after ${CONFIRM_POLLS} s: ${signature}`);
}

// ---- Bindings for the Leptos page (browser only) ----

/** The burner's address, creating the burner on first use. */
export async function burnerAddress() {
  const burner = await loadOrCreateBurner();
  return base58Encode(burner.publicKey);
}

/** Sign `txB64` with the stored burner, send it, wait; returns the signature. */
export async function burnerSignAndSend(rpcUrl, txB64) {
  const burner = await loadOrCreateBurner();
  const tx = await signTransaction(txB64, burner);
  return sendAndConfirm(rpcUrl, tx);
}

/** Drop the burner; the next call makes a new one. */
export function forgetBurner() {
  globalThis.localStorage.removeItem(STORAGE_KEY);
}

/** Whether this browser can make the burner (WebCrypto Ed25519). */
export async function burnerSupported() {
  try {
    await createBurner();
    return true;
  } catch (_) {
    return false;
  }
}
