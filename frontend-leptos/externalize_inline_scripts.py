#!/usr/bin/env python3
"""Move every inline executable <script> in the dist HTML pages into its own file.

Plan 029 (8.23): the CSP drops `script-src 'unsafe-inline'`. index.html has
two inline scripts: the service-worker registration (static) and Trunk's
module loader, which names the content-hashed wasm and so changes every
build. A hash-based CSP would have to change per build in both `_headers` and
the Worker's SECURITY_HEADERS. Instead, each inline script becomes
`/inline-<sha256[:16]>.js`, referenced with an SRI pin, so the policy stays a
constant and `'self'` covers them.

Execution order is unchanged: an external classic script without async/defer
runs at its position like the inline one did, and a module script is deferred
either way.

Every `*.html` at the top of the dist dir is processed: index.html, and
staff-shell.html in the staff build (.issues/169).

Runs as Trunk's post_build hook (Trunk.toml) on the staging dir, so every
`trunk build`/`trunk serve` gets it. It is also the gate: it exits 1 if an
inline executable script is still there afterwards.

Usage:
    python3 externalize_inline_scripts.py [dist_dir]   # default: $TRUNK_STAGING_DIR, else dist
    python3 externalize_inline_scripts.py --self-test
"""

import base64
import hashlib
import os
import re
import sys
import tempfile
from pathlib import Path

SCRIPT_RE = re.compile(r"<script\b([^>]*)>(.*?)</script>", re.DOTALL | re.IGNORECASE)
TYPE_RE = re.compile(r"""\btype\s*=\s*["']?([^"'\s>]+)""", re.IGNORECASE)
SRC_RE = re.compile(r"\bsrc\s*=", re.IGNORECASE)
# Types the browser executes. Anything else (application/ld+json, …) is a
# data block that CSP does not govern.
EXECUTABLE_TYPES = {"", "module", "text/javascript", "application/javascript"}


def is_inline_executable(attrs: str, body: str) -> bool:
    if SRC_RE.search(attrs) or not body.strip():
        return False
    match = TYPE_RE.search(attrs)
    kind = match.group(1).lower() if match else ""
    return kind in EXECUTABLE_TYPES


def inline_executables(html: str) -> list[str]:
    return [m.group(0)[:80] for m in SCRIPT_RE.finditer(html) if is_inline_executable(m.group(1), m.group(2))]


def pages(dist: Path) -> list[Path]:
    return sorted(dist.glob("*.html"))


def externalize(dist: Path) -> list[str]:
    written: list[str] = []

    def replace(m: re.Match) -> str:
        attrs, body = m.group(1), m.group(2)
        if not is_inline_executable(attrs, body):
            return m.group(0)
        data = body.encode("utf-8")
        name = f"inline-{hashlib.sha256(data).hexdigest()[:16]}.js"
        (dist / name).write_bytes(data)
        sri = "sha384-" + base64.b64encode(hashlib.sha384(data).digest()).decode()
        written.append(name)
        return f'<script{attrs} src="/{name}" integrity="{sri}"></script>'

    for page in pages(dist):
        html = page.read_text(encoding="utf-8")
        page.write_text(SCRIPT_RE.sub(replace, html), encoding="utf-8")
    return written


def main(dist: Path) -> int:
    if not pages(dist):
        print(f"❌ no HTML page in {dist}", file=sys.stderr)
        return 1
    written = externalize(dist)
    for name in written:
        print(f"📤 inline script → {dist / name}")
    status = 0
    for page in pages(dist):
        left = inline_executables(page.read_text(encoding="utf-8"))
        if left:
            print(f"❌ inline executable script still in {page.name}: {left}", file=sys.stderr)
            status = 1
    return status


def self_test() -> int:
    page = (
        "<head><script>navigator.serviceWorker;</script>"
        '<script type="application/ld+json">{"a":1}</script>'
        '<script defer src="https://example.invalid/x.js"></script>'
        "<script></script></head>"
        '<body><script type="module">import init from "/a.js"; await init();</script></body>'
    )
    results: list[bool] = []

    def check(name: str, ok: bool) -> None:
        print(f"  {'ok ' if ok else 'FAIL'} {name}")
        results.append(ok)

    check("detector finds both executable inline scripts", len(inline_executables(page)) == 2)
    check("detector ignores JSON-LD, src= and empty scripts",
          inline_executables('<script type="application/ld+json">{}</script><script src="/a.js"></script><script></script>') == [])
    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        (dist / "index.html").write_text(page, encoding="utf-8")
        written = externalize(dist)
        html = (dist / "index.html").read_text(encoding="utf-8")
        check("two files written", len(written) == 2 and all((dist / n).exists() for n in written))
        check("no inline executable left", inline_executables(html) == [])
        check("module type kept", '<script type="module" src="/inline-' in html)
        check("JSON-LD untouched", '<script type="application/ld+json">{"a":1}</script>' in html)
        body = (dist / written[0]).read_bytes()
        sri = "sha384-" + base64.b64encode(hashlib.sha384(body).digest()).decode()
        check("SRI pin matches the file", f'integrity="{sri}"' in html)
        check("second run is a no-op", externalize(dist) == [])
        (dist / "staff-shell.html").write_text(page, encoding="utf-8")
        again = externalize(dist)
        staff = (dist / "staff-shell.html").read_text(encoding="utf-8")
        check("a second shell is externalized too", sorted(again) == sorted(written) and inline_executables(staff) == [])
    print(f"self-test: {sum(results)}/{len(results)}")
    return 0 if all(results) else 1


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        sys.exit(self_test())
    default = os.environ.get("TRUNK_STAGING_DIR", "dist")
    sys.exit(main(Path(sys.argv[1] if len(sys.argv) > 1 else default)))
