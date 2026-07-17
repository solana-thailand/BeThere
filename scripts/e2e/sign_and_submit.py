#!/usr/bin/env python3
"""
Shared helper: sign a base64-encoded Solana transaction with a keypair
and submit via RPC sendTransaction.

Usage:
    python3 sign_and_submit.py <tx_b64> <keypair_json> <rpc_url>

Prints SIGNATURE=<sig> on success or ERROR=<msg> on failure.

Requires the PyNaCl package (nacl.signing for Ed25519 signing):
    python3 -m venv .venv && .venv/bin/pip install pynacl
"""

from __future__ import annotations

import base64
import json
import sys
import urllib.error
import urllib.request
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    pass

import nacl.signing


def main() -> None:
    tx_b64 = str(sys.argv[1])
    keypair_json = str(sys.argv[2])
    rpc_url = str(sys.argv[3])

    tx_bytes = bytearray(base64.b64decode(tx_b64))
    keypair_raw = json.loads(keypair_json)
    keypair_data = [int(x) for x in keypair_raw]

    signing_key = nacl.signing.SigningKey(bytes(keypair_data[:32]))

    pos = 0
    sig_count = int(tx_bytes[pos])
    pos += 1
    sig_start = pos
    pos += sig_count * 64
    message = bytes(tx_bytes[pos:])

    signed = signing_key.sign(message)
    tx_bytes[sig_start : sig_start + 64] = signed.signature

    tx_b64_signed = base64.b64encode(bytes(tx_bytes)).decode()

    payload: dict[str, Any] = {
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
            body = resp.read()
            result: dict[str, Any] = json.loads(body.decode())
            sig = result.get("result", "")
            if sig:
                print(f"SIGNATURE={sig}")  # noqa: T201
            else:
                print(f"ERROR={result}")  # noqa: T201
    except urllib.error.HTTPError as e:
        err_body = e.read().decode()[:500]
        print(f"ERROR={e}")  # noqa: T201
        if err_body:
            print(f"BODY={err_body}")  # noqa: T201
    except Exception as e:
        print(f"ERROR={e}")  # noqa: T201


if __name__ == "__main__":
    main()
