#!/usr/bin/env bash
# Build the Leptos WASM frontend for production.
# Strips trunk's live-reload WebSocket script from the output HTML,
# which would otherwise show a blank overlay when served without trunk.
#
# Usage:
#   bash build.sh          # Build once (production)
#   bash build.sh --watch  # Auto-rebuild on file changes

set -euo pipefail

cleanup_html() {
    echo "🧹 Cleaning trunk live-reload script from dist/index.html..."
    python3 << 'PY'
import re, sys

with open("dist/index.html", "r") as f:
    html = f.read()

# Remove nonce attributes
html = re.sub(r' nonce="[^"]*"', '', html)

# Remove trunk WS live-reload script (second <script>...</script> block)
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
}

bump_sw_version() {
    # The service worker caches assets under CACHE_VERSION and only re-activates
    # (and purges stale caches) when its own bytes change. If the version isn't
    # bumped per deploy, users keep the STALE cached frontend even after a deploy
    # (they'd need a manual Cmd+Shift+R). Derive the version from a hash of the
    # final index.html — which references every content-hashed asset (WASM/CSS/JS)
    # — so any asset change invalidates the SW cache automatically.
    if [[ ! -f dist/sw.js || ! -f dist/index.html ]]; then
        echo "⚠️  dist/sw.js or dist/index.html missing — skipping SW version bump"
        return
    fi
    local ver
    ver="$(shasum -a 256 dist/index.html | cut -c1-16)"
    sed -i.bak -E "s/var CACHE_VERSION = \"[^\"]*\";/var CACHE_VERSION = \"bethere-${ver}\";/" dist/sw.js
    rm -f dist/sw.js.bak
    echo "🔁 SW cache version → bethere-${ver} (auto-invalidates stale caches on deploy)"
}

build() {
    echo "🏗️  Building Leptos WASM frontend..."
    ~/.cargo/bin/trunk build --release

    # Trunk only copies JS files directly referenced by #[wasm_bindgen(module = "...")].
    # lazy_assets.js is imported by scanner.js/clipboard.js but not by Rust directly,
    # so trunk skips it. Copy manually to avoid module resolution failures at runtime.
    SNIPPET_DIR="$(ls -d dist/snippets/event-checkin-frontend-*/js 2>/dev/null | head -1)"
    if [[ -n "$SNIPPET_DIR" && -f js/lazy_assets.js ]]; then
        cp js/lazy_assets.js "$SNIPPET_DIR/lazy_assets.js"
        echo "📋 Copied js/lazy_assets.js → $SNIPPET_DIR/lazy_assets.js"
    else
        echo "⚠️  No snippet dir or lazy_assets.js not found — QR scanner may fail at runtime"
    fi

    cleanup_html
    bump_sw_version
}

# --watch mode: auto-rebuild on file changes
if [[ "${1:-}" == "--watch" ]]; then
    echo "👀 Watching frontend for changes..."
    echo "   Run 'cd worker && bash deploy.sh dev' in another terminal for the server."
    echo "   Hard-refresh browser (Cmd+Shift+R) after rebuild to pick up new assets."
    echo ""
    ~/.cargo/bin/cargo-watch \
        -w src \
        -w style.css \
        -w index.html \
        -s 'bash build.sh'
else
    build
fi
