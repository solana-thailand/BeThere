#!/usr/bin/env python3
"""Fence for the staff-only stylesheets (.issues/169).

`frontend-leptos/styles/style-NN-*.staff.css` hold rules that only the staff
shell (staff-app.html) links. Their rules are there because every selector
names a class the attendee app never renders. If an attendee page later starts
using one of those classes, it renders unstyled in the attendee shell and
nothing else notices: the staff shell, where people test admin pages, still
looks right.

This gate reads the attendee wasm that dist/index.html loads and applies the
split's own test to every selector in the .staff.css sheets: it fails when a
selector's classes ALL occur in that wasm, because then it may match on an
attendee page that the attendee shell leaves unstyled. `.admin-panel.open` is
fine while `admin-panel` is staff-only, even though `open` is everywhere. The
fix is to move the rule back into the attendee sheet it came from.

A class name "occurs" when its bytes appear anywhere in the wasm, which
over-counts (substrings of longer names match too). That errs toward failing,
which is the safe side for a fence. It assumes no class name is assembled
from fragments at runtime; tests/css_class_audit.rs documents that invariant.

Usage:
  python3 scripts/verify/staff_css_fence.py [--dist frontend-leptos/dist]
  python3 scripts/verify/staff_css_fence.py --self-test

Exit: 0 clean, 1 an attendee-used class is staff-only, 2 broken invocation.
"""
from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
STYLES = ROOT / "frontend-leptos" / "styles"
CLASS_RE = re.compile(r"\.(-?[_a-zA-Z][_a-zA-Z0-9-]*)")
COMMENT_RE = re.compile(r"/\*.*?\*/", re.S)


def selectors(css: str) -> list[tuple[str, frozenset[str]]]:
    """Every selector with its class names (declaration bodies are skipped)."""
    css = COMMENT_RE.sub("", css)
    out: list[tuple[str, frozenset[str]]] = []
    # A selector list is the text before a `{` since the previous `{`, `}` or `;`.
    for head in re.findall(r"([^{};]+)\{", css):
        head = head.strip()
        if head.startswith("@"):
            continue
        for sel in head.split(","):
            sel = " ".join(sel.split())
            if sel:
                out.append((sel, frozenset(CLASS_RE.findall(sel))))
    return out


def staff_selectors(styles: pathlib.Path) -> list[tuple[str, frozenset[str]]]:
    out: list[tuple[str, frozenset[str]]] = []
    for sheet in sorted(styles.glob("style-*.staff.css")):
        out += [(f"{sheet.name}: {sel}", names) for sel, names in selectors(sheet.read_text())]
    return out


def attendee_wasm(dist: pathlib.Path) -> bytes:
    index = (dist / "index.html").read_text()
    m = re.search(r'(event-checkin-frontend-[0-9a-f]+_bg\.wasm)', index)
    if not m:
        raise SystemExit(f"❌ no wasm referenced by {dist / 'index.html'}")
    return (dist / m.group(1)).read_bytes()


def leaks(sels: list[tuple[str, frozenset[str]]], wasm: bytes) -> list[str]:
    """Selectors that may match in the attendee app: every class occurs in it.

    A selector with no class at all (an element or :root rule) cannot be told
    apart from attendee markup, so it counts as a leak too.
    """
    return sorted(sel for sel, names in sels if all(n.encode() in wasm for n in names))


def self_test() -> int:
    sels = selectors(
        "/* .ignored { } */ .only-staff, .panel.open:hover { color: red }"
        " @media (max-width: 1px) { .in-media .x { top: 0 } } body { margin: 0 }"
    )
    assert [s for s, _ in sels] == [".only-staff", ".panel.open:hover", ".in-media .x", "body"], sels
    wasm = b"\x00<div class=\"only-staff open\">\x00in-media\x00x"
    found = leaks(sels, wasm)
    assert found == [".in-media .x", ".only-staff", "body"], found
    assert leaks(selectors(".panel.open { top: 0 }"), wasm) == []
    print("✅ staff_css_fence self-test: a selector the attendee app can match fails; one naming a staff-only class passes")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--dist", default=str(ROOT / "frontend-leptos" / "dist"))
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    dist = pathlib.Path(args.dist)
    if not (dist / "index.html").is_file():
        print(f"❌ {dist}/index.html missing: build the frontend first", file=sys.stderr)
        return 2
    sels = staff_selectors(STYLES)
    if not sels:
        print("❌ no staff-only selectors found: is styles/ missing its .staff.css sheets?", file=sys.stderr)
        return 2
    found = leaks(sels, attendee_wasm(dist))
    if found:
        print("❌ staff-only selectors that may match on attendee pages:", file=sys.stderr)
        for sel in found:
            print(f"   {sel}", file=sys.stderr)
        print("   Move those rules from style-NN-*.staff.css back into the attendee sheet.", file=sys.stderr)
        return 1
    print(f"✅ staff_css_fence: {len(sels)} staff-only selectors, none can match in the attendee wasm")
    return 0


if __name__ == "__main__":
    sys.exit(main())
