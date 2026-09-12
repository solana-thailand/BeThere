"""Asset manifest for the assets-upload-session API.

The hash must match wrangler's `hashFile`: BLAKE3 over the base64 of the file
contents concatenated with the bare extension, truncated to 32 hex chars. A
mismatch makes the API request buckets the uploader cannot satisfy.
"""

import base64
import os

import blake3

# `_headers` / `_redirects` are Cloudflare *config* files parsed by
# `wrangler deploy`, not servable assets. The PUT fallback cannot apply them,
# and uploading them anyway just exposes them as fetchable blobs (Issue #057).
CONFIG_FILES = frozenset({"_headers", "_redirects"})


def asset_hash(contents: bytes, extension: str) -> str:
    """Reproduce wrangler's content hash for one asset."""
    b64 = base64.b64encode(contents).decode()
    return blake3.blake3((b64 + extension).encode()).hexdigest()[:32]


def build_manifest(dist: str) -> dict[str, dict[str, object]]:
    """Walk `dist` and return the `{ '/path': {hash, size} }` manifest."""
    manifest: dict[str, dict[str, object]] = {}
    for root, _dirs, files in os.walk(dist):
        for name in files:
            if name in CONFIG_FILES:
                continue
            full_path = os.path.join(root, name)
            relative = "/" + os.path.relpath(full_path, dist)
            with open(full_path, "rb") as handle:
                contents = handle.read()
            extension = os.path.splitext(full_path)[1].lstrip(".")
            manifest[relative] = {
                "hash": asset_hash(contents, extension),
                "size": len(contents),
            }
    return manifest
