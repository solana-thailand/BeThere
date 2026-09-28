"""Run one read-only SELECT on the remote D1 through wrangler (shared by the reports)."""

import json
import subprocess
from pathlib import Path

WORKER = Path(__file__).resolve().parents[2] / "worker"


def select(sql: str, staging: bool) -> list[dict]:
    """Rows of `sql` from prod (or staging) D1. Raises RuntimeError on failure."""
    db = "bethere-db-staging" if staging else "bethere-db"
    cmd = ["npx", "wrangler", "d1", "execute", db, "--remote", "--json", "--command", " ".join(sql.split())]
    if staging:
        cmd += ["--env", "staging"]
    out = subprocess.run(cmd, cwd=WORKER, capture_output=True, text=True)
    start = out.stdout.find("[")
    if out.returncode != 0 or start < 0:
        raise RuntimeError(f"wrangler d1 execute failed: {out.stderr.strip()[-300:]}")
    return json.loads(out.stdout[start:])[0]["results"]
