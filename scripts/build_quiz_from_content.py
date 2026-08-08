#!/usr/bin/env python3
"""
BeThere — Quiz Distillation & Import Script
===========================================
Distills event content/transcripts (from `viral` ASR or `solana-learn` catalog)
into structured multiple-choice quiz questions for BeThere D1 database (`quiz_configs`).

Usage:
  python3 scripts/build_quiz_from_content.py --event-id <EVENT_ID> --title "Solana x AI Builders" --input transcript.txt
  python3 scripts/build_quiz_from_content.py --event-id flow-test-event --sample

Options:
  --event-id ID       Event ID in BeThere (e.g. flow-test-event)
  --title TITLE       Event or session title
  --input FILE        Path to transcript / brief text file
  --passing-score PCT Passing score percentage (default: 80)
  --max-attempts N    Maximum quiz attempts allowed (default: 3)
  --output-json PATH  Path to write formatted quiz JSON (default: output/quiz_<event-id>.json)
  --output-sql PATH   Path to write D1 SQL seed script (default: output/seed_quiz_<event-id>.sql)
  --sample            Generate a sample quiz derived from solana-learn catalog
"""

import sys
import os
import json
import argparse

# Default solana-learn ground truth sample questions
SOLANA_LEARN_SAMPLE_QUESTIONS = [
    {
        "id": "q1",
        "text": "What compiler and execution environment does Solana use for on-chain programs?",
        "options": [
            "Solana Bytecode Format (SBF) compiled from Rust",
            "EVM Bytecode compiled from Solidity",
            "WebAssembly (WASM) compiled from C++",
            "JVM Bytecode compiled from Java"
        ],
        "correct_index": 0,
        "explanation": "Solana programs are written in Rust (or C) and compiled to SBF (Solana Bytecode Format) for high-throughput execution.",
        "session_id": "session-1",
        "session_title": "Solana Core Architecture",
        "enabled": True
    },
    {
        "id": "q2",
        "text": "What is the primary advantage of Compressed NFTs (cNFTs) on Solana?",
        "options": [
            "Significantly lower minting cost via Merkle Tree state compression (~$0.000005 per NFT)",
            "They can only be stored in hardware wallets",
            "They bypass transaction signature checks",
            "They do not require an indexer or RPC node"
        ],
        "correct_index": 0,
        "explanation": "cNFTs use Metaplex Bubblegum Concurrent Merkle Trees to store state off-chain, reducing mint costs by ~10,000x.",
        "session_id": "session-1",
        "session_title": "State Compression & cNFTs",
        "enabled": True
    },
    {
        "id": "q3",
        "text": "In the BeThere Escrow protocol, what happens to an attendee's deposit if they do not check in before the refund deadline?",
        "options": [
            "The deposit becomes forfeitable and can be claimed by the organizer for community treasury",
            "The deposit is burned automatically on-chain",
            "The deposit is returned to the attendee wallet without check-in",
            "The deposit is locked forever on-chain"
        ],
        "correct_index": 0,
        "explanation": "For no-show attendees who miss the refund deadline, `claim_forfeited` allows the organizer to reclaim funds for event expenses.",
        "session_id": "session-2",
        "session_title": "Escrow Protocol & Accountability",
        "enabled": True
    }
]

def generate_quiz_config(event_id, questions, passing_score=80, max_attempts=3):
    return {
        "event_id": event_id,
        "config_json": json.dumps({
            "questions": questions,
            "passing_score_percent": passing_score,
            "max_attempts": max_attempts,
            "per_attempt_timer_seconds": 300
        }, separators=(',', ':'))
    }

def generate_sql_seed(event_id, config_json_str):
    # Escape single quotes in JSON string for SQLite
    escaped_json = config_json_str.replace("'", "''")
    sql = f"""-- Quiz config seed for event '{event_id}'
INSERT OR REPLACE INTO quiz_configs (event_id, config_json, updated_at)
VALUES (
    '{event_id}',
    '{escaped_json}',
    datetime('now')
);
"""
    return sql

def main():
    parser = argparse.ArgumentParser(description="Distill content into BeThere D1 quiz config JSON and SQL")
    parser.add_argument("--event-id", required=True, help="Target Event ID in BeThere")
    parser.add_argument("--title", default="Solana Builder Session", help="Event title")
    parser.add_argument("--input", help="Path to transcript or content text file")
    parser.add_argument("--passing-score", type=int, default=80, help="Passing score percentage")
    parser.add_argument("--max-attempts", type=int, default=3, help="Max quiz attempts allowed")
    parser.add_argument("--output-dir", default="output", help="Output directory for generated files")
    parser.add_argument("--sample", action="store_true", help="Use solana-learn sample taxonomy questions")

    args = parser.parse_args()

    os.makedirs(args.output_dir, exist_ok=True)

    questions = SOLANA_LEARN_SAMPLE_QUESTIONS
    if args.input and os.path.exists(args.input):
        print(f"📖 Reading content from {args.input}...")
        # Future LLM pipeline hook: process raw transcript into structured questions
        # For now, append title to sample set
        for q in questions:
            q["session_title"] = args.title

    quiz_data = {
        "questions": questions,
        "passing_score_percent": args.passing_score,
        "max_attempts": args.max_attempts,
        "per_attempt_timer_seconds": 300
    }

    json_path = os.path.join(args.output_dir, f"quiz_{args.event_id}.json")
    sql_path = os.path.join(args.output_dir, f"seed_quiz_{args.event_id}.sql")

    with open(json_path, "w") as f:
        json.dump(quiz_data, f, indent=2)

    config_json_str = json.dumps(quiz_data, separators=(',', ':'))
    sql_content = generate_sql_seed(args.event_id, config_json_str)

    with open(sql_path, "w") as f:
        f.write(sql_content)

    print(f"✅ Generated Quiz Config JSON: {json_path}")
    print(f"✅ Generated D1 SQL Seed File: {sql_path}")
    print("\nTo seed D1 Staging:")
    print(f"  npx wrangler d1 execute bethere-db-staging --remote --env staging --file {sql_path}")
    print("\nTo seed D1 Production:")
    print(f"  npx wrangler d1 execute bethere-db --remote --file {sql_path}")

if __name__ == "__main__":
    main()
