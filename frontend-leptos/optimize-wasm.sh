#!/usr/bin/env bash
# WASM post-build optimization script
# Runs wasm-opt and wasm-strip on the dist WASM file
# Requires: binaryen (wasm-opt), wabt (wasm-strip)
#
# NOT WIRED INTO ANYTHING. Neither build.sh, deploy.sh nor CI calls this. Before
# you wire it up, read the next paragraph — it used to make things worse.
#
# IT USED -Oz, AND -Oz IS A REGRESSION HERE. Measured 2026-09-23 on the shipped
# wasm (.issues/135 §6.1), against what Cloudflare actually serves — brotli at
# roughly quality 4, not the library default of 11:
#
#     level   raw bytes              br4 bytes (what a user downloads)
#     none    5,478,875              1,646,742
#     -Oz     5,048,846  (-430,029)  1,650,371  (+3,629  WORSE)
#     -O2     5,334,606  (-144,269)  1,637,846  (-8,896  better)
#     -O3     5,262,260              1,643,638  (-3,104)
#     -O4     5,266,407              1,653,103  (+6,361  WORSE)
#
# -Oz shrinks the raw file by 7.8% and grows the transfer, because optimizing
# for size folds away exactly the repetition brotli feeds on. Judging a wasm
# change by `ls -l` is how that looks like a win. The flags below are now -O2,
# the only level that helped, and it is worth 8.9 KB — about a third of the
# frontend size gate's warn line, which is why nobody has bothered wiring this
# up. Re-measure with scripts/verify/frontend_size_budget.sh, not with ls.

set -euo pipefail

# --- Config ---
DIST_DIR="$(cd "$(dirname "$0")" && pwd)/dist"
# An array, not a string: these are separate argv words for wasm-opt, and an
# unquoted string expansion to achieve that is a word-splitting trap (SC2086).
WASM_OPT_FLAGS=(-O2 --enable-bulk-memory-opt --enable-nontrapping-float-to-int)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*"; }

# --- Check dependencies ---
check_deps() {
    local missing=0

    if ! command -v wasm-opt &>/dev/null; then
        err "wasm-opt not found. Install: brew install binaryen"
        missing=1
    else
        ok "wasm-opt $(wasm-opt --version 2>/dev/null || echo 'found')"
    fi

    if ! command -v wasm-strip &>/dev/null; then
        warn "wasm-strip not found. Install: brew install wabt"
        warn "Script will continue without stripping (wasm-opt alone still helps)"
    else
        ok "wasm-strip found"
    fi

    if [[ $missing -eq 1 ]]; then
        err "Missing required dependencies. Aborting."
        exit 1
    fi
}

# --- Find WASM file ---
find_wasm() {
    local input="${1:-}"

    if [[ -n "$input" ]]; then
        if [[ -f "$input" ]]; then
            echo "$input"
            return
        fi
        err "Specified file not found: $input"
        exit 1
    fi

    # Auto-find: prefer *_bg.wasm in dist/
    local found
    found="$(find "$DIST_DIR" -maxdepth 1 -name '*_bg.wasm' 2>/dev/null | head -1)"
    if [[ -z "$found" ]]; then
        found="$(find "$DIST_DIR" -maxdepth 1 -name '*.wasm' 2>/dev/null | head -1)"
    fi

    if [[ -z "$found" ]]; then
        err "No .wasm file found in ${DIST_DIR}/"
        err "Build first with: cd frontend-leptos && cargo leptos build --release"
        exit 1
    fi

    echo "$found"
}

# --- Human-readable size ---
human_size() {
    local bytes
    bytes="$(stat -f%z "$1" 2>/dev/null || stat -c%s "$1" 2>/dev/null)"
    if [[ bytes -ge 1048576 ]]; then
        echo "$(echo "scale=2; $bytes / 1048576" | bc)MB"
    elif [[ bytes -ge 1024 ]]; then
        echo "$(echo "scale=1; $bytes / 1024" | bc)KB"
    else
        echo "${bytes}B"
    fi
}

# --- Main ---
main() {
    local wasm_file
    wasm_file="$(find_wasm "${1:-}")"

    info "Target: ${wasm_file}"

    local before_bytes
    before_bytes="$(stat -f%z "$wasm_file")"
    info "Before:  $(human_size "$wasm_file") (${before_bytes} bytes)"

    local tmpfile
    tmpfile="$(mktemp "${wasm_file}.XXXXXX")"
    trap 'rm -f "$tmpfile"' EXIT

    # Step 1: wasm-opt
    info "Running wasm-opt ${WASM_OPT_FLAGS[*]} ..."
    if ! wasm-opt "${WASM_OPT_FLAGS[@]}" -o "$tmpfile" "$wasm_file"; then
        err "wasm-opt failed"
        exit 1
    fi
    ok "wasm-opt complete"

    # Step 2: wasm-strip (optional)
    if command -v wasm-strip &>/dev/null; then
        info "Running wasm-strip ..."
        if wasm-strip "$tmpfile"; then
            ok "wasm-strip complete"
        else
            warn "wasm-strip failed (non-fatal)"
        fi
    fi

    local after_bytes
    after_bytes="$(stat -f%z "$tmpfile")"

    # Replace original
    mv "$tmpfile" "$wasm_file"
    trap - EXIT

    # Report
    local saved=$((before_bytes - after_bytes))
    local pct
    pct="$(echo "scale=1; $saved * 100 / $before_bytes" | bc)"

    echo ""
    echo "=========================================="
    ok "WASM optimization complete!"
    echo "  Before:  $(human_size "$wasm_file") was ${before_bytes} bytes"
    echo "  After:   $(human_size "$wasm_file") now ${after_bytes} bytes"
    echo "  Saved:   ${saved} bytes (${pct}%)"
    echo "=========================================="
}

check_deps
main "$@"
