#!/usr/bin/env python3
"""Check `.benchmarks/NNN_*.md` records and every citation of them.

Plan 031 §2 (rules from docs/gist_rs_study.md: a rule without a check rotted
within a day upstream). The rules themselves live in `.benchmarks/README.md`.
Numbers are allocated by `numbering_gate.py --next .benchmarks`, which also
catches duplicates.

Each record opens with a header block, one `Key: value` per line, before the
first `##`:

    Status:   valid | retracted: <why> | superseded by NNN
    Rung:     what is measured, with its plan ref
    Session:  <session-name>, <unix-epoch>
    Commit:   <sha, 7+ hex>
    Gate:     `<correctness command>` exit 0
    Lanes:    single | <A> vs <B>, interleaved
    Load:     box / environment during the run

Citations: any `.benchmarks/NNN` in `.plans`, `.issues`, `docs`, README.md or
CLAUDE.md must resolve to a record. Citing a retracted record needs the word
"retract" on the same line, so a dead number cannot be quoted as live.

Usage:
    python3 scripts/verify/bench_records.py              # check, exit 1 on failure
    python3 scripts/verify/bench_records.py --self-test  # prove the check can fail
"""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
from dataclasses import dataclass
from enum import Enum
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BENCH_DIR = ".benchmarks"
RECORD_RE = re.compile(r"^(\d{3})_.+\.md$")
CITE_RE = re.compile(r"(?<![\w/.-])\.benchmarks/(\d{3})")  # `repo/.benchmarks/` is another repo
FIELDS = ("Status", "Rung", "Session", "Commit", "Gate", "Lanes", "Load")
CITING_GLOBS = (".plans/*.md", ".issues/*.md", "docs/**/*.md", "README.md", "CLAUDE.md")


class Verdict(Enum):
    VALID = "valid"
    RETRACTED = "retracted"
    SUPERSEDED = "superseded"


@dataclass(frozen=True)
class Record:
    number: str
    name: str
    verdict: Verdict | None
    errors: tuple[str, ...]


def header(text: str) -> dict[str, str]:
    """`Key: value` lines before the first `##` heading."""
    fields: dict[str, str] = {}
    for line in text.splitlines():
        if line.startswith("## "):
            break
        match = re.match(r"^([A-Z][a-z]+):\s*(.*)$", line)
        if match:
            fields.setdefault(match.group(1), match.group(2).strip())
    return fields


def parse_status(value: str, known: set[str]) -> tuple[Verdict | None, list[str]]:
    match value.split(maxsplit=1):
        case ["valid", *_]:
            return Verdict.VALID, []
        case [word, rest] if word.rstrip(":") == "retracted" and rest.strip():
            return Verdict.RETRACTED, []
        case [word, *_] if word.rstrip(":") == "retracted":
            return Verdict.RETRACTED, ["Status: retracted needs the reason"]
        case ["superseded", rest]:
            target = re.fullmatch(r"by (\d{3})\b.*", rest.strip())
            if target is None:
                return Verdict.SUPERSEDED, ["Status: superseded needs 'by NNN'"]
            if target.group(1) not in known:
                return Verdict.SUPERSEDED, [f"Status: superseded by {target.group(1)}, which does not exist"]
            return Verdict.SUPERSEDED, []
        case _:
            return None, [f"Status: '{value}' is not valid | retracted: <why> | superseded by NNN"]


def check_record(number: str, path: Path, known: set[str]) -> Record:
    fields = header(path.read_text())
    errors = [f"missing {name}:" for name in FIELDS if not fields.get(name)]
    verdict, status_errors = parse_status(fields.get("Status", ""), known) if fields.get("Status") else (None, [])
    errors += status_errors
    gate = fields.get("Gate", "")
    if gate and not (re.search(r"`[^`]+`", gate) and re.search(r"\bexit 0\b", gate)):
        errors.append("Gate: needs a `command` and 'exit 0' (green before a number is quoted)")
    lanes = fields.get("Lanes", "")
    if lanes and lanes != "single" and not (" vs " in lanes and "interleaved" in lanes):
        errors.append("Lanes: 'single', or '<A> vs <B>, interleaved' (same session)")
    commit = fields.get("Commit", "")
    if commit and not re.fullmatch(r"[0-9a-f]{7,40}\b.*", commit):
        errors.append("Commit: needs a 7+ hex sha")
    session = fields.get("Session", "")
    if session and not re.fullmatch(r"\S+, \d{10}", session):
        errors.append("Session: '<session-name>, <unix-epoch>'")
    return Record(number, path.name, verdict, tuple(errors))


def check(root: Path) -> tuple[list[str], int]:
    """Return (violations, record count)."""
    directory = root / BENCH_DIR
    if not directory.is_dir():
        return [f"{BENCH_DIR}: directory missing"], 0
    paths = {m.group(1): p for p in sorted(directory.iterdir()) if (m := RECORD_RE.match(p.name))}
    known = set(paths)
    records = {n: check_record(n, p, known) for n, p in paths.items()}
    errors = [f"{BENCH_DIR}/{r.name}: {e}" for r in records.values() for e in r.errors]
    for pattern in CITING_GLOBS:
        for path in sorted(root.glob(pattern)):
            for lineno, line in enumerate(path.read_text().splitlines(), 1):
                for cited in CITE_RE.findall(line):
                    where = f"{path.relative_to(root)}:{lineno}"
                    match records.get(cited):
                        case None:
                            errors.append(f"{where}: cites {BENCH_DIR}/{cited}, which does not exist")
                        case Record(verdict=Verdict.RETRACTED) if "retract" not in line.lower():
                            errors.append(f"{where}: cites retracted {BENCH_DIR}/{cited} as if live")
    return errors, len(records)


RECORD = """# {n} — fixture
Status: {status}
Rung: 028 M1 check-in cpuTime
Session: event-checkin-00, 1790000000
Commit: {commit}
Gate: {gate}
Lanes: {lanes}
Load: idle laptop

## Result
"""


def self_test() -> int:
    good = dict(status="valid", commit="4798c2c", gate="`cargo test -p event-checkin-worker` exit 0",
                lanes="baseline vs cached-key, interleaved")
    cases: list[tuple[str, dict[str, dict[str, str]], str, bool]] = [
        ("clean record", {"001": good}, "", True),
        ("gate not green", {"001": {**good, "gate": "`cargo test` exit 1"}}, "", False),
        ("gate without command", {"001": {**good, "gate": "tests pass, exit 0"}}, "", False),
        ("lanes not interleaved", {"001": {**good, "lanes": "baseline vs cached-key"}}, "", False),
        ("single lane", {"001": {**good, "lanes": "single"}}, "", True),
        ("bad status word", {"001": {**good, "status": "done"}}, "", False),
        ("retracted without reason", {"001": {**good, "status": "retracted"}}, "", False),
        ("superseded by missing", {"001": {**good, "status": "superseded by 009"}}, "", False),
        ("superseded by existing", {"001": {**good, "status": "superseded by 002"}, "002": good}, "", True),
        ("bad commit", {"001": {**good, "commit": "HEAD"}}, "", False),
        ("dangling citation", {"001": good}, "see .benchmarks/007\n", False),
        ("live citation", {"001": good}, "p50 1.2 ms (.benchmarks/001)\n", True),
        ("another repo's record", {"001": good}, "see katgpt-rs/.benchmarks/881_x.md\n", True),
        ("retracted cited as live",
         {"001": {**good, "status": "retracted: load artefact, see 002"}, "002": good},
         "p50 1.2 ms (.benchmarks/001)\n", False),
        ("retracted cited as retracted",
         {"001": {**good, "status": "retracted: load artefact, see 002"}, "002": good},
         "was 1.2 ms (.benchmarks/001, retracted)\n", True),
    ]
    failed = 0
    for label, files, plan_text, want_clean in cases:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / BENCH_DIR).mkdir()
            (root / ".plans").mkdir()
            for number, values in files.items():
                (root / BENCH_DIR / f"{number}_fixture.md").write_text(RECORD.format(n=number, **values))
            (root / ".plans" / "001_fixture.md").write_text(plan_text)
            errors, _ = check(root)
            ok = (not errors) == want_clean
            failed += not ok
            print(f"  {'ok ' if ok else '❌ '} {label} → {'clean' if not errors else errors[0]}")
    with tempfile.TemporaryDirectory() as tmp:
        errors, _ = check(Path(tmp))
        ok = bool(errors)
        failed += not ok
        print(f"  {'ok ' if ok else '❌ '} missing directory → {errors[0] if errors else 'clean'}")
    total = len(cases) + 1
    if failed:
        print(f"❌ self-test: {failed}/{total} failed")
        return 1
    print(f"✅ self-test: {total}/{total}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true", help="prove the check can fail")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    errors, count = check(ROOT)
    for error in errors:
        print(f"❌ {error}")
    if errors:
        return 1
    print(f"✅ bench records clean ({count} record(s); citations resolve)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
