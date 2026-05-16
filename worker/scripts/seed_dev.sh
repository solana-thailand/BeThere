#!/usr/bin/env bash
# seed_dev.sh — Seed preview KV with production event data for local dev.
#
# Usage:
#   bash scripts/seed_dev.sh          # Copy current production events to preview KV
#   bash scripts/seed_dev.sh --clean  # Wipe preview KV first, then seed
#
# After seeding:
#   bash deploy.sh dev          # Local dev with remote KV (seeded preview namespace)
#   bash deploy.sh dev --local  # Local dev with SQLite KV (empty)

set -euo pipefail
cd "$(dirname "$0")/.."

PREVIEW_NS="7d74e1f62fb545be811eaefc8b059dee"
PROD_URL="https://bethere.solana-thailand.workers.dev"

# wrangler dev --remote uses the preview_id namespace for KV.
# We seed that preview namespace with production data.

# Read secrets from .dev.vars for fields not exposed by public API
sheet_id=$(grep "^GOOGLE_SHEET_ID=" .dev.vars 2>/dev/null | cut -d= -f2 || echo "")
sheet_name=$(grep "^GOOGLE_SHEET_NAME=" .dev.vars 2>/dev/null | cut -d= -f2 || echo "Attendees")
staff_sheet=$(grep "^GOOGLE_STAFF_SHEET_NAME=" .dev.vars 2>/dev/null | cut -d= -f2 || echo "staff")
staff_emails=$(grep "^STAFF_EMAILS=" .dev.vars 2>/dev/null | cut -d= -f2 || echo "")
super_admins=$(grep "^SUPER_ADMIN_EMAILS=" .dev.vars 2>/dev/null | cut -d= -f2 || echo "")

echo "🌱 Seeding preview KV ($PREVIEW_NS) from production..."

# Step 1: Optionally clean preview KV
if [[ "${1:-}" == "--clean" ]]; then
    echo "🧹 Cleaning preview KV..."
    keys_json=$(npx wrangler kv key list --namespace-id "$PREVIEW_NS" --remote 2>/dev/null)
    key_count=$(echo "$keys_json" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo 0)
    if [[ "$key_count" -gt 0 ]]; then
        echo "   Deleting $key_count keys..."
        echo "$keys_json" | python3 -c "
import sys, json
for k in json.load(sys.stdin):
    print(k['name'])
" | while IFS= read -r key; do
            yes | npx wrangler kv key delete --namespace-id "$PREVIEW_NS" "$key" --remote 2>/dev/null || true
        done
        echo "   ✅ Deleted $key_count keys"
    fi
fi

# Step 2: Fetch event list from production API
echo "📡 Fetching events from production..."
events_resp=$(curl -sf "$PROD_URL/api/public/events")
event_count=$(echo "$events_resp" | python3 -c "import sys,json; print(len(json.load(sys.stdin)['data']['events']))" 2>/dev/null || echo 0)

if [[ "$event_count" -eq 0 ]]; then
    echo "❌ No active events found on production."
    exit 1
fi

echo "   Found $event_count event(s)"

# Step 3: Build and write the event index (EventIndex with full EventMeta fields)
events_index=$(echo "$events_resp" | python3 -c "
import sys, json
events = json.load(sys.stdin)['data']['events']
index = {'events': [{
    'id': e['id'],
    'name': e['name'],
    'slug': e['slug'],
    'status': e['status'],
    'event_start_ms': e['event_start_ms'],
    'event_end_ms': e['event_end_ms'],
    'sheet_id': '$sheet_id',
    'created_at': e.get('created_at', ''),
    'organizer_emails': '$super_admins'.split(',') if '$super_admins' else [],
    'deposit_enabled': e.get('deposit_enabled', False),
    'max_refundable_deposits': 0,
    'escrow_address': '',
    'escrow_status': 'none',
    'event_format': e.get('event_format', 'in_person'),
} for e in events]}
print(json.dumps(index))
")

echo "📝 Writing event index..."
echo "$events_index" > /tmp/bethere_events_index.json
npx wrangler kv key put --namespace-id "$PREVIEW_NS" "events" --path /tmp/bethere_events_index.json --remote 2>/dev/null

# Step 4: Fetch and write each event config
slugs=$(echo "$events_resp" | python3 -c "
import sys, json
for e in json.load(sys.stdin)['data']['events']:
    print(e['slug'])
")

for slug in $slugs; do
    echo "📡 Fetching config for: $slug"
    event_detail=$(curl -sf "$PROD_URL/api/public/event/$slug")
    event_id=$(echo "$event_detail" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['id'])")

    # Build full EventConfig from public data + local secrets
    config_json=$(echo "$event_detail" | python3 -c "
import sys, json
d = json.load(sys.stdin)['data']
config = {
    'id': d.get('id', ''),
    'name': d.get('name', ''),
    'slug': d.get('slug', ''),
    'tagline': d.get('tagline', ''),
    'link': d.get('link', ''),
    'status': d.get('status', 'active'),
    'event_start_ms': d.get('event_start_ms', 0),
    'event_end_ms': d.get('event_end_ms', 0),
    'sheet_id': '$sheet_id',
    'sheet_name': '$sheet_name',
    'staff_sheet_name': '$staff_sheet',
    'quiz_enabled': d.get('quiz_enabled', True),
    'nft_collection_mint': '',
    'nft_metadata_uri': '',
    'nft_image_url': '',
    'nft_name_template': 'BeThere - ' + d.get('name', ''),
    'nft_symbol': 'BETHERE',
    'nft_description_template': 'Proof of attendance at ' + d.get('name', ''),
    'organizer_emails': '$super_admins'.split(',') if '$super_admins' else ['ratchapon.poc@gmail.com'],
    'staff_emails': '$staff_emails'.split(',') if '$staff_emails' else ['ratchapon.poc@gmail.com'],
    'claim_base_url': 'http://localhost:8787/claim',
    'merkle_tree': '',
    'deposit_enabled': d.get('deposit_enabled', False),
    'deposit_amount_usdc': d.get('deposit_amount_usdc', 0),
    'deposit_amount_thb': d.get('deposit_amount_thb', 0),
    'promptpay_id': '',
    'escrow_address': '',
    'escrow_status': 'none',
    'organizer_wallet': '',
    'on_chain_event_id': 0,
    'refund_deadline_hours': d.get('refund_deadline_hours', 168),
    'max_refundable_deposits': 0,
    'description': d.get('description', ''),
    'location': d.get('location', ''),
    'event_format': d.get('event_format', 'in_person'),
    'require_contact_info': d.get('require_contact_info', True),
    'created_at': d.get('created_at', ''),
    'updated_at': d.get('created_at', ''),
    'updated_by': '',
}
print(json.dumps(config))
")

    echo "📝 Writing event:$event_id"
    echo "$config_json" > "/tmp/bethere_event_${event_id}.json"
    npx wrangler kv key put --namespace-id "$PREVIEW_NS" "event:$event_id" --path "/tmp/bethere_event_${event_id}.json" --remote 2>/dev/null
done

echo ""
echo "✅ Done! Seeded $event_count event(s) to preview KV."
echo ""
echo "Test locally:"
echo "  cd worker && bash deploy.sh dev"
echo "  open http://localhost:8787/e/$slug"
