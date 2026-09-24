#!/usr/bin/env python3
"""Fail when `domain` pulls an app or platform crate into its wasm32 graph.

Plan 031 (from riir-reflex's import fence). `domain` is the shared core that
both the worker and the frontend build for wasm32. If it ever depends on
either of them, or on a platform crate (workers-rs, leptos, web-sys, a native
runtime), the layering is gone and every consumer inherits that crate.

The graph comes from:

    cargo tree -p event-checkin-domain --target wasm32-unknown-unknown \
      -e normal --all-features --locked --prefix none

The rules:

  * any crate in FORBIDDEN, or matching a FORBIDDEN_PREFIXES entry, fails;
  * JS-bridge crates are pinned **in both directions**. chrono's default
    `wasmbind` feature pulls `js-sys` + `wasm-bindgen` on wasm32, and
    `Utc::now()` needs them there, so they are allowed. A new bridge crate
    fails, and so does a pinned one that disappears (the pin is stale: shrink
    it so the fence tightens);
  * a blindness floor exits 2 when the graph looks empty or truncated
    (no `event-checkin-domain`, `serde` or `chrono`, or fewer than
    MIN_CRATES names), so a broken `cargo tree` never reads as clean.

Exit: 0 clean · 1 fence breached or pin stale · 2 cannot see the graph.

Usage:
    python3 scripts/verify/domain_import_fence.py
    python3 scripts/verify/domain_import_fence.py --tree-file tree.txt
    python3 scripts/verify/domain_import_fence.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

CARGO_TREE = [
    "cargo", "tree", "-p", "event-checkin-domain",
    "--target", "wasm32-unknown-unknown", "-e", "normal",
    "--all-features", "--locked", "--prefix", "none",
]

FORBIDDEN = frozenset({
    # Our own app crates: domain sits below all of them.
    "event-checkin-worker", "event-checkin-frontend", "flow-harness", "bethere-escrow",
    # Platform crates the app layers own.
    "worker", "worker-macros", "worker-sys", "web-sys", "reqwest", "tokio", "axum",
})
FORBIDDEN_PREFIXES = ("leptos", "gloo", "solana-", "wasm-bindgen-futures")

JS_BRIDGE = re.compile(r"^(js-sys|wasm-bindgen.*|web-sys)$")
JS_BRIDGE_PINNED = frozenset({
    "js-sys", "wasm-bindgen", "wasm-bindgen-macro",
    "wasm-bindgen-macro-support", "wasm-bindgen-shared",
})

REQUIRED = ("event-checkin-domain", "serde", "chrono")
MIN_CRATES = 10


def parse_tree(text: str) -> set[str]:
    """Crate names from `cargo tree --prefix none` output (`name vX.Y.Z ...`)."""
    names = set()
    for line in text.splitlines():
        parts = line.split()
        if len(parts) >= 2 and parts[1].startswith("v"):
            names.add(parts[0])
    return names


def check(names: set[str]) -> tuple[int, list[str]]:
    missing = [r for r in REQUIRED if r not in names]
    if missing or len(names) < MIN_CRATES:
        return 2, [f"blind: {len(names)} crates, missing {missing or 'none'} (floor {MIN_CRATES})"]

    problems = []
    for name in sorted(names):
        match name:
            case n if n in FORBIDDEN or n.startswith(FORBIDDEN_PREFIXES):
                problems.append(f"forbidden crate in domain's wasm32 graph: {n}")
            case n if JS_BRIDGE.match(n) and n not in JS_BRIDGE_PINNED:
                problems.append(f"unpinned JS-bridge crate: {n}")
    for stale in sorted(JS_BRIDGE_PINNED - names):
        problems.append(f"stale pin, no longer in the graph (remove it from JS_BRIDGE_PINNED): {stale}")
    return (1 if problems else 0), problems


def run(names: set[str]) -> int:
    code, problems = check(names)
    for p in problems:
        print(f"❌ {p}")
    if code == 0:
        print(f"✅ domain wasm32 graph: {len(names)} crates, no app/platform crates, "
              f"JS bridge = pinned {len(JS_BRIDGE_PINNED)}")
    return code


def self_test() -> int:
    clean = {*REQUIRED, *JS_BRIDGE_PINNED, "serde_json", "num-traits", "itoa", "memchr"}
    cases = [
        ("clean graph", clean, 0),
        ("worker app crate", clean | {"event-checkin-worker"}, 1),
        ("workers-rs", clean | {"worker"}, 1),
        ("leptos by prefix", clean | {"leptos_dom"}, 1),
        ("web-sys", clean | {"web-sys"}, 1),
        ("new JS bridge crate", clean | {"wasm-bindgen-backend"}, 1),
        ("stale pin", clean - {"js-sys"}, 1),
        ("empty graph", set(), 2),
        ("domain missing", clean - {"event-checkin-domain"}, 2),
        ("tiny graph", {*REQUIRED}, 2),
    ]
    failed = 0
    for label, names, want in cases:
        got, _ = check(names)
        ok = got == want
        failed += not ok
        print(f"{'✅' if ok else '❌'} {label}: exit {got} (want {want})")
    parsed = parse_tree("event-checkin-domain v0.1.0 (/x)\nchrono v0.4.44\nserde v1.0.0 (*)\n\n")
    parse_ok = parsed == {"event-checkin-domain", "chrono", "serde"}
    failed += not parse_ok
    print(f"{'✅' if parse_ok else '❌'} parse_tree: {sorted(parsed)}")
    print(f"self-test: {len(cases) + 1 - failed}/{len(cases) + 1}")
    return 1 if failed else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tree-file", type=Path, help="saved `cargo tree` output instead of running cargo")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if args.tree_file:
        return run(parse_tree(args.tree_file.read_text()))
    proc = subprocess.run(CARGO_TREE, cwd=ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        print(f"❌ cargo tree failed ({proc.returncode}):\n{proc.stderr}", file=sys.stderr)
        return 2
    return run(parse_tree(proc.stdout))


if __name__ == "__main__":
    sys.exit(main())
