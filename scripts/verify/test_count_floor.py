#!/usr/bin/env python3
"""Fail when a cargo test binary passes fewer tests than its recorded floor.

Plan 030 (the "green-zero" lie from katgpt-rs). `cargo test` prints
`test result: ok. 0 passed` and exits 0 when a whole test file stops running.
That happens after a module split that drops a `mod` line, a `#![cfg(feature)]`
that CI never enables, or a renamed file. `domain/tests/alloc_count.rs` was
exactly that: it compiled in CI on every run and executed 0 of its 10 tests.

This gate parses a saved `cargo test` log. Each test binary is keyed by its
target name plus source (`pod tests/pod.rs`, `doc-tests event_checkin_domain`)
and compared with `test_floors.json`:

  * a binary below its floor fails (this includes dropping to 0);
  * a binary in the floors file but missing from the log fails (the file or
    target was removed);
  * any `test result: FAILED` or a non-zero `filtered out` fails;
  * a log with no result lines at all fails (the step captured nothing);
  * a binary in the log but not in the floors file is reported, not failed;
    run `--update` to start guarding it.

`--update` only raises floors and adds new binaries. Lowering a floor or
removing a binary needs `--allow-decrease`, so the commit that shrinks a suite
has to be a deliberate one that says why.

Usage:
    cargo test --workspace --locked 2>&1 | tee ws.log
    python3 scripts/verify/test_count_floor.py --suite workspace ws.log
    python3 scripts/verify/test_count_floor.py --suite workspace ws.log --update
    python3 scripts/verify/test_count_floor.py --self-test
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from pathlib import Path

FLOORS = Path(__file__).resolve().with_name("test_floors.json")

RUNNING_RE = re.compile(r"^\s*Running (?P<src>\S+(?: \S+)?) \((?P<path>[^)]+)\)\s*$")
DOCTESTS_RE = re.compile(r"^\s*Doc-tests (?P<name>\S+)\s*$")
RESULT_RE = re.compile(
    r"^test result: (?P<status>ok|FAILED)\. (?P<passed>\d+) passed; (?P<failed>\d+) failed;"
    r".*?(?P<filtered>\d+) filtered out"
)
HASH_SUFFIX_RE = re.compile(r"-[0-9a-f]{16}$")
# CI sets CARGO_TERM_COLOR=always, so `Running` / `Doc-tests` arrive wrapped in
# SGR escapes and would never match the patterns above.
ANSI_RE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


class ParseError(Exception):
    pass


def binary_key(src: str, path: str) -> str:
    """`unittests src/lib.rs` + `.../deps/event_checkin_domain-<hash>` → `event_checkin_domain src/lib.rs`."""
    target = HASH_SUFFIX_RE.sub("", Path(path).name.removesuffix(".exe"))
    return f"{target} {src.removeprefix('unittests ')}"


def parse_log(text: str) -> tuple[dict[str, int], list[str]]:
    """Return passed counts per binary, plus hard problems found in the log."""
    counts: dict[str, int] = {}
    problems: list[str] = []
    current: str | None = None
    for raw in text.splitlines():
        line = ANSI_RE.sub("", raw)
        if m := RUNNING_RE.match(line):
            current = binary_key(m["src"], m["path"])
            continue
        if m := DOCTESTS_RE.match(line):
            current = f"doc-tests {m['name']}"
            continue
        m = RESULT_RE.match(line)
        if not m:
            continue
        if current is None:
            raise ParseError(f"result line with no preceding binary: {line!r}")
        if current in counts:
            raise ParseError(f"binary reported twice (ambiguous key): {current}")
        counts[current] = int(m["passed"])
        if m["status"] != "ok" or int(m["failed"]):
            problems.append(f"{current}: {m['failed']} failed")
        if int(m["filtered"]):
            problems.append(f"{current}: {m['filtered']} filtered out (a filter hides tests from the count)")
        current = None
    if not counts:
        problems.append("no `test result:` lines in the log (did the test step run and was its output captured?)")
    return counts, problems


def check(counts: dict[str, int], floors: dict[str, int]) -> tuple[list[str], list[str]]:
    failures: list[str] = []
    notices: list[str] = []
    for key, floor in sorted(floors.items()):
        if key not in counts:
            failures.append(f"{key}: missing from the log (floor {floor})")
        elif counts[key] < floor:
            failures.append(f"{key}: {counts[key]} passed, floor {floor}")
    for key in sorted(counts.keys() - floors.keys()):
        notices.append(f"{key}: {counts[key]} passed, not guarded yet (run --update)")
    return failures, notices


def update(counts: dict[str, int], floors: dict[str, int], allow_decrease: bool) -> tuple[dict[str, int], list[str]]:
    refused: list[str] = []
    new = dict(floors)
    for key, n in counts.items():
        if key in floors and n < floors[key] and not allow_decrease:
            refused.append(f"{key}: {floors[key]} → {n}")
            continue
        new[key] = n
    for key in floors.keys() - counts.keys():
        if allow_decrease:
            del new[key]
        else:
            refused.append(f"{key}: would be removed")
    return dict(sorted(new.items())), refused


def load_floors(path: Path) -> dict[str, dict[str, int]]:
    return json.loads(path.read_text()) if path.exists() else {}


def run(suite: str, log: Path, floors_path: Path, do_update: bool, allow_decrease: bool) -> int:
    if not log.is_file():
        print(f"❌ {log}: log file missing")
        return 2
    try:
        counts, problems = parse_log(log.read_text(errors="replace"))
    except ParseError as e:
        print(f"❌ {log}: {e}")
        return 2
    all_floors = load_floors(floors_path)
    floors = all_floors.get(suite, {})
    total = sum(counts.values())

    if do_update:
        if problems:
            print("❌ refusing to update floors from a bad log:")
            print("\n".join(f"   {p}" for p in problems))
            return 1
        new, refused = update(counts, floors, allow_decrease)
        if refused:
            print("❌ these floors would go down; re-run with --allow-decrease and say why in the commit:")
            print("\n".join(f"   {r}" for r in refused))
            return 1
        all_floors[suite] = new
        floors_path.write_text(json.dumps(dict(sorted(all_floors.items())), indent=2) + "\n")
        print(f"✅ {suite}: {len(new)} binaries, {sum(new.values())} tests floored in {floors_path.name}")
        return 0

    if not floors:
        print(f"❌ suite {suite!r} has no floors in {floors_path.name} (run --update)")
        return 1
    failures, notices = check(counts, floors)
    failures = problems + failures
    for n in notices:
        print(f"ℹ️  {n}")
    if failures:
        print(f"❌ {suite}: {len(failures)} test-count problem(s):")
        print("\n".join(f"   {f}" for f in failures))
        return 1
    print(f"✅ {suite}: {len(counts)} binaries, {total} passed (floor {sum(floors.values())})")
    return 0


def self_test() -> int:
    """Prove each failure mode fires and a healthy log passes."""

    def log(*bins: tuple[str, str, int]) -> str:
        out = []
        for src, target, n in bins:
            if src == "doc":
                out.append(f"   Doc-tests {target}")
            else:
                out.append(f"     Running {src} (/t/debug/deps/{target}-0123456789abcdef)")
            out.append(f"test result: ok. {n} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s")
        return "\n".join(out) + "\n"

    healthy = log(("unittests src/lib.rs", "dom", 5), ("unittests src/main.rs", "dom", 0),
                  ("tests/pod.rs", "pod", 3), ("doc", "dom", 0))
    floors = {"dom src/lib.rs": 5, "dom src/main.rs": 0, "pod tests/pod.rs": 3, "doc-tests dom": 0}
    failed_log = healthy.replace("3 passed; 0 failed", "2 passed; 1 failed").replace("ok. 2", "FAILED. 2")
    filtered_log = healthy.replace("0 filtered out", "4 filtered out", 1)
    # What cargo writes under CARGO_TERM_COLOR=always (CI).
    coloured = (healthy.replace("     Running", "\x1b[1m\x1b[92m     Running\x1b[0m")
                .replace("   Doc-tests", "\x1b[1m\x1b[92m   Doc-tests\x1b[0m"))

    cases: list[tuple[str, str, dict[str, int], int]] = [
        ("healthy log passes", healthy, floors, 0),
        ("a colour-coded (CI) log passes", coloured, floors, 0),
        ("more tests than the floor passes", healthy.replace("5 passed", "9 passed"), floors, 0),
        ("a binary dropping to 0 fails", healthy.replace("5 passed", "0 passed"), floors, 1),
        ("a binary below its floor fails", healthy.replace("3 passed", "2 passed"), floors, 1),
        ("a vanished binary fails", log(("unittests src/lib.rs", "dom", 5), ("unittests src/main.rs", "dom", 0), ("doc", "dom", 0)), floors, 1),
        ("a FAILED result fails", failed_log, floors, 1),
        ("a filtered run fails", filtered_log, floors, 1),
        ("an empty log fails", "   Compiling dom v0.1.0\n", floors, 1),
        ("a suite with no floors fails", healthy, {}, 1),
        ("an unguarded new binary is reported, not failed", healthy + log(("tests/new.rs", "new", 2)), floors, 0),
    ]
    failures = 0
    with tempfile.TemporaryDirectory() as tmp:
        for name, text, fl, want in cases:
            log_path = Path(tmp, "t.log")
            log_path.write_text(text)
            floors_path = Path(tmp, "floors.json")
            floors_path.write_text(json.dumps({"s": fl} if fl else {}))
            got = run("s", log_path, floors_path, False, False)
            ok = got == want
            failures += not ok
            print(f"{'✅' if ok else '❌'} {name} (exit {got}, want {want})")

        # --update ratchets up, refuses to go down, and accepts a decrease only when told to.
        floors_path = Path(tmp, "floors.json")
        log_path = Path(tmp, "t.log")
        floors_path.write_text(json.dumps({"s": floors}))
        log_path.write_text(healthy.replace("3 passed", "2 passed"))
        refused = run("s", log_path, floors_path, True, False) == 1 and json.loads(floors_path.read_text())["s"] == floors
        allowed = run("s", log_path, floors_path, True, True) == 0 and json.loads(floors_path.read_text())["s"]["pod tests/pod.rs"] == 2
        log_path.write_text(healthy.replace("5 passed", "7 passed"))
        raised = run("s", log_path, floors_path, True, False) == 0 and json.loads(floors_path.read_text())["s"]["dom src/lib.rs"] == 7
        for name, ok in (("--update refuses a decrease", refused), ("--allow-decrease lowers a floor", allowed), ("--update raises a floor", raised)):
            failures += not ok
            print(f"{'✅' if ok else '❌'} {name}")

    total = len(cases) + 3
    print(f"\nself-test: {total - failures}/{total} passed")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("log", nargs="?", type=Path, help="saved `cargo test` output")
    parser.add_argument("--suite", help="floor group in test_floors.json (workspace, frontend, ...)")
    parser.add_argument("--update", action="store_true", help="raise floors / add binaries from the log")
    parser.add_argument("--allow-decrease", action="store_true", help="with --update, also lower or drop floors")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if not args.log or not args.suite:
        parser.error("LOG and --suite are required unless --self-test")
    return run(args.suite, args.log, FLOORS, args.update, args.allow_decrease)


if __name__ == "__main__":
    sys.exit(main())
