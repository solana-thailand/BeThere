#!/usr/bin/env python3
"""
diag_kv_usage.py — Diagnose Cloudflare Workers KV daily quota consumption.

Answers: "which KV quota bucket (reads vs writes+deletes+lists) is being burned,
by how much, and is it on track to hit the free-tier ceiling before reset?"

This script does NOT consume KV operations. It reads usage counters from the
Cloudflare GraphQL Analytics API (read-only, analytics endpoint), so running it
repeatedly through Demo Day is free and safe.

Data sources (all REAL — no fabricated numbers):
  - OAuth token: read from wrangler's local config (`npx wrangler login` first).
  - Usage data:  Cloudflare Analytics API, last 3 calendar days, account-wide.

Schema robustness:
  - The exact Workers KV analytics dataset name and field names change over time
    (e.g. cloudflareWorkersKvOperationsAdaptive, ...KvmRequestsAdaptive, ...).
  - This script INTROSPECTS the live schema first, discovers the dataset name,
    its `sum` fields, and its `dimensions`, then issues the actual query with
    only valid field names. This makes it resilient to Cloudflare schema drift.

Free tier (Workers free plan) — the relevant ceiling:
  - 100,000 read ops / day
  -   1,000 write + delete + list ops / day   ← the tight bucket

Usage:
  python3 scripts/diag_kv_usage.py            # last 3 days
  python3 scripts/diag_kv_usage.py --days 7   # custom range

Exit codes:
  0 = ran successfully (even if usage is high)
  2 = hard failure (no token, schema unreadable, API error)
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request
from datetime import datetime, timedelta, timezone

# ---------------------------------------------------------------------------
# Configuration — resolved at import time, no shell interpolation at runtime.
# Values are facts about THIS project's Cloudflare deployment (see wrangler.toml).
# ---------------------------------------------------------------------------
ACCOUNT_ID = "bb8f9ffa91e24d9ce850cbbc4fd45935"
EVENTS_KV_NS = "c8a6a87f9ed34ce0a3c8e48b84039214"

GRAPHQL_URL = "https://api.cloudflare.com/client/v4/graphql"

# Cloudflare Workers KV free-tier daily limits.
LIMIT_READS_PER_DAY = 100_000
LIMIT_WDLS_PER_DAY = 1_000  # writes + deletes + lists share ONE bucket

WRANGLER_CONFIG_PATHS = [
    os.path.expanduser("~/Library/Preferences/.wrangler/config/default.toml"),
    os.path.expanduser("~/.wrangler/config/default.toml"),
    os.path.expanduser("~/.config/.wrangler/config/default.toml"),
]


# ---------------------------------------------------------------------------
# Token + HTTP
# ---------------------------------------------------------------------------


def load_oauth_token() -> str:
    """Read wrangler's OAuth token from its local config file."""
    for path in WRANGLER_CONFIG_PATHS:
        if not os.path.isfile(path):
            continue
        with open(path, "r", encoding="utf-8") as f:
            text = f.read()
        m = re.search(r'oauth_token\s*=\s*"([^"]+)"', text)
        if m:
            return m.group(1)
    raise SystemExit(
        "ERROR: no wrangler OAuth token found.\n"
        "Run `npx wrangler login` first, then re-run this script."
    )


def graphql(token: str, query: str, variables: dict | None = None) -> dict:
    """POST a GraphQL request to the Cloudflare Analytics API."""
    body = json.dumps({"query": query, "variables": variables or {}}).encode("utf-8")
    req = urllib.request.Request(
        GRAPHQL_URL,
        data=body,
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            payload = json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", errors="replace")
        raise SystemExit(f"HTTP {e.code} from GraphQL API:\n{detail}")
    if payload.get("errors"):
        raise SystemExit("GraphQL errors:\n" + json.dumps(payload["errors"], indent=2))
    return payload.get("data", {})


# ---------------------------------------------------------------------------
# Schema introspection — find the dataset + fields WITHOUT hardcoding names.
# ---------------------------------------------------------------------------


def _named_type(t: dict | None) -> str | None:
    """Unwrap NON_NULL / LIST wrappers to the innermost named type."""
    while t and not t.get("name") and t.get("ofType"):
        t = t["ofType"]
    return t.get("name") if t else None


def _field_map(token: str, type_name: str) -> dict:
    """Return {field_name: {kind, named_type}} for a given GraphQL type."""
    # Unwrap NON_NULL / LIST wrappers by nesting ofType three levels deep.
    # One token per line so the brace structure is auditable and stays balanced.
    q = (
        "{\n"
        f'  __type(name: "{type_name}") {{\n'
        "    fields {\n"
        "      name\n"
        "      type {\n"
        "        name\n"
        "        kind\n"
        "        ofType {\n"
        "          name\n"
        "          kind\n"
        "          ofType {\n"
        "            name\n"
        "            kind\n"
        "            ofType {\n"
        "              name\n"
        "            }\n"
        "          }\n"
        "        }\n"
        "      }\n"
        "    }\n"
        "  }\n"
        "}\n"
    )
    data = graphql(token, q)
    t = data.get("__type") or {}
    out = {}
    for f in t.get("fields", []) or []:
        out[f["name"]] = {
            "kind": f["type"].get("kind"),
            "named_type": _named_type(f["type"]),
        }
    return out


def discover_kv_schema(token: str) -> tuple[str | None, list[str], list[str]]:
    """
    Walk viewer -> accounts -> <dataset> to find the KV analytics dataset.

    Returns (dataset_name, sum_fields, dimension_fields).
    Returns (None, [], []) if the schema has no KV dataset we can read.
    """
    # 1) viewer type
    q_root = "{ __schema { queryType { fields { name type { name kind } } } } }"
    root = graphql(token, q_root)
    viewer_type = None
    for f in root["__schema"]["queryType"]["fields"]:
        if f["name"] == "viewer":
            viewer_type = f["type"].get("name") or _named_type(f["type"])
            break
    if not viewer_type:
        return None, [], []

    # 2) accounts field -> Account type
    viewer_fields = _field_map(token, viewer_type)
    if "accounts" not in viewer_fields:
        return None, [], []
    account_type = viewer_fields["accounts"]["named_type"]
    if not account_type:
        return None, [], []

    # 3) find KV-ish dataset on the Account type
    account_fields = _field_map(token, account_type)
    kv_candidates: list[str] = []
    for name, meta in account_fields.items():
        lname = name.lower()
        if "kv" in lname and ("workers" in lname or "adaptive" in lname):
            kv_candidates.append(name)
    if not kv_candidates:
        # looser fallback
        kv_candidates = [n for n in account_fields if "kv" in n.lower()]
    if not kv_candidates:
        return None, [], []

    # Prefer datasets that look like operations/requests adaptive groups.
    chosen = None
    for preference in (
        "operationsadaptive",
        "requestsadaptive",
        "usageadaptive",
        "adaptive",
    ):
        for c in kv_candidates:
            if preference in c.lower():
                chosen = c
                break
        if chosen:
            break
    if not chosen:
        chosen = kv_candidates[0]

    # 4) inspect the dataset's sum + dimensions fields.
    # `chosen` is a FIELD NAME on the Account type — resolve its actual type
    # before introspecting, otherwise we query a non-existent type name and
    # get back an empty field list.
    ds_type = account_fields[chosen]["named_type"]
    if not ds_type:
        # Fall back: some schemas name the element type after the field.
        ds_type = chosen
    ds_fields = _field_map(token, ds_type)
    sum_fields: list[str] = []
    dim_fields: list[str] = []
    sum_type = ds_fields.get("sum", {}).get("named_type")
    dims_type = ds_fields.get("dimensions", {}).get("named_type")
    if sum_type:
        sum_fields = list(_field_map(token, sum_type).keys())
    if dims_type:
        dim_fields = list(_field_map(token, dims_type).keys())

    return chosen, sum_fields, dim_fields


# ---------------------------------------------------------------------------
# Actual usage query
# ---------------------------------------------------------------------------


def query_kv_usage(
    token: str,
    dataset: str,
    sum_fields: list[str],
    dim_fields: list[str],
    days: int,
) -> list[dict]:
    """Query the discovered KV dataset for the last N calendar days."""
    today = datetime.now(timezone.utc).date()
    start = today - timedelta(days=days - 1)

    # Request date + actionType dimensions. `actionType` splits read/write/delete/
    # list, which is how we map rows onto the right free-tier quota bucket.
    requested_dims = []
    for d in dim_fields:
        if d.lower() in ("date", "actiontype"):
            requested_dims.append(d)
    if "date" not in [d.lower() for d in requested_dims]:
        # Most adaptive datasets expose `date` even if introspection was incomplete.
        requested_dims.append("date")
    dims_block = "\n".join(f"            {d}" for d in requested_dims)

    # Sum fields discovered via introspection. Fallback to `requests` — the
    # canonical counter on Cloudflare adaptive-group datasets.
    sum_block = (
        "\n".join(f"            {f}" for f in sum_fields)
        if sum_fields
        else "            requests"
    )

    q = (
        "query KvUsage($account: String!, $start: Date!, $end: Date!) {\n"
        "  viewer {\n"
        "    accounts(filter: { accountTag: $account }) {\n"
        f"      {dataset}(\n"
        "        filter: { date_geq: $start, date_leq: $end }\n"
        "        limit: 1000\n"
        "      ) {\n"
        f"        sum {{\n{sum_block}\n        }}\n"
        f"        dimensions {{\n{dims_block}\n        }}\n"
        "      }\n"
        "    }\n"
        "  }\n"
        "}\n"
    )
    data = graphql(
        token,
        q,
        {
            "account": ACCOUNT_ID,
            "start": start.isoformat(),
            "end": today.isoformat(),
        },
    )
    accounts = data.get("viewer", {}).get("accounts", []) or []
    if not accounts:
        return []
    return accounts[0].get(dataset, []) or []


# ---------------------------------------------------------------------------
# Aggregation + presentation
# ---------------------------------------------------------------------------


def _to_int(v) -> int:
    if v is None:
        return 0
    try:
        return int(v)
    except (TypeError, ValueError):
        return 0


def classify_field(name: str) -> str:
    """Map a sum-field name to a quota bucket."""
    n = name.lower()
    if "read" in n:
        return "reads"
    if "write" in n:
        return "writes"
    if "delete" in n:
        return "deletes"
    if "list" in n:
        return "lists"
    return "other"


def classify_action(action_type: str) -> str:
    """Map a Cloudflare KV actionType value to a quota bucket."""
    n = (action_type or "").lower()
    if "read" in n or "get" in n:
        return "reads"
    if "write" in n or "put" in n:
        return "writes"
    if "delete" in n:
        return "deletes"
    if "list" in n:
        return "lists"
    return "other"


def aggregate(rows: list[dict], sum_fields: list[str]) -> dict[str, dict[str, int]]:
    """
    Aggregate rows into {date: {reads, writes, deletes, lists, other}}.

    The `kvOperationsAdaptiveGroups` dataset exposes a single `requests` counter
    per row and splits operation type via the `actionType` dimension. So we
    bucket `sum.requests` by `dimensions.actionType`, not by sum-field name
    (the only sum fields are `requests` and `objectBytes`, neither of which
    encodes the operation type).
    """
    by_date: dict[str, dict[str, int]] = {}
    for row in rows:
        dims = row.get("dimensions", {}) or {}
        date = dims.get("date") or dims.get("timeslot") or "unknown"
        date = str(date)[:10]
        agg = by_date.setdefault(
            date,
            {"reads": 0, "writes": 0, "deletes": 0, "lists": 0, "other": 0},
        )
        bucket = classify_action(dims.get("actionType", ""))
        s = row.get("sum", {}) or {}
        # Prefer the canonical `requests` counter; fall back to first sum field
        # for schemas where the counter is named differently.
        count = _to_int(s.get("requests"))
        if count == 0 and sum_fields:
            count = _to_int(s.get(sum_fields[0]))
        agg[bucket] += count
    return by_date


def print_report(by_date: dict[str, dict[str, int]]) -> None:
    today = datetime.now(timezone.utc).date().isoformat()

    header = (
        f"{'Date (UTC)':<12} {'Reads':>12} {'Writes':>9} {'Deletes':>9} "
        f"{'Lists':>9} {'W+D+L':>11} {'% W+D+L':>9} {'% Reads':>9}"
    )
    print(header)
    print("-" * len(header))

    for date in sorted(by_date.keys(), reverse=True):
        a = by_date[date]
        wdl = a["writes"] + a["deletes"] + a["lists"]
        wdl_pct = (wdl / LIMIT_WDLS_PER_DAY) * 100
        reads_pct = (a["reads"] / LIMIT_READS_PER_DAY) * 100

        wdl_flag = ""
        if wdl_pct >= 80:
            wdl_flag = " !"
        elif wdl_pct >= 50:
            wdl_flag = " ."

        reads_flag = " !" if reads_pct >= 80 else (" ." if reads_pct >= 50 else "")

        print(
            f"{date:<12} {a['reads']:>12,} {a['writes']:>9,} {a['deletes']:>9,} "
            f"{a['lists']:>9,} {wdl:>11,} {wdl_pct:>8.1f}%{wdl_flag} "
            f"{reads_pct:>8.1f}%{reads_flag}"
        )

    print()
    print("Legend:  W+D+L = writes + deletes + lists (shared 1,000/day free bucket)")
    print("         ' !' = >=80% of daily limit (danger zone)")
    print("         ' .' = >=50% of daily limit (the warning threshold)")

    print()
    if today in by_date:
        a = by_date[today]
        wdl = a["writes"] + a["deletes"] + a["lists"]
        print(f"Today ({today}):")
        print(
            f"  Reads:        {a['reads']:>8,} / {LIMIT_READS_PER_DAY:,}  "
            f"({a['reads'] / LIMIT_READS_PER_DAY * 100:.1f}%)  "
            f"headroom {max(0, LIMIT_READS_PER_DAY - a['reads']):,}"
        )
        print(
            f"  W+D+L:        {wdl:>8,} / {LIMIT_WDLS_PER_DAY:,}  "
            f"({wdl / LIMIT_WDLS_PER_DAY * 100:.1f}%)  "
            f"headroom {max(0, LIMIT_WDLS_PER_DAY - wdl):,}"
        )
    else:
        print(f"Today ({today}): no usage recorded yet.")


# ---------------------------------------------------------------------------
# Detail mode — hourly + per-action breakdown for a single date
# ---------------------------------------------------------------------------


def query_detail(
    token: str,
    dataset: str,
    sum_fields: list[str],
    dim_fields: list[str],
    date_iso: str,
) -> list[dict]:
    """
    Query one date at hourly + actionType + namespaceId granularity.

    This is what tells us whether writes are spread evenly (per-request
    application traffic) or bursty (cron / scheduled job). The daily cleanup
    cron runs at 03:00 UTC, so a write/list spike at that hour implicates it.
    """
    # Only request dimensions that actually exist on the discovered dataset.
    wanted = ("datetimehour", "actiontype", "namespaceid")
    requested_dims = [d for d in dim_fields if d.lower() in wanted]
    if not any(d.lower() == "datetimehour" for d in requested_dims):
        # No hourly dimension available — fall back to the date dimension so
        # the query still returns at least per-action totals for the day.
        requested_dims = [d for d in dim_fields if d.lower() in ("date", "actiontype")]
    dims_block = "\n".join(f"            {d}" for d in requested_dims)

    sum_block = (
        "\n".join(f"            {f}" for f in sum_fields)
        if sum_fields
        else "            requests"
    )

    q = (
        "query KvDetail($account: String!, $date: Date!) {\n"
        "  viewer {\n"
        "    accounts(filter: { accountTag: $account }) {\n"
        f"      {dataset}(\n"
        "        filter: { date_geq: $date, date_leq: $date }\n"
        "        limit: 1000\n"
        "      ) {\n"
        f"        sum {{\n{sum_block}\n        }}\n"
        f"        dimensions {{\n{dims_block}\n        }}\n"
        "      }\n"
        "    }\n"
        "  }\n"
        "}\n"
    )
    data = graphql(token, q, {"account": ACCOUNT_ID, "date": date_iso})
    accounts = data.get("viewer", {}).get("accounts", []) or []
    if not accounts:
        return []
    return accounts[0].get(dataset, []) or []


def print_detail(rows: list[dict], date_iso: str) -> None:
    """Print hourly × action matrix + per-action totals for one date."""
    # Collapse each row into (hour, action, namespace) -> requests
    by_hour: dict[str, dict[str, int]] = {}
    per_action: dict[str, int] = {}
    namespaces: dict[str, int] = {}
    for row in rows:
        dims = row.get("dimensions", {}) or {}
        hour = str(dims.get("datetimeHour") or dims.get("date") or "?")
        hour = hour[:13]  # trim to "YYYY-MM-DD HH" if full datetime
        action = str(dims.get("actionType") or "unknown").lower()
        ns = str(dims.get("namespaceId") or "unknown")
        count = _to_int((row.get("sum") or {}).get("requests"))
        by_hour.setdefault(
            hour, {"reads": 0, "writes": 0, "deletes": 0, "lists": 0, "other": 0}
        )
        bucket = classify_action(action)
        by_hour[hour][bucket] += count
        per_action[action] = per_action.get(action, 0) + count
        namespaces[ns] = namespaces.get(ns, 0) + count

    print(f"Hourly breakdown for {date_iso} (UTC):")
    print()
    header = f"{'Hour':<17} {'Reads':>9} {'Writes':>9} {'Deletes':>9} {'Lists':>9} {'W+D+L':>9}"
    print(header)
    print("-" * len(header))
    for hour in sorted(by_hour.keys()):
        a = by_hour[hour]
        wdl = a["writes"] + a["deletes"] + a["lists"]
        marker = ""
        if hour.endswith("03") and wdl > 0:
            marker = "  <- 03:00 UTC (cleanup cron window)"
        print(
            f"{hour:<17} {a['reads']:>9,} {a['writes']:>9,} {a['deletes']:>9,} "
            f"{a['lists']:>9,} {wdl:>9,}{marker}"
        )

    print()
    print("Per-action totals:")
    for action in sorted(per_action.keys()):
        print(f"  {action:<12} {per_action[action]:>10,}")

    print()
    print("Per-namespace totals:")
    for ns in sorted(namespaces.keys()):
        label = "EVENTS" if ns == EVENTS_KV_NS else ns
        print(f"  {label:<40} {namespaces[ns]:>10,}")

    # Pattern verdict — steady vs bursty is the actionable signal.
    print()
    write_hours = [(h, a["writes"]) for h, a in by_hour.items() if a["writes"] > 0]
    if write_hours:
        total_writes = sum(w for _, w in write_hours)
        peak_hour, peak_writes = max(write_hours, key=lambda x: x[1])
        peak_share = peak_writes / total_writes if total_writes else 0
        print("Write pattern:")
        print(f"  total write ops:  {total_writes:,} across {len(write_hours)} hour(s)")
        print(
            f"  peak hour:        {peak_hour} ({peak_writes:,} writes = {peak_share:.0%} of total)"
        )
        if peak_share >= 0.5:
            print(
                f"  verdict:          BURSTY — >=50% of writes concentrated in one hour."
            )
            print(
                f"                     Likely a scheduled/cron job (cleanup runs 03:00 UTC)."
            )
        else:
            print(f"  verdict:          STEADY — writes spread across many hours.")
            print(
                f"                     Likely per-request application traffic (dual-writes /"
            )
            print(f"                     KV write-back in resolve_event_or_fallback).")


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Diagnose Cloudflare Workers KV quota usage."
    )
    parser.add_argument(
        "--days",
        type=int,
        default=3,
        help="Number of trailing calendar days to report (default: 3).",
    )
    parser.add_argument(
        "--detail",
        type=str,
        default=None,
        metavar="YYYY-MM-DD",
        help="Show hourly + per-action + per-namespace breakdown for one date.",
    )
    args = parser.parse_args()
    if args.days < 1:
        print("--days must be >= 1", file=sys.stderr)
        return 2
    if args.detail:
        try:
            datetime.strptime(args.detail, "%Y-%m-%d").date()
        except ValueError:
            print("--detail must be YYYY-MM-DD", file=sys.stderr)
            return 2

    print("=" * 78)
    print("BeThere — Cloudflare Workers KV usage diagnostic (read-only)")
    print("=" * 78)
    print(f"Account:         {ACCOUNT_ID}")
    print(f"EVENTS namespace:{EVENTS_KV_NS}")
    print(
        f"Free-tier limit: {LIMIT_READS_PER_DAY:,} reads/day, "
        f"{LIMIT_WDLS_PER_DAY:,} writes+deletes+lists/day"
    )
    print(
        "Note:            This script hits the Analytics API only — "
        "it does NOT consume KV ops."
    )
    print()

    token = load_oauth_token()

    print("[1/3] Discovering KV analytics dataset via schema introspection...")
    dataset, sum_fields, dim_fields = discover_kv_schema(token)
    if not dataset:
        print("  ERROR: no KV dataset found in the Cloudflare analytics schema.")
        print("  The schema may have changed. Check the dashboard manually:")
        print("    https://dash.cloudflare.com -> Workers & Pages -> KV -> Usage")
        return 2
    print(f"  dataset:    {dataset}")
    print(f"  sum fields: {sum_fields}")
    print(f"  dimensions: {dim_fields}")
    print()

    if args.detail:
        print(f"[2/2] Querying hourly detail for {args.detail}...")
        rows = query_detail(token, dataset, sum_fields, dim_fields, args.detail)
        if not rows:
            print("  No rows returned for that date (analytics delay or no usage).")
            return 0
        print()
        print_detail(rows, args.detail)
        return 0

    print(f"[2/3] Querying usage for the last {args.days} day(s)...")
    rows = query_kv_usage(token, dataset, sum_fields, dim_fields, args.days)
    if not rows:
        print("  No KV usage rows returned (analytics delay or namespace idle).")
        return 0
    by_date = aggregate(rows, sum_fields)
    print()

    print("[3/3] Daily breakdown vs free-tier limits:")
    print()
    print_report(by_date)
    return 0


if __name__ == "__main__":
    sys.exit(main())
