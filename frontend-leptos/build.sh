#!/usr/bin/env bash
# Build the Leptos WASM frontend for production.
# Strips trunk's live-reload WebSocket script from the output HTML,
# which would otherwise show a blank overlay when served without trunk.

set -euo pipefail

echo "🏗️  Building Leptos WASM frontend..."
~/.cargo/bin/trunk build --release

echo "🧹 Cleaning trunk live-reload script from dist/index.html..."
python3 << 'PY'
import re, sys

with open("dist/index.html", "r") as f:
    html = f.read()

# Remove nonce attributes
html = re.sub(r' nonce="[^"]*"', '', html)

# Remove trunk WS live-reload script (second <script>...</script> block)
# Pattern: </script> followed by <script>"use strict"; ... containing TRUNK_ADDRESS
html = re.sub(
    r'(</script>)\s*<script>\s*"use strict";.*?</script>\s*',
    r'\1\n    ',
    html,
    count=1,
    flags=re.DOTALL
)

# Remove any remaining {{__TRUNK_*}} artifacts
html = html.replace("{{__TRUNK_ADDRESS__}}", "")
html = html.replace("{{__TRUNK_WS_BASE__}}", "")

with open("dist/index.html", "w") as f:
    f.write(html)

print("  ✅ Done")
PY

echo "📦 Output:"
ls -lh dist/
