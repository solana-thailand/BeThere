#!/usr/bin/env python3
"""landing_photos_upload.py: put the landing photos into R2, checked both ways.

    python3 scripts/landing_photos_upload.py <photo-dir> staging|prod

The list is `worker/landing-photos.jsonl` (one photo per line, with the sha256
of the full-size file and of its thumbnail). Every file is checked against the
list BEFORE upload, uploaded under `landing-photos/`, then read back from R2
and checked again. Nothing in git: the images stay out of this public repo
(.plans/043 L9). The files must already have their metadata stripped; this
script never re-encodes them.

Taking a photo down: delete its line in the list, then

    npx wrangler r2 object delete <bucket>/landing-photos/<file> --remote

for the file and its thumbnail (buckets: bethere-assets, bethere-assets-staging).

Exit 0 when every object is in R2 with the listed hash; 1 otherwise.
"""
from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIST = ROOT / "worker" / "landing-photos.jsonl"
BUCKETS = {"staging": "bethere-assets-staging", "prod": "bethere-assets"}
PREFIX = "landing-photos/"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def wrangler(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["npx", "wrangler", "r2", "object", *args, "--remote"],
        cwd=ROOT / "worker",
        capture_output=True,
        check=False,
    )


def main() -> int:
    if len(sys.argv) != 3 or sys.argv[2] not in BUCKETS:
        print(__doc__, file=sys.stderr)
        return 1
    src, bucket = Path(sys.argv[1]), BUCKETS[sys.argv[2]]
    objects = []
    for line in LIST.read_text(encoding="utf-8").splitlines():
        row = json.loads(line)
        objects += [(row["file"], row["sha256"]), (row["thumb"], row["thumb_sha256"])]
    for name, want in objects:
        local = (src / name).read_bytes()
        if sha256(local) != want:
            print(f"❌ {name}: local file does not match the list", file=sys.stderr)
            return 1
    for name, want in objects:
        key = f"{bucket}/{PREFIX}{name}"
        put = wrangler("put", key, "--file", str(src / name), "--content-type", "image/jpeg")
        if put.returncode != 0:
            print(f"❌ {name}: upload failed: {put.stderr.decode()[-300:]}", file=sys.stderr)
            return 1
        got = wrangler("get", key, "--pipe")
        if got.returncode != 0 or sha256(got.stdout) != want:
            print(f"❌ {name}: read back from R2 does not match the list", file=sys.stderr)
            return 1
        print(f"✅ {key} ({len(got.stdout):,} B, sha256 {want[:12]}…)")
    print(f"✅ {len(objects)} objects in {bucket}/{PREFIX}, each read back with its listed sha256")
    return 0


if __name__ == "__main__":
    sys.exit(main())
