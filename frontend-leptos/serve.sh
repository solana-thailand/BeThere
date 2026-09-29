#!/usr/bin/env bash
# Fast iteration loop: `trunk serve` on the fast-dev cargo profile.
#
# Usage:
#   bash serve.sh                # serves on :3001, proxies /api to :8787
#   bash serve.sh --port 3002    # extra args go to trunk serve
#   bash serve.sh --staff        # the staff shell (/admin, /staff, …)
#
# A one-line view edit rebuilds in ~10 s here, against ~120 s on the release
# profile. Three settings are needed together, none of them alone gets there:
#   - `--release false` selects the fast-dev profile (index.html's rust link;
#     Trunk 0.21 ignores --cargo-profile without that link);
#   - CARGO_INCREMENTAL=1, because ~/.cargo/config.toml sets incremental=false;
#   - RUSTC_WRAPPER= , because sccache refuses to run with incremental on.
# Production builds stay on `bash build.sh` (release profile, size gates).

set -euo pipefail

cd "$(dirname "$0")"

# Trunk reads NO_COLOR as a boolean (see build.sh).
if [[ -n "${NO_COLOR:-}" && "${NO_COLOR}" != "true" && "${NO_COLOR}" != "false" ]]; then
    export NO_COLOR=true
fi

export CARGO_INCREMENTAL=1
export RUSTC_WRAPPER=

# `bash serve.sh --staff` serves the staff shell (scanner, admin, …) instead:
# the staff feature plus the staff-only stylesheets (.issues/169).
if [[ "${1:-}" == "--staff" ]]; then
    shift
    python3 staff_shell_html.py
    exec ~/.cargo/bin/trunk serve --release false --features staff staff-shell.html "$@"
fi

exec ~/.cargo/bin/trunk serve --release false "$@"
