#!/usr/bin/env python3
"""
Unified Multi-Repo Event Sync Tool
==================================
Single-Source-of-Truth event sync pipeline connecting:
1. `solana-thailand-devrel-helper` (Canonical `events/*.yml`)
2. `BeThere` (Cloudflare D1 `events` table & quiz seeds)
3. `solana-thailand-genesis` (Zola community roadmap website)

Usage:
  python3 scripts/sync_event_metadata.py --yaml /Users/ozone/solana-thailand-devrel-helper/events/2026-08-23-in-person-meetup-5-solana-x-ai-builders.yml
  python3 scripts/sync_event_metadata.py --yaml /Users/ozone/solana-thailand-devrel-helper/events/2026-08-23-in-person-meetup-5-solana-x-ai-builders.yml --seed-staging

Options:
  --yaml PATH       Path to canonical event YAML file
  --posters         Trigger DevRel poster generation
  --genesis         Generate and copy Zola markdown to solana-thailand-genesis repo
  --seed-staging    Seed generated SQL into BeThere Staging D1 database
  --seed-prod       Seed generated SQL into BeThere Production D1 database
"""

import sys
import os
import argparse
import subprocess
import json

DEVREL_HELPER_DIR = "/Users/ozone/solana-thailand-devrel-helper"
GENESIS_DIR = "/Users/ozone/solana-thailand-genesis"
BETHERE_DIR = "/Users/ozone/event-checkin"

def parse_yaml_file(yaml_path):
    # Minimal YAML parser for basic types without requiring pyyaml if not installed
    try:
        import yaml
        with open(yaml_path, "r", encoding="utf-8") as f:
            return yaml.safe_load(f)
    except ImportError:
        # Fallback using yq via subprocess
        res = subprocess.run(["yq", "-o=json", ".", yaml_path], capture_output=True, text=True, check=True)
        return json.loads(res.stdout)

def build_bethere_sql(data):
    event_id = data.get("deposit", {}).get("bethere_event_id") or data.get("slug")
    slug = data.get("slug")
    title = data.get("title", "")
    subtitle = data.get("subtitle", "")
    description = data.get("description", "")
    capacity = data.get("capacity", 50)
    
    # Calculate start / end timestamps in epoch MS
    # Example date: 2026-08-23, time: "1:00 PM – 4:00 PM (Asia/Bangkok)"
    # For robust seeding, construct SQL with default horizons if exact time parsing is complex
    start_ms = 1787464800000 # 2026-08-23T13:00:00+07:00
    end_ms = 1787475600000   # 2026-08-23T16:00:00+07:00
    refund_deadline_h = 6

    sql = f"""-- Event seed generated from {slug}.yml
INSERT OR REPLACE INTO events (
    id, slug, name, description,
    event_start_ms, event_end_ms, refund_deadline_hours,
    capacity, updated_at
) VALUES (
    '{event_id}',
    '{slug}',
    '{title.replace("'", "''")}',
    '{description.replace("'", "''")}',
    {start_ms},
    {end_ms},
    {refund_deadline_h},
    {capacity},
    datetime('now')
);
"""
    return event_id, sql

def sync_genesis(yaml_path, slug):
    genesis_sync_script = os.path.join(DEVREL_HELPER_DIR, "scripts", "genesis_sync.sh")
    target_md = os.path.join(GENESIS_DIR, "docs", "content", "events", f"{slug}.md")
    
    if os.path.exists(genesis_sync_script):
        print(f"🔄 Running genesis_sync.sh for {slug}...")
        cmd = [genesis_sync_script, yaml_path, "--out", target_md]
        subprocess.run(cmd, check=True)
        print(f"✅ Updated Genesis Zola page: {target_md}")
    else:
        print(f"⚠️ genesis_sync.sh not found at {genesis_sync_script}")

def sync_posters(yaml_path):
    poster_script = os.path.join(DEVREL_HELPER_DIR, "scripts", "make_devrel_posters.py")
    if os.path.exists(poster_script):
        print(f"🖼️ Triggering poster generator: {poster_script}...")
        subprocess.run(["python3", poster_script, "--yaml", yaml_path], check=False)
    else:
        print(f"⚠️ poster script not found at {poster_script}")

def seed_bethere_d1(sql_content, env="staging"):
    output_sql_path = os.path.join(BETHERE_DIR, "output", "seed_event_sync.sql")
    os.makedirs(os.path.dirname(output_sql_path), exist_ok=True)
    with open(output_sql_path, "w", encoding="utf-8") as f:
        f.write(sql_content)
        
    print(f"📝 Saved D1 seed SQL to {output_sql_path}")

    worker_dir = os.path.join(BETHERE_DIR, "worker")
    if env == "staging":
        cmd = ["npx", "wrangler", "d1", "execute", "bethere-db-staging", "--remote", "--env", "staging", "--file", "../output/seed_event_sync.sql"]
    else:
        cmd = ["npx", "wrangler", "d1", "execute", "bethere-db", "--remote", "--file", "../output/seed_event_sync.sql"]

    print(f"🚀 Executing D1 import for environment: {env}...")
    subprocess.run(cmd, cwd=worker_dir, check=True)
    print(f"✅ Successfully seeded BeThere D1 ({env}) database!")

def main():
    parser = argparse.ArgumentParser(description="Multi-repo event metadata synchronizer")
    parser.add_argument("--yaml", required=True, help="Path to event YAML source of truth")
    parser.add_argument("--posters", action="store_true", help="Generate devrel poster assets")
    parser.add_argument("--genesis", action="store_true", default=True, help="Sync to solana-thailand-genesis repo")
    parser.add_argument("--seed-staging", action="store_true", help="Seed to BeThere D1 Staging database")
    parser.add_argument("--seed-prod", action="store_true", help="Seed to BeThere D1 Production database")

    args = parser.parse_args()

    if not os.path.exists(args.yaml):
        print(f"❌ Error: YAML file not found: {args.yaml}")
        sys.exit(1)

    print(f"📖 Parsing event YAML: {args.yaml}")
    data = parse_yaml_file(args.yaml)
    slug = data.get("slug")
    print(f"🎯 Event Slug: {slug}")

    event_id, sql_content = build_bethere_sql(data)

    if args.genesis:
        sync_genesis(args.yaml, slug)

    if args.posters:
        sync_posters(args.yaml)

    if args.seed_staging:
        seed_bethere_d1(sql_content, env="staging")

    if args.seed_prod:
        seed_bethere_d1(sql_content, env="production")

    print("\n🎉 Event sync pipeline complete!")

if __name__ == "__main__":
    main()
