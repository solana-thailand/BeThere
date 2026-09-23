#!/usr/bin/env python3
"""Fail when two `.issues` / `.plans` / `.handovers` files share a number.

Plan 030 (adapted from katgpt-rs `scripts/numbering_gate.py`). Sessions pick the
next number by eye ("ls, take the last one, add one"), so two sessions working in
the same tree at once pick the same number. "Issue 124" then means two different
things, and every cross-reference to it is ambiguous. By 2026-09-23 this had
already happened for plan 014 and handovers 001, 124 and 126.

The fix has two parts:

  * Allocation: each directory holds a `.highwater` file with the highest
    number handed out. `--next DIR` reads it, adds one, writes it back under
    an exclusive lock and prints the number. Two sessions calling it at once
    get different numbers.
  * Detection: the default run fails on any number that more than one `.md`
    file uses, unless the exact set of files is pinned below with a reason. A
    pinned group fails too if a file joins it (a new collision hiding behind
    an old one) or leaves it (the pin is stale, so delete it). The run also
    fails if `.highwater` is below the highest number on disk, because that
    means a file was numbered by eye.

Only `.md` files count. `.issues/091_backfill.sql` sits next to
`.issues/091_*.md` on purpose, as the issue's companion script.

Usage:
    python3 scripts/verify/numbering_gate.py              # check, exit 1 on failure
    python3 scripts/verify/numbering_gate.py --next .issues   # allocate + print
    python3 scripts/verify/numbering_gate.py --self-test  # prove the gate can fail
"""

from __future__ import annotations

import argparse
import fcntl
import re
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DIRS = (".issues", ".plans", ".handovers")
HIGHWATER = ".highwater"
NUMBERED_RE = re.compile(r"^(\d{3})_.+\.md$")

# Collisions that happened before the gate existed. Renumbering them would break
# every commit message and document that cites them, so they are pinned instead.
# Keyed by (directory, number); the value is the exact file set and the reason.
PINNED: dict[tuple[str, str], tuple[frozenset[str], str]] = {
    (".plans", "014"): (
        frozenset({
            "014_feature_flag_discipline.md",
            "014_goat_gate_discipline.md",
            "014_katgpt_rs_paradigm_migration.md",
            "014_negative_results.md",
            "014_no_transformer_vm.md",
            "014_phase2_4_typestate_audit.md",
            "014_policy_audit.md",
            "014_ssot_audit.md",
            "014_wire_audit.md",
        }),
        "one plan split into sibling documents under a shared number",
    ),
    (".handovers", "001"): (
        frozenset({
            "001_compilation_fix_and_cleanup.md",
            "001_deposit_module_extraction.md",
        }),
        "two sessions both started numbering at 001",
    ),
    (".handovers", "124"): (
        frozenset({
            "124_browser_test_checklist.md",
            "124_campaigns_ux_completion.md",
        }),
        "concurrent sessions picked the same number",
    ),
    (".handovers", "126"): (
        frozenset({
            "126_mainnet_launch_handover.md",
            "126_plan_005_flow_harness_scaffold.md",
        }),
        "concurrent sessions picked the same number",
    ),
}


def numbered_files(directory: Path) -> dict[str, set[str]]:
    groups: dict[str, set[str]] = defaultdict(set)
    for path in directory.iterdir():
        match = NUMBERED_RE.match(path.name)
        if match:
            groups[match.group(1)].add(path.name)
    return groups


def read_highwater(directory: Path) -> int | None:
    path = directory / HIGHWATER
    if not path.is_file():
        return None
    text = path.read_text().strip()
    return int(text) if text.isdigit() else None


def check(
    root: Path,
    dirs: tuple[str, ...] = DIRS,
    pinned: dict[tuple[str, str], tuple[frozenset[str], str]] = PINNED,
) -> list[str]:
    """Return one message per violation; an empty list means the tree is clean."""
    errors: list[str] = []
    for name in dirs:
        directory = root / name
        if not directory.is_dir():
            errors.append(f"{name}: directory missing")
            continue
        groups = numbered_files(directory)
        for number, files in sorted(groups.items()):
            pin = pinned.get((name, number))
            match (len(files) > 1, pin):
                case (True, None):
                    errors.append(f"{name}/{number}: used by {len(files)} files: {sorted(files)}")
                case (_, (expected, _)) if files != expected:
                    joined = sorted(files - expected)
                    left = sorted(expected - files)
                    errors.append(f"{name}/{number}: pinned set changed; joined={joined} left={left}")
        for (pin_dir, number), _ in pinned.items():
            if pin_dir == name and number not in groups:
                errors.append(f"{name}/{number}: pinned but no file uses it; delete the pin")
        highest = max((int(n) for n in groups), default=0)
        highwater = read_highwater(directory)
        match highwater:
            case None:
                errors.append(f"{name}/{HIGHWATER}: missing or not a number (expected >= {highest:03d})")
            case value if value < highest:
                errors.append(
                    f"{name}/{HIGHWATER}: {value:03d} is below {highest:03d} on disk; "
                    f"allocate with --next {name} instead of picking by eye"
                )
    return errors


def allocate(directory: Path) -> str:
    """Hand out the next number for `directory`. Safe against a concurrent caller."""
    path = directory / HIGHWATER
    path.touch(exist_ok=True)
    with path.open("r+") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        text = handle.read().strip()
        on_disk = max((int(n) for n in numbered_files(directory)), default=0)
        current = max(int(text) if text.isdigit() else 0, on_disk)
        allocated = f"{current + 1:03d}"
        handle.seek(0)
        handle.truncate()
        handle.write(f"{allocated}\n")
    return allocated


def self_test() -> int:
    """Run the check against fixtures that must pass and fixtures that must fail."""
    pin = {(".d", "002"): (frozenset({"002_a.md", "002_b.md"}), "fixture")}

    def scenario(files: list[str], highwater: str | None) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp) / ".d"
            directory.mkdir()
            for file in files:
                (directory / file).write_text("")
            if highwater is not None:
                (directory / HIGHWATER).write_text(f"{highwater}\n")
            return check(Path(tmp), (".d",), pin)

    base = ["001_x.md", "002_a.md", "002_b.md", "003_y.md", "003_y.sql"]
    cases: list[tuple[str, list[str], str | None, bool]] = [
        ("clean tree, pinned group intact, .sql companion ignored", base, "003", True),
        ("highwater ahead of disk (allocated, file not written yet)", base, "005", True),
        ("new unpinned collision", [*base, "003_z.md"], "003", False),
        ("file joins a pinned group", [*base, "002_c.md"], "003", False),
        ("file leaves a pinned group", ["001_x.md", "002_a.md", "003_y.md"], "003", False),
        ("pinned number no longer on disk", ["001_x.md", "003_y.md"], "003", False),
        ("file numbered past the highwater by eye", [*base, "004_w.md"], "003", False),
        ("highwater missing", base, None, False),
        ("highwater not a number", base, "abc", False),
    ]
    failures = 0
    for label, files, highwater, want_clean in cases:
        errors = scenario(files, highwater)
        ok = (not errors) == want_clean
        failures += not ok
        print(f"{'✅' if ok else '❌'} {label}: {'clean' if not errors else errors}")

    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        (directory / "007_x.md").write_text("")
        (directory / HIGHWATER).write_text("004\n")
        got = [allocate(directory), allocate(directory)]
        ok = got == ["008", "009"]
        failures += not ok
        print(f"{'✅' if ok else '❌'} --next takes max(highwater, disk) and never repeats: {got}")

    total = len(cases) + 1
    print(f"\n{'❌' if failures else '✅'} {total - failures}/{total} self-test cases behaved as expected.")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--next", metavar="DIR", choices=DIRS, help="allocate the next number in DIR")
    mode.add_argument("--self-test", action="store_true", help="prove the gate fails on bad fixtures")
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if args.next:
        print(allocate(ROOT / args.next))
        return 0

    errors = check(ROOT)
    for error in errors:
        print(f"❌ {error}")
    if errors:
        print(f"\n{len(errors)} numbering violation(s). Allocate numbers with --next DIR.")
        return 1
    pinned = ", ".join(f"{d}/{n}" for d, n in PINNED)
    print(f"✅ numbering clean across {', '.join(DIRS)} (pinned legacy collisions: {pinned})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
