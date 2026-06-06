#!/usr/bin/env bash
# migrate_kv_walkins_to_d1.sh — One-time migration of walk-in KV records to D1.
#
# Scans KV for walkin:* keys, parses the JSON values, and upserts them into D1.
# Safe to run multiple times — uses INSERT ... WHERE NOT EXISTS to avoid duplicates.
#
# Prerequisites:
#   - wrangler CLI authenticated (npx wrangler login)
#   - python3 with sqlite3 module (standard on macOS)
#
# Usage:
#   bash scripts/migrate_kv_walkins_to_d1.sh           # Run migration
#   bash scripts/migrate_kv_walkins_to_d1.sh --dry-run  # Preview without writing to D1

set -euo pipefail
cd "$(dirname "$0")/.."

KV_NS="c8a6a87f9ed34ce0a3c8e48b84039214"  # Production EVENTS KV namespace
D1_DB="bethere-db"                          # D1 database name
DRY_RUN=false

if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
    echo "🔍 DRY RUN — no D1 writes will be performed"
fi

echo "📋 Migrating walk-in KV records to D1..."
echo "   KV namespace: $KV_NS"
echo "   D1 database:  $D1_DB"
echo ""

# Step 1: List all walkin:* keys from KV
echo "📡 Scanning KV for walkin:* keys..."
KEYS_JSON=$(npx wrangler kv key list --namespace-id "$KV_NS" --prefix "walkin:" --remote 2>/dev/null)

WALKIN_COUNT=$(echo "$KEYS_JSON" | python3 -c "
import sys, json
keys = json.load(sys.stdin)
walkins = [k['name'] for k in keys if k['name'].startswith('walkin:')]
print(len(walkins))
" 2>/dev/null || echo 0)

if [[ "$WALKIN_COUNT" == "0" ]]; then
    echo "✅ No walkin:* keys found in KV. Nothing to migrate."
    exit 0
fi

echo "   Found $WALKIN_COUNT walk-in key(s) to migrate"
echo ""

# Step 2: Read each key's value and collect into a temp file
TEMP_SQL=$(mktemp /tmp/bethere_walkin_migration_XXXXXX.sql)
TEMP_CSV=$(mktemp /tmp/bethere_walkin_migration_XXXXXX.csv)

echo "📥 Reading walk-in records from KV..."
MIGRATED=0
SKIPPED=0
ERRORS=0

# Extract key names
WALKIN_KEYS=$(echo "$KEYS_JSON" | python3 -c "
import sys, json
keys = json.load(sys.stdin)
for k in keys:
    if k['name'].startswith('walkin:'):
        print(k['name'])
")

while IFS= read -r key; do
    # Read the JSON value from KV
    value=$(npx wrangler kv key get --namespace-id "$KV_NS" "$key" --remote 2>/dev/null || echo "")

    if [[ -z "$value" ]]; then
        echo "   ⚠️  Empty value for key: $key"
        ERRORS=$((ERRORS + 1))
        continue
    fi

    # Parse JSON and extract fields
    # Key format: walkin:{event_id}:{email}
    parsed=$(echo "$value" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    # Generate SQL INSERT with WHERE NOT EXISTS for idempotency
    event_id = d.get('event_id', '').replace(\"'\", \"''\")
    email = d.get('email', '').replace(\"'\", \"''\").lower()
    name = d.get('name', '').replace(\"'\", \"''\")
    phone = d.get('phone') or ''
    phone = phone.replace(\"'\", \"''\") if phone else ''
    claim_token = d.get('claim_token', '').replace(\"'\", \"''\")
    checked_in_at = d.get('checked_in_at', '').replace(\"'\", \"''\")
    checked_in_by = d.get('checked_in_by', '').replace(\"'\", \"''\")
    wallet_address = d.get('wallet_address') or ''
    claimed_at = d.get('claimed_at') or ''

    # Use claim_token as the id if it looks like a UUID, otherwise generate one
    import uuid
    try:
        record_id = str(uuid.UUID(claim_token))
    except:
        record_id = str(uuid.uuid4())

    # Escape for SQL
    wallet_escaped = wallet_address.replace(\"'\", \"''\") if wallet_address else ''
    claimed_escaped = claimed_at.replace(\"'\", \"''\") if claimed_at else ''

    sql = f\"\"\"INSERT INTO attendees (id, event_id, email, name, approval_status, participation_type, contact_channel, contact_handle, checked_in_at, checked_in_by, claim_token, claimed_at, claim_asset_id, claim_signature, wallet_address, created_at, updated_at)
SELECT '{record_id}', '{event_id}', '{email}', '{name}', 'approved', 'walkin', '{phone}', '', '{checked_in_at}', '{checked_in_by}', '{claim_token}', '{claimed_escaped}', '', '', '{wallet_escaped}', datetime('now'), datetime('now')
WHERE NOT EXISTS (
    SELECT 1 FROM attendees WHERE event_id = '{event_id}' AND email = '{email}' AND participation_type = 'walkin'
);\"

    print(sql)
except Exception as e:
    print(f'-- ERROR: {e}', file=sys.stderr)
    sys.exit(1)
" 2>/dev/null)

    if [[ $? -ne 0 || -z "$parsed" ]]; then
        echo "   ⚠️  Failed to parse: $key"
        ERRORS=$((ERRORS + 1))
        continue
    fi

    echo "$parsed" >> "$TEMP_SQL"
    MIGRATED=$((MIGRATED + 1))

done <<< "$WALKIN_KEYS"

echo ""
echo "📊 Migration summary:"
echo "   Records to migrate: $MIGRATED"
echo "   Errors during read: $ERRORS"
echo ""

if [[ "$MIGRATED" == "0" ]]; then
    echo "ℹ️  No valid records to migrate."
    rm -f "$TEMP_SQL" "$TEMP_CSV"
    exit 0
fi

# Step 3: Show SQL for review
echo "📄 Generated SQL (first 5 statements):"
head -5 "$TEMP_SQL"
echo "   ..."
echo ""

if [[ "$DRY_RUN" == "true" ]]; then
    echo "🔍 DRY RUN — SQL written to: $TEMP_SQL"
    echo "   Review and run manually with:"
    echo "   npx wrangler d1 execute $D1_DB --file=$TEMP_SQL --remote"
    exit 0
fi

# Step 4: Execute migration against D1
echo "📤 Executing migration against D1..."
npx wrangler d1 execute "$D1_DB" --file="$TEMP_SQL" --remote 2>&1 || {
    echo "❌ D1 migration failed!"
    echo "   SQL file preserved at: $TEMP_SQL"
    echo "   Fix errors and retry with:"
    echo "   npx wrangler d1 execute $D1_DB --file=$TEMP_SQL --remote"
    exit 1
}

echo ""
echo "✅ Migration complete!"
echo "   $MIGRATED walk-in record(s) migrated to D1"
if [[ $ERRORS -gt 0 ]]; then
    echo "   ⚠️  $ERRORS record(s) had errors and were skipped"
fi
echo ""

# Step 5: Verify — count walkins in D1 vs KV
echo "🔍 Verification..."
D1_COUNT=$(npx wrangler d1 execute "$D1_DB" --command="SELECT COUNT(*) as cnt FROM attendees WHERE participation_type = 'walkin'" --remote 2>/dev/null | python3 -c "
import sys, json
try:
    lines = sys.stdin.read()
    # wrangler outputs JSON in a specific format
    idx = lines.find('\"cnt\"')
    if idx >= 0:
        # Extract the number after cnt
        import re
        m = re.search(r'\"cnt\"\s*:\s*(\d+)', lines)
        if m:
            print(m.group(1))
        else:
            print('?')
    else:
        print('?')
except:
    print('?')
" 2>/dev/null || echo "?")

echo "   D1 walk-in count: $D1_COUNT"
echo "   KV walk-in keys:  $WALKIN_COUNT"
echo ""
echo "   If D1 count >= KV count, migration was successful."
echo "   You can safely proceed to remove KV mirror writes in code."
echo ""

# Cleanup
rm -f "$TEMP_SQL" "$TEMP_CSV"
echo "🧹 Temp files cleaned up."
