#!/usr/bin/env python3
"""
Shared helper: sign a base64-encoded Solana transaction with a keypair
and submit via RPC sendTransaction.

Usage:
    python3 sign_and_submit.py <tx_b64> <keypair_json> <rpc_url>

Prints SIGNATURE=<sig> on success or ERROR=<msg> on failure.
"""

import base64
import json
import sys
import urllib.request

tx_b64 = sys.argv[1]
keypair_json = sys.argv[2]
rpc_url = sys.argv[3]

tx_bytes = bytearray(base64.b64decode(tx_b64))
keypair_data = json.loads(keypair_json)

import nacl.signing

signing_key = nacl.signing.SigningKey(bytes(keypair_data[:32]))

pos = 0
sig_count = tx_bytes[pos]
pos += 1
sig_start = pos
pos += sig_count * 64
message = bytes(tx_bytes[pos:])

signed = signing_key.sign(message)
tx_bytes[sig_start : sig_start + 64] = signed.signature

tx_b64_signed = base64.b64encode(bytes(tx_bytes)).decode()

payload = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "sendTransaction",
    "params": [tx_b64_signed, {"encoding": "base64", "skipPreflight": True}],
}

req = urllib.request.Request(
    rpc_url,
    data=json.dumps(payload).encode(),
    headers={"Content-Type": "application/json"},
)

try:
    with urllib.request.urlopen(req, timeout=30) as resp:
        result = json.loads(resp.read().decode())
        sig = result.get("result", "")
        if sig:
            print(f"SIGNATURE={sig}")
        else:
            print(f"ERROR={result}")
except Exception as e:
    err_body = ""
    # HTTP errors may have .read() for response body
    if hasattr(e, "read"):
        err_body = e.read().decode()[:500]  # pyright: ignore
    print(f"ERROR={e}")
    if err_body:
        print(f"BODY={err_body}")
