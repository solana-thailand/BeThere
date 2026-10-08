#!/usr/bin/env python3
"""gmail_refresh_token.py: one-time consent for the subscribe sender (plan 045 R4.12).

    python3 scripts/gmail_refresh_token.py <client_secret.json> [out_file]

Run it once on the owner's machine after `docs/gmail_sender_setup.md` steps
1-5. It opens the browser on Google's consent page for the single scope
`gmail.send` (send only: it cannot read, list or delete mail). Sign in as the
sender account, allow, and the script writes the refresh token to `out_file`
(default `~/.bethere-gmail-refresh-token`, mode 600). The token is never
printed. Standard library only; the flow is a loopback redirect with PKCE.

Exit 0 when the file holds a refresh token; 1 otherwise.
"""
from __future__ import annotations

import base64
import hashlib
import http.server
import json
import os
import secrets
import sys
import threading
import urllib.parse
import urllib.request
import webbrowser
from pathlib import Path

SCOPE = "https://www.googleapis.com/auth/gmail.send"
AUTH_URL = "https://accounts.google.com/o/oauth2/v2/auth"
TOKEN_URL = "https://oauth2.googleapis.com/token"
DEFAULT_OUT = Path.home() / ".bethere-gmail-refresh-token"


def load_client(path: Path) -> tuple[str, str]:
    """client_id and client_secret from the JSON Google Cloud downloads."""
    data = json.loads(path.read_text())
    block = data.get("installed") or data.get("web")
    if not block:
        raise SystemExit("❌ not an OAuth client file (no 'installed' or 'web' key)")
    return block["client_id"], block["client_secret"]


def pkce_pair() -> tuple[str, str]:
    verifier = secrets.token_urlsafe(64)
    challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
    return verifier, challenge


def auth_url(client_id: str, redirect: str, state: str, challenge: str) -> str:
    query = {
        "client_id": client_id,
        "redirect_uri": redirect,
        "response_type": "code",
        "scope": SCOPE,
        "access_type": "offline",
        "prompt": "consent",
        "state": state,
        "code_challenge": challenge,
        "code_challenge_method": "S256",
    }
    return f"{AUTH_URL}?{urllib.parse.urlencode(query)}"


def wait_for_code(
    state: str,
) -> tuple[dict[str, str], http.server.HTTPServer, threading.Thread]:
    """Serve one loopback request in the background.

    The returned dict gains `redirect` now and `code` (or `error`) once the
    browser comes back.
    """
    result: dict[str, str] = {}

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self) -> None:  # noqa: N802 (stdlib name)
            params = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query)
            ok = params.get("state", [""])[0] == state and "code" in params
            if ok:
                result["code"] = params["code"][0]
            else:
                result["error"] = params.get("error", ["state mismatch"])[0]
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.end_headers()
            self.wfile.write(b"Done. You can close this tab." if ok else b"Failed. See the terminal.")

        def log_message(self, *_args: object) -> None:
            return

    server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
    redirect = f"http://127.0.0.1:{server.server_port}"
    result["redirect"] = redirect
    thread = threading.Thread(target=server.handle_request, daemon=True)
    thread.start()
    return result, server, thread


def exchange(client_id: str, client_secret: str, code: str, redirect: str, verifier: str) -> dict:
    body = urllib.parse.urlencode({
        "client_id": client_id,
        "client_secret": client_secret,
        "code": code,
        "code_verifier": verifier,
        "grant_type": "authorization_code",
        "redirect_uri": redirect,
    }).encode()
    request = urllib.request.Request(TOKEN_URL, data=body, method="POST")
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.loads(response.read())


def main() -> int:
    if len(sys.argv) not in (2, 3) or sys.argv[1] in ("-h", "--help"):
        print(__doc__, file=sys.stderr)
        return 1
    client_id, client_secret = load_client(Path(sys.argv[1]).expanduser())
    out = Path(sys.argv[2]).expanduser() if len(sys.argv) == 3 else DEFAULT_OUT

    state = secrets.token_urlsafe(24)
    verifier, challenge = pkce_pair()
    result, server, thread = wait_for_code(state)
    url = auth_url(client_id, result["redirect"], state, challenge)
    print("Opening the consent page. Sign in as the SENDER account (bethere.sol@gmail.com).")
    print("If the browser does not open, paste this URL:\n" + url)
    webbrowser.open(url)
    thread.join(timeout=300)
    server.server_close()

    if "code" not in result:
        print(f"❌ consent failed: {result.get('error', 'timed out after 5 minutes')}", file=sys.stderr)
        return 1
    tokens = exchange(client_id, client_secret, result["code"], result["redirect"], verifier)
    refresh = tokens.get("refresh_token")
    if not refresh:
        print("❌ no refresh_token in Google's reply (was 'prompt=consent' skipped?)", file=sys.stderr)
        return 1
    if SCOPE not in tokens.get("scope", "").split():
        print(f"❌ granted scope is {tokens.get('scope')!r}, expected {SCOPE}", file=sys.stderr)
        return 1

    fd = os.open(out, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as handle:
        handle.write(refresh)
    os.chmod(out, 0o600)
    print(f"✅ refresh token written to {out} (mode 600, gmail.send only). Tell the agent it is ready.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
