#!/usr/bin/env python3
"""Fail when CI installs a Rust toolchain other than the one rust-toolchain.toml pins.

Plan 030 §3. Before the pin, CI tracked `stable` while local builds (and so the
wasm `deploy.sh` ships) ran whatever the shell exported: 1.97.1 on 2026-09-24,
one release behind CI. The pin makes one channel the answer everywhere, and this
gate keeps the two places that name it from drifting apart:

  * rust-toolchain.toml must pin an exact `x.y.z` channel (a moving `stable`
    is no pin) and list the wasm32 target both shipped crates build for.
  * Every `toolchain:` in `.github/workflows/*.yml` must equal that channel.
    The one exception is the weekly rot lane, which exists to run `stable`
    and must name nothing else.

Usage:
    python3 scripts/verify/toolchain_pin_gate.py              # check, exit 1 on failure
    python3 scripts/verify/toolchain_pin_gate.py --self-test  # prove the gate can fail
"""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLCHAIN_FILE = "rust-toolchain.toml"
WORKFLOWS = Path(".github/workflows")
ROT_LANE = "toolchain-rot.yml"
ROT_CHANNEL = "stable"
REQUIRED_TARGET = "wasm32-unknown-unknown"
EXACT_CHANNEL_RE = re.compile(r"^\d+\.\d+\.\d+$")
TOOLCHAIN_KEY_RE = re.compile(r"^\s*toolchain:\s*['\"]?([^'\"\s#]+)")


def check(root: Path) -> list[str]:
    errors: list[str] = []
    toolchain_path = root / TOOLCHAIN_FILE
    if not toolchain_path.is_file():
        return [f"{TOOLCHAIN_FILE} is missing"]
    toolchain = tomllib.loads(toolchain_path.read_text()).get("toolchain", {})
    channel = toolchain.get("channel", "")
    if not EXACT_CHANNEL_RE.match(channel):
        errors.append(f"{TOOLCHAIN_FILE}: channel {channel!r} is not an exact x.y.z release")
    if REQUIRED_TARGET not in toolchain.get("targets", []):
        errors.append(f"{TOOLCHAIN_FILE}: targets must list {REQUIRED_TARGET}")

    for workflow in sorted((root / WORKFLOWS).glob("*.yml")):
        want = ROT_CHANNEL if workflow.name == ROT_LANE else channel
        for line_no, line in enumerate(workflow.read_text().splitlines(), start=1):
            match = TOOLCHAIN_KEY_RE.match(line)
            if match is None or match.group(1) == want:
                continue
            errors.append(
                f"{WORKFLOWS / workflow.name}:{line_no}: toolchain {match.group(1)!r}, want {want!r}"
            )
    return errors


def self_test() -> int:
    good_toml = (
        '[toolchain]\nchannel = "1.98.1"\n'
        'targets = ["wasm32-unknown-unknown"]\n'
    )
    pinned_ci = "      - uses: x\n        with:\n          toolchain: 1.98.1\n"
    rot_ci = "          toolchain: stable\n"
    cases: list[tuple[str, str, dict[str, str], bool]] = [
        ("clean tree passes", good_toml, {"ci.yml": pinned_ci, ROT_LANE: rot_ci}, True),
        ("CI on stable fails", good_toml, {"ci.yml": rot_ci}, False),
        ("CI on another release fails", good_toml, {"ci.yml": "  toolchain: '1.97.1'\n"}, False),
        ("rot lane on the pin fails", good_toml, {ROT_LANE: pinned_ci}, False),
        ("moving channel fails", good_toml.replace("1.98.1", "stable"), {"ci.yml": rot_ci}, False),
        ("missing wasm32 target fails", '[toolchain]\nchannel = "1.98.1"\n', {"ci.yml": pinned_ci}, False),
        ("missing toml fails", "", {"ci.yml": pinned_ci}, False),
    ]
    failed = 0
    for name, toml_text, workflows, want_clean in cases:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            if toml_text:
                (root / TOOLCHAIN_FILE).write_text(toml_text)
            (root / WORKFLOWS).mkdir(parents=True)
            for file_name, body in workflows.items():
                (root / WORKFLOWS / file_name).write_text(body)
            clean = not check(root)
        status = "ok" if clean == want_clean else "FAIL"
        failed += status == "FAIL"
        print(f"  [{status}] {name}")
    if failed:
        print(f"❌ self-test: {failed} case(s) wrong")
        return 1
    print(f"✅ self-test: {len(cases)} cases")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    errors = check(ROOT)
    if errors:
        print("❌ toolchain pin drift:")
        for error in errors:
            print(f"  {error}")
        return 1
    print("✅ CI toolchains match rust-toolchain.toml")
    return 0


if __name__ == "__main__":
    sys.exit(main())
