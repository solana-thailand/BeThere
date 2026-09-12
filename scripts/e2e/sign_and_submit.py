#!/usr/bin/env python3
"""
Shared helper: sign a base64-encoded Solana transaction with a keypair,
submit via RPC sendTransaction, then confirm and inspect meta.err.

Why confirmation: sendTransaction with skipPreflight returns a signature
even when the transaction later FAILS on-chain (e.g. InvalidAccountData,
Custom program errors). Without confirming and reading meta.err, callers
cannot distinguish a genuinely successful TX from a failed one. This helper
blocks until the TX is available via getTransaction, then reports STATUS.

Usage:
    SIGNER_KEYPAIR_PATH=~/.config/solana/id.json \
    SOLANA_RPC_URL=https://api.devnet.solana.com \
        python3 sign_and_submit.py <tx_b64>

The signer is taken as a **path** and read here, so the secret key never
appears in this process's argument vector — argv is readable by any local user
via `ps`, which is how the key used to leak (.issues/073). The RPC URL moves to
the environment for the same reason: it may carry an `?api-key=` credential.

Legacy positional form, still accepted so older callers keep working:
    python3 sign_and_submit.py <tx_b64> <keypair_json> <rpc_url>
It passes the raw key on the command line and warns on stderr. Do not add new
callers that use it.

Output lines (in order):
    SIGNATURE=<sig>     emitted when sendTransaction accepts the TX
    STATUS=CONFIRMED    TX landed in a block and meta.err is null (genuine success)
    STATUS=FAILED       TX landed but meta.err is non-null; an ERR= line follows
    STATUS=TIMEOUT      getTransaction never returned the TX within CONFIRM_TIMEOUT
    ERR=<json>          only emitted when STATUS=FAILED
    ERROR=<msg>         emitted (instead of SIGNATURE=) when submission itself failed

Environment:
    SIGNER_KEYPAIR_PATH path to a Solana keypair JSON file (preferred over argv)
    SOLANA_RPC_URL      RPC endpoint (preferred over argv)
    CONFIRM_TIMEOUT     seconds to wait for getTransaction to surface the TX (default 45)

Requires the PyNaCl package (nacl.signing for Ed25519 signing):
    python3 -m venv .venv && .venv/bin/pip install pynacl
"""

from __future__ import annotations

import base64
import json
import os
import pathlib
import sys
import time
import urllib.error
import urllib.request
from typing import Any

import nacl.signing


def _rpc(
    method: str, params: list[Any], rpc_url: str, timeout: float = 30.0
) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    }
    req = urllib.request.Request(
        rpc_url,
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode())


def _confirm_and_check(sig: str, rpc_url: str, max_wait: float) -> tuple[str, str]:
    """Poll getTransaction until available, then inspect meta.err.

    Returns (status, err) where status is CONFIRMED | FAILED | TIMEOUT and
    err is the JSON-encoded meta.err (empty for CONFIRMED/TIMEOUT unless an
    RPC error was observed, which is returned in the err slot for context).
    """
    deadline = time.monotonic() + max_wait
    last_note = ""
    while time.monotonic() < deadline:
        time.sleep(2.0)
        try:
            res = _rpc(
                "getTransaction",
                [sig, {"maxSupportedTransactionVersion": 0}],
                rpc_url,
            )
        except Exception as e:  # noqa: BLE001 - transient RPC errors are retried
            last_note = f"rpc error: {e}"
            continue
        tx = res.get("result")
        if tx is None:
            continue  # not yet rooted in a block
        meta = tx.get("meta") or {}
        err = meta.get("err")
        if err is None:
            return ("CONFIRMED", "")
        return ("FAILED", json.dumps(err))
    return ("TIMEOUT", last_note or "tx not available within timeout")


def _load_keypair_json() -> str:
    """Return the keypair JSON, preferring the path in the environment.

    Reading the file here keeps the secret out of argv. The positional form is
    still honoured for callers that have not migrated, but it warns, because
    anything in argv is world-readable locally while the process lives.
    """
    path = os.environ.get("SIGNER_KEYPAIR_PATH", "")
    if path:
        resolved = pathlib.Path(path).expanduser()
        try:
            return resolved.read_text()
        except OSError as exc:
            raise SystemExit(
                f"ERROR: cannot read SIGNER_KEYPAIR_PATH={resolved}: {exc}"
            ) from exc

    if len(sys.argv) > 2 and sys.argv[2]:
        print(  # noqa: T201
            "WARNING: keypair passed as a command-line argument, which exposes "
            "it via `ps`. Use SIGNER_KEYPAIR_PATH instead (.issues/073).",
            file=sys.stderr,
        )
        return str(sys.argv[2])

    raise SystemExit(
        "ERROR: no signer. Set SIGNER_KEYPAIR_PATH=<keypair.json>."
    )


def _resolve_rpc_url() -> str:
    """Return the RPC endpoint, preferring the environment over argv."""
    from_env = os.environ.get("SOLANA_RPC_URL", "")
    if from_env:
        return from_env

    if len(sys.argv) > 3 and sys.argv[3]:
        return str(sys.argv[3])

    raise SystemExit(
        "ERROR: no RPC endpoint. Set SOLANA_RPC_URL=<url>."
    )


def main() -> None:
    if len(sys.argv) < 2 or not sys.argv[1]:
        raise SystemExit(
            "ERROR: missing <tx_b64>. Usage: "
            "SIGNER_KEYPAIR_PATH=<path> SOLANA_RPC_URL=<url> "
            "python3 sign_and_submit.py <tx_b64>"
        )

    tx_b64 = str(sys.argv[1])
    keypair_json = _load_keypair_json()
    rpc_url = _resolve_rpc_url()
    confirm_timeout = float(os.environ.get("CONFIRM_TIMEOUT", "45"))

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

    try:
        result = _rpc(
            "sendTransaction",
            [tx_b64_signed, {"encoding": "base64", "skipPreflight": True}],
            rpc_url,
        )
    except urllib.error.HTTPError as e:
        err_body = e.read().decode()[:500]
        print(f"ERROR={e}")  # noqa: T201
        if err_body:
            print(f"BODY={err_body}")  # noqa: T201
        return
    except Exception as e:  # noqa: BLE001
        print(f"ERROR={e}")  # noqa: T201
        return

    sig = result.get("result", "")
    if not sig:
        print(f"ERROR={result}")  # noqa: T201
        return

    print(f"SIGNATURE={sig}")  # noqa: T201
    status, err = _confirm_and_check(sig, rpc_url, confirm_timeout)
    print(f"STATUS={status}")  # noqa: T201
    if err:
        print(f"ERR={err}")  # noqa: T201


if __name__ == "__main__":
    main()
