#!/usr/bin/env python3
"""globe_data_import.py: the landing globe's data, from the owner's generator.

    python3 scripts/globe_data_import.py [path/to/globe-data.js]

The source is `bethere-ux/globe-data.js` in solana-thailand-devrel-helper,
written by `bethere-ux/globe_data.py` (three Luma calendars, de-duplicated, plus
our completed events; .plans/043 L10). This script does not fetch or count
anything itself: it checks the shape, drops what the page does not draw, and
writes `frontend-leptos/globe/globe-data.json`, which the landing loads only
when the goal section scrolls near (frontend-leptos/globe/globe.js).

Kept: measured_at, window_days, source, solana_events (the goal bar's
denominator), home, the 2-degree event cells, the land dots, and per country
its name, centre and events as [name, date, city, upcoming]. Dropped: the Luma
ids (the page links nowhere by id) and the file's own `ours`. The numerator
comes from GET /api/public/stats at view time (build plan rule 1), not from a
copy frozen in this file.

Exit 0 on success, 1 on a missing file or a shape the page cannot draw.
"""
from __future__ import annotations

import json
import re
import sys
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SRC = Path.home() / "solana-thailand-devrel-helper" / "bethere-ux" / "globe-data.js"
OUT = ROOT / "frontend-leptos" / "globe" / "globe-data.json"
PREFIX = re.compile(r"^\s*(?://[^\n]*\n\s*)*window\.GLOBE\s*=\s*", re.S)


def fail(msg: str) -> None:
    print(f"❌ {msg}", file=sys.stderr)
    sys.exit(1)


def load(src: Path) -> dict:
    if not src.is_file():
        fail(f"{src} not found")
    text = src.read_text(encoding="utf-8")
    m = PREFIX.match(text)
    if not m:
        fail(f"{src}: expected `window.GLOBE = {{…}};`")
    return json.loads(text[m.end():].strip().rstrip(";"))


def is_point(p: object) -> bool:
    return (
        isinstance(p, list)
        and len(p) >= 2
        and all(isinstance(v, (int, float)) for v in p[:2])
        and -90 <= p[0] <= 90
        and -180 <= p[1] <= 180
    )


def slim(d: dict) -> dict:
    for key in ("measured_at", "window_days", "source", "solana_events", "home", "events", "countries", "land"):
        if key not in d:
            fail(f"missing key {key!r}")
    date.fromisoformat(d["measured_at"])
    if not (isinstance(d["solana_events"], int) and d["solana_events"] > 0):
        fail("solana_events must be a positive integer")
    if not is_point(d["home"]):
        fail("home is not a [lat, lng] point")
    for cell in d["events"]:
        if not (is_point(cell) and len(cell) == 3 and isinstance(cell[2], int) and cell[2] > 0):
            fail(f"bad event cell {cell!r}")
    for dot in d["land"]:
        if not is_point(dot):
            fail(f"bad land dot {dot!r}")
    countries = {}
    for code, c in d["countries"].items():
        events = []
        for e in c["events"]:
            if not (isinstance(e, list) and len(e) == 5):
                fail(f"{code}: event row is not [name, date, city, id, upcoming]: {e!r}")
            date.fromisoformat(e[1])
            events.append([e[0], e[1], e[2], 1 if e[4] else 0])
        has_centre = c.get("lat") is not None and c.get("lng") is not None
        if has_centre and not is_point([c["lat"], c["lng"]]):
            fail(f"{code}: bad centre")
        countries[code] = {
            "name": c["name"],
            "lat": c["lat"] if has_centre else None,
            "lng": c["lng"] if has_centre else None,
            "events": events,
        }
    return {
        "measured_at": d["measured_at"],
        "window_days": d["window_days"],
        "source": d["source"],
        "solana_events": d["solana_events"],
        "home": d["home"][:2],
        "events": d["events"],
        "countries": countries,
        "land": [p[:2] for p in d["land"]],
    }


def main() -> None:
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_SRC
    out = slim(load(src))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(out, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
    n_events = sum(len(c["events"]) for c in out["countries"].values())
    print(
        f"✅ {OUT.relative_to(ROOT)}: measured {out['measured_at']}, {out['solana_events']} Solana events, "
        f"{len(out['countries'])} countries, {n_events} named events, {OUT.stat().st_size:,} B"
    )


if __name__ == "__main__":
    main()
