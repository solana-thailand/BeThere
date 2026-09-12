"""Asset upload for the PUT API fallback.

Mirrors wrangler's `syncAssets`:

  1. POST .../assets-upload-session with the manifest -> init JWT + buckets.
  2. For every requested hash, POST .../workers/assets/upload?base64=true as
     multipart/form-data (NOT JSON), field name = the hash, value = the base64
     contents, authorised with the init JWT. Each response carries a running
     completion JWT in `result.jwt`.
  3. The final completion JWT is what the PUT API needs in `assets.jwt`.

Bugs this flow already fixed and must keep fixed: uploading every file instead
of the requested buckets; sending application/json (uploads silently failed, so
the init JWT was used as if it were a completion JWT); and leaving the `cfwau_`
prefix on the JWT, which the PUT API rejects with 10021.
"""

import base64
import json
import os
import sys
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from .mime import mime_for

# The assets-upload-session API prefixes upload-token JWTs with `cfwau_`
# (Cloudflare Workers Assets Upload). Completion JWTs do not carry it, but the
# init JWT — returned verbatim when nothing needs uploading — sometimes does.
JWT_PREFIX = "cfwau_"


def status(message: str) -> None:
    """Progress goes to stderr so it is never captured as the JWT."""
    print(message, file=sys.stderr)


def strip_jwt_prefix(jwt: str) -> str:
    """Remove the `cfwau_` prefix the PUT API refuses to decode."""
    return jwt.removeprefix(JWT_PREFIX)


def _post_json(url: str, token: str, payload: dict) -> dict:
    request = Request(
        url,
        data=json.dumps(payload).encode(),
        headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
        method="POST",
    )
    with urlopen(request) as response:
        return json.load(response)


def _multipart_body(field: str, value: str, content_type: str) -> tuple[bytes, str]:
    """Build one multipart/form-data part; returns (body, boundary)."""
    boundary = "----bethere" + os.urandom(8).hex()
    body = (
        f"--{boundary}\r\n"
        f'Content-Disposition: form-data; name="{field}"; filename="{field}"\r\n'
        f"Content-Type: {content_type}\r\n\r\n"
        f"{value}\r\n"
        f"--{boundary}--\r\n"
    ).encode()
    return body, boundary


def upload_assets(api_base: str, script: str, oauth: str, dist: str, manifest: dict) -> str:
    """Upload every requested asset and return the completion JWT."""
    # Map hash -> path first and read nothing yet. Most deploys change a handful
    # of files; base64-ing the whole dist up front would hold the entire frontend
    # in memory to upload two of them.
    path_by_hash = {info["hash"]: relative for relative, info in manifest.items()}

    init = _post_json(
        f"{api_base}/workers/scripts/{script}/assets-upload-session",
        oauth,
        {"manifest": manifest},
    )["result"]
    init_jwt = init["jwt"]
    requested = [h for bucket in init.get("buckets", []) for h in bucket]

    if not requested:
        status("   Assets already up-to-date (no new files to upload)")
        return strip_jwt_prefix(init_jwt)

    status(f"   Uploading {len(requested)} asset file(s)...")
    upload_url = f"{api_base}/workers/assets/upload?base64=true"
    completion_jwt = ""
    for index, file_hash in enumerate(requested, 1):
        relative = path_by_hash[file_hash]
        with open(os.path.join(dist, relative.lstrip("/")), "rb") as handle:
            encoded = base64.b64encode(handle.read()).decode()
        body, boundary = _multipart_body(file_hash, encoded, mime_for(relative))
        request = Request(
            upload_url,
            data=body,
            headers={
                "Authorization": f"Bearer {init_jwt}",
                "Content-Type": f"multipart/form-data; boundary={boundary}",
            },
            method="POST",
        )
        try:
            with urlopen(request) as response:
                result = json.load(response).get("result", {})
        except HTTPError as error:
            detail = error.read().decode()[:300]
            raise RuntimeError(
                f"upload {index}/{len(requested)} failed: {error.code} {detail}"
            ) from error
        completion_jwt = result.get("jwt", "") or completion_jwt
        status(f"  Uploaded {index}/{len(requested)}")

    if not completion_jwt:
        raise RuntimeError("upload API returned no completion JWT")
    return strip_jwt_prefix(completion_jwt)
