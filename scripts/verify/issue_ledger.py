#!/usr/bin/env python3
"""Check what each `.issues/*.md` Status line claims against git.

Issue 140. An issue's Status is prose written once, at the moment it was true.
"fixed, not deployed" stays on the page after the next deploy ships the fix;
"Branch: X (unpushed)" stays after X is merged and deleted. The next session
reads the prose, trusts it, and plans on it. This script gives each claim a
verdict so a stale claim gets flagged before anyone builds on it.

What it can check mechanically:
  * every commit cited in the Status block exists and is on HEAD
  * every branch cited there still exists
  * "not deployed" vs. the newest `deploy/production/*` tag (worker/deploy.sh
    writes one per deploy since 2026-09-23 — before that, deploy state is
    UNKNOWN, not "not deployed")

What it cannot: whether the behaviour is still fixed. That needs the issue's own
reproduction. This script only narrows what to re-verify first.

Usage:
    python3 scripts/verify/issue_ledger.py            # flagged issues only
    python3 scripts/verify/issue_ledger.py --all      # every issue
    python3 scripts/verify/issue_ledger.py --strict   # exit 1 on any flag
    python3 scripts/verify/issue_ledger.py --prod-ref <commit>   # what-if

Read-only.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ISSUES = ROOT / ".issues"

STATUS_RE = re.compile(r"^\W*status\W*:?", re.IGNORECASE)
SHA_RE = re.compile(r"`([0-9a-f]{7,40})`")
BRANCH_RE = re.compile(r"`((?:feature|hotfix|release|fix)/[\w./-]+)`")
# How commit messages name an issue: `.issues/138`, `#138`, `docs(138)`, `issue 138`.
LINK_RE = re.compile(r"(?:\.issues/|#|\(|\b[Ii]ssue )(\d{3})\b")
# Doc-only commits land after the deploy they describe; they are not the fix.
CODE_PATHS = (".", ":!.issues", ":!.plans", ":!.handovers", ":!docs", ":!*.md")


class Claim(Enum):
    OPEN = "open"
    FIXED_UNDEPLOYED = "fixed, not deployed"
    DEPLOYED = "deployed"
    FIXED = "fixed"
    NONE = "no status"
    OTHER = "unclassified"


class Flag(Enum):
    STALE_UNDEPLOYED = "says not deployed, but every cited commit is in the newest prod tag"
    DEPLOY_CONTRADICTED = "says deployed, but a cited commit is not in the newest prod tag"
    NOT_ON_HEAD = "a cited fix commit is not on HEAD"
    BRANCH_GONE = "a cited branch no longer exists"
    UNVERIFIABLE = "claims a fix, but no commit is cited in Status or names the issue"


@dataclass
class Verdict:
    path: Path
    claim: Claim
    commits: list[str] = field(default_factory=list)
    linked: list[str] = field(default_factory=list)
    flags: list[Flag] = field(default_factory=list)


def git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True)


def resolve_commit(ref: str) -> str | None:
    out = git("rev-parse", "--verify", "-q", f"{ref}^{{commit}}")
    return out.stdout.strip() if out.returncode == 0 else None


def is_ancestor(commit: str, of: str) -> bool:
    return git("merge-base", "--is-ancestor", commit, of).returncode == 0


def newest_prod_tag() -> str | None:
    out = git("tag", "-l", "deploy/production/*", "--sort=-creatordate")
    tags = out.stdout.split()
    return tags[0] if tags else None


def commits_by_issue() -> dict[str, list[str]]:
    """Code-touching commits whose message names an issue, keyed by number."""
    out = git("log", "--format=%H%x00%s%n%b%x01", "--", *CODE_PATHS)
    index: dict[str, list[str]] = {}
    for entry in out.stdout.split("\x01"):
        sha, _, msg = entry.strip().partition("\x00")
        for number in set(LINK_RE.findall(msg)):
            index.setdefault(number, []).append(sha)
    return index


def status_block(text: str) -> str:
    lines = text.splitlines()[:40]
    for i, line in enumerate(lines):
        if STATUS_RE.match(line):
            block = [line]
            for nxt in lines[i + 1 : i + 12]:
                if not nxt.strip() or nxt.startswith("#"):
                    break
                block.append(nxt)
            return "\n".join(block)
    return ""


def classify(block: str) -> Claim:
    low = block.lower()
    match low:
        case "":
            return Claim.NONE
        case _ if "not deployed" in low or "undeployed" in low:
            return Claim.FIXED_UNDEPLOYED
        case _ if "deployed" in low:
            return Claim.DEPLOYED
        case _ if re.search(r"\b(fixed|implemented|resolved|shipped|built|closed)\b", low):
            return Claim.FIXED
        case _ if "open" in low:
            return Claim.OPEN
        case _:
            return Claim.OTHER


def judge(path: Path, prod: str | None, links: dict[str, list[str]]) -> Verdict:
    block = status_block(path.read_text(encoding="utf-8"))
    verdict = Verdict(path, classify(block))
    # 8-hex ids are also Cloudflare version ids; only what git resolves counts.
    verdict.commits = [c for c in (resolve_commit(s) for s in SHA_RE.findall(block)) if c]
    verdict.linked = [c for c in links.get(path.name[:3], []) if c not in verdict.commits]
    evidence = verdict.commits + verdict.linked
    claims_fix = verdict.claim in (Claim.FIXED_UNDEPLOYED, Claim.DEPLOYED, Claim.FIXED)

    if claims_fix and not evidence:
        verdict.flags.append(Flag.UNVERIFIABLE)
    if any(not is_ancestor(c, "HEAD") for c in verdict.commits):
        verdict.flags.append(Flag.NOT_ON_HEAD)
    for branch in BRANCH_RE.findall(block):
        if git("rev-parse", "--verify", "-q", f"refs/heads/{branch}").returncode != 0:
            verdict.flags.append(Flag.BRANCH_GONE)
            break
    if prod and evidence:
        shipped = all(is_ancestor(c, prod) for c in evidence)
        match verdict.claim:
            case Claim.FIXED_UNDEPLOYED if shipped:
                verdict.flags.append(Flag.STALE_UNDEPLOYED)
            case Claim.DEPLOYED if not shipped:
                verdict.flags.append(Flag.DEPLOY_CONTRADICTED)
            case _:
                pass
    return verdict


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--all", action="store_true", help="print every issue, not only flagged ones")
    parser.add_argument("--strict", action="store_true", help="exit 1 if any issue is flagged")
    parser.add_argument("--prod-ref", help="treat this commit as prod (what-if; overrides the newest deploy tag)")
    args = parser.parse_args()

    prod = args.prod_ref or newest_prod_tag()
    links = commits_by_issue()
    verdicts = [judge(p, prod, links) for p in sorted(ISSUES.glob("[0-9][0-9][0-9]_*.md"))]

    print(f"prod reference: {prod or 'NONE — no deploy/production/* tag yet; deploy state is UNKNOWN'}")
    counts: dict[Claim, int] = {}
    for v in verdicts:
        counts[v.claim] = counts.get(v.claim, 0) + 1
    print("claims: " + ", ".join(f"{c.value}={n}" for c, n in sorted(counts.items(), key=lambda kv: -kv[1])))

    flagged = [v for v in verdicts if v.flags]
    for v in verdicts if args.all else flagged:
        cited = " ".join(c[:8] for c in v.commits) or "-"
        print(f"\n{v.path.name}\n  claim: {v.claim.value}  cited: {cited}  linked by message: {len(v.linked)}")
        for f in v.flags:
            print(f"  ⚠ {f.name}: {f.value}")

    by_flag: dict[Flag, int] = {}
    for v in flagged:
        for f in v.flags:
            by_flag[f] = by_flag.get(f, 0) + 1
    print(f"\n{len(flagged)}/{len(verdicts)} issues flagged" + "".join(f"\n  {f.name}: {n}" for f, n in by_flag.items()))
    return 1 if args.strict and flagged else 0


if __name__ == "__main__":
    sys.exit(main())
