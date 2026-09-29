#!/usr/bin/env bash
# Build the Leptos WASM frontend for production.
# Strips trunk's live-reload WebSocket script from the output HTML,
# which would otherwise show a blank overlay when served without trunk.
#
# Usage:
#   bash build.sh          # Build once (production)
#   bash build.sh --watch  # Auto-rebuild on file changes

set -euo pipefail

# Trunk reads NO_COLOR as a boolean option. Some shells and agent runtimes use
# the conventional NO_COLOR=1 form, which Trunk 0.21 rejects before building.
if [[ -n "${NO_COLOR:-}" && "${NO_COLOR}" != "true" && "${NO_COLOR}" != "false" ]]; then
    export NO_COLOR=true
fi

# The staff shell (.issues/169): the same crate with `--features staff`, built
# into STAFF_DIST and merged into dist/ as staff-app.html. `_redirects` serves
# it for /staff, /admin, …; the attendee shell (index.html) never loads it.
STAFF_DIST="dist-staff"

cleanup_html() {
    local html_file="$1"
    echo "🧹 Cleaning trunk live-reload script from ${html_file}..."
    HTML_FILE="$html_file" python3 << 'PY'
import os, re, sys

path = os.environ["HTML_FILE"]
with open(path, "r") as f:
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

# Trunk emits the modulepreload tags in a different order on every build, so
# index.html (and the SW CACHE_VERSION hashed from it) churned on rebuilds that
# changed nothing (plan 028 F1). Sort them: main bundle first, then by href.
def sort_preloads(run):
    tags = re.findall(r'<link rel="modulepreload"[^>]*>', run.group(0))
    href = lambda t: re.search(r'href="([^"]*)"', t).group(1)
    tags.sort(key=lambda t: (href(t).startswith("/snippets/"), href(t)))
    return "".join(tags)

html = re.sub(r'<link rel="modulepreload"[^>]*>(?:\s*<link rel="modulepreload"[^>]*>)*', sort_preloads, html)

with open(path, "w") as f:
    f.write(html)

print("  ✅ Done")
PY
}

merge_staff_shell() {
    # Every asset name is content-hashed (or byte-identical in both builds:
    # styles, snippets, sw.js, _headers), so the staff files sit beside the
    # attendee ones. Only the shell is renamed.
    mv "$STAFF_DIST/index.html" "$STAFF_DIST/staff-app.html"
    cp -R "$STAFF_DIST"/. dist/
    rm -rf "$STAFF_DIST"
    echo "🧩 Merged the staff shell → dist/staff-app.html"
}

bump_sw_version() {
    # The service worker caches assets under CACHE_VERSION and only re-activates
    # (and purges stale caches) when its own bytes change. If the version isn't
    # bumped per deploy, users keep the STALE cached frontend even after a deploy
    # (they'd need a manual Cmd+Shift+R). Derive the version from a hash of the
    # final index.html — which references every content-hashed asset (WASM/CSS/JS)
    # — so any asset change invalidates the SW cache automatically.
    # staff-app.html is hashed in too, so a staff-only change also purges.
    if [[ ! -f dist/sw.js || ! -f dist/index.html || ! -f dist/staff-app.html ]]; then
        echo "⚠️  dist/sw.js, dist/index.html or dist/staff-app.html missing — skipping SW version bump"
        return
    fi
    local ver
    ver="$(cat dist/index.html dist/staff-app.html | shasum -a 256 | cut -c1-16)"
    sed -i.bak -E "s/var CACHE_VERSION = \"[^\"]*\";/var CACHE_VERSION = \"bethere-${ver}\";/" dist/sw.js
    rm -f dist/sw.js.bak
    # sed exits 0 on no match. A missed bump means activate never purges the
    # previous build's caches (plan 028 F1), so fail the build instead.
    if ! grep -q "var CACHE_VERSION = \"bethere-${ver}\";" dist/sw.js; then
        echo "❌ SW cache version bump did not apply — check the CACHE_VERSION line in sw.js" >&2
        exit 1
    fi
    echo "🔁 SW cache version → bethere-${ver} (auto-invalidates stale caches on deploy)"
}

precompress_assets() {
    # Cloudflare compresses assets on the fly at ~brotli q4; q11 done once here
    # saves ~380 KB per first load (.issues/135 §6.3) and ~20 KB per jsQR load
    # (plan 028 F10). The Worker serves this sibling to clients that accept br
    # (worker/src/precompressed.rs) and falls back to the plain file otherwise,
    # so a missing .br only costs bytes. Keep this list in step with
    # PRECOMPRESSED_ASSETS and wrangler.toml's run_worker_first.
    local asset
    for asset in dist/event-checkin-frontend-*_bg.wasm dist/jsqr-*.js; do
        [[ -f "$asset" ]] || { echo "⚠️  No $asset in dist/ — skipping its precompression"; continue; }
        # shellcheck disable=SC2016 # ${...} below is a JS template literal
        node -e '
const fs = require("fs"), z = require("zlib");
const [src] = process.argv.slice(1);
const raw = fs.readFileSync(src);
const br = z.brotliCompressSync(raw, { params: {
  [z.constants.BROTLI_PARAM_QUALITY]: 11,
  [z.constants.BROTLI_PARAM_SIZE_HINT]: raw.length,
}});
if (!z.brotliDecompressSync(br).equals(raw)) { console.error("brotli round-trip mismatch"); process.exit(1); }
fs.writeFileSync(src + ".br", br);
console.log(`🗜️  ${src}.br: ${raw.length} → ${br.length} bytes (brotli q11)`);
' "$asset"
    done
}

build() {
    echo "🏗️  Building Leptos WASM frontend..."
    # Short commit for the landing footer's build line (.plans/038 P2-f); the
    # footer hides the line when this is empty (a plain `trunk build`).
    BETHERE_GIT_SHA="$(git rev-parse --short=7 HEAD 2>/dev/null || true)"
    export BETHERE_GIT_SHA
    # Staff first: every `trunk build` replaces its whole dist directory.
    ~/.cargo/bin/trunk build --release --features staff --dist "$STAFF_DIST"
    ~/.cargo/bin/trunk build --release

    # Trunk only copies JS files directly referenced by #[wasm_bindgen(module = "...")].
    # lazy_assets.js is imported by scanner.js/clipboard.js/slip_qr.js but not by Rust directly,
    # so trunk skips it. Copy manually to avoid module resolution failures at runtime.
    # An unmatched glob expands to the literal pattern, so test each directory
    # itself rather than the string being non-empty.
    local snippet_dir copied=0
    for snippet_dir in dist/snippets/event-checkin-frontend-*/js "$STAFF_DIST"/snippets/event-checkin-frontend-*/js; do
        [[ -d "$snippet_dir" && -f js/lazy_assets.js ]] || continue
        cp js/lazy_assets.js "$snippet_dir/lazy_assets.js"
        echo "📋 Copied js/lazy_assets.js → $snippet_dir/lazy_assets.js"
        copied=1
    done
    [[ "$copied" == 1 ]] || echo "⚠️  No snippet dir or lazy_assets.js not found — QR scanner may fail at runtime"

    cleanup_html dist/index.html
    cleanup_html "$STAFF_DIST/index.html"
    merge_staff_shell
    echo "📦 Output:"
    ls -lh dist/
    bump_sw_version
    precompress_assets
}

# --watch mode: auto-rebuild on file changes
if [[ "${1:-}" == "--watch" ]]; then
    echo "👀 Watching frontend for changes..."
    echo "   Run 'cd worker && bash deploy.sh dev' in another terminal for the server."
    echo "   Hard-refresh browser (Cmd+Shift+R) after rebuild to pick up new assets."
    echo ""
    ~/.cargo/bin/cargo-watch \
        -w src \
        -w styles \
        -w index.html \
        -s 'bash build.sh'
else
    build
fi
