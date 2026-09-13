"""Content types for assets uploaded through the PUT API fallback.

The upload endpoint stores and later serves each object with the Content-Type
declared on its multipart part. The 2026-07-26 incident proved this: the
fallback's hardcoded `application/octet-stream` part was served verbatim, so
the browser downloaded the JS glue instead of executing it.

Mirror wrangler's `syncAssets`: type each part by extension, add charset=utf-8
to text/*, and for an unmapped extension sniff rather than guess octet-stream.
`application/null` is wrangler's sentinel for "store no Content-Type"; it lets
Cloudflare pick, which is strictly better than forcing a download.
"""

import mimetypes
import os

# Explicit map first — `mimetypes` is platform-dependent (it reads /etc/mime.types)
# and must never decide the type of a shipped asset.
MIME_BY_EXTENSION = {
    "html": "text/html; charset=utf-8",
    "htm": "text/html; charset=utf-8",
    "js": "text/javascript; charset=utf-8",
    "mjs": "text/javascript; charset=utf-8",
    "css": "text/css; charset=utf-8",
    "txt": "text/plain; charset=utf-8",
    "wgsl": "text/plain; charset=utf-8",
    "xml": "application/xml",
    "wasm": "application/wasm",
    "json": "application/json",
    "map": "application/json",
    "webmanifest": "application/manifest+json",
    "svg": "image/svg+xml",
    "png": "image/png",
    "ico": "image/x-icon",
    "jpg": "image/jpeg",
    "jpeg": "image/jpeg",
    "gif": "image/gif",
    "webp": "image/webp",
    "woff": "font/woff",
    "woff2": "font/woff2",
    "ttf": "font/ttf",
    "otf": "font/otf",
}

# Wrangler's sentinel for "store no Content-Type and let the edge decide".
UNKNOWN_TYPE = "application/null"


def mime_for(path: str) -> str:
    """Return the Content-Type to declare for `path`."""
    ext = os.path.splitext(path)[1].lstrip(".").lower()
    if ext in MIME_BY_EXTENSION:
        return MIME_BY_EXTENSION[ext]
    return mimetypes.guess_type(path)[0] or UNKNOWN_TYPE
