#!/usr/bin/env bash
# Scan shipped wasm/js artifacts for build-host paths and secret-shaped strings.
#
# Ported from gist-rs/riir-reflex scripts/binary_leak_scan.sh (MIT) for
# .plans/031. The symbol-table arm is dropped: these are wasm modules, not
# native binaries.
#
#   scripts/verify/wasm_leak_scan.sh [--report-only] <file>...
#   scripts/verify/wasm_leak_scan.sh --self-test
#
# Classes:
#   paths    /Users/..., /home/..., or an uppercase drive letter plus two path
#            segments. Panic-location strings from std and the cargo registry
#            publish the builder's username and toolchain. The fix is
#            --remap-path-prefix (after RTM#6, .plans/031 §3).
#   secrets  ghp_ / github_pat_ / AKIA / sk- / xox?- / Slack webhook / PEM
#            private key. Always fatal, even with --report-only.
# Not checked: repo-relative panic paths (module names only).
#
# Exit: 0 clean (or paths only, with --report-only) · 1 leak · 2 usage error,
# a missing file, or zero files scanned.
set -euo pipefail

path_re='/(Users|home)/[A-Za-z0-9._-]+/[A-Za-z0-9._/-]+|[A-Z]:[\\/][A-Za-z0-9_. -]+[\\/][A-Za-z0-9_. -]+'
secret_re='ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{10,}|hooks\.slack\.com/services/T[A-Za-z0-9/]+|-----BEGIN [A-Z ]*PRIVATE KEY-----'

scan() {
  local report_only="$1"
  shift
  [[ $# -ge 1 ]] || { echo "❌ no files to scan" >&2; return 2; }
  local path_hits=0 secret_hits=0 file hits count
  for file in "$@"; do
    [[ -f "$file" ]] || { echo "❌ missing file: $file" >&2; return 2; }
    hits="$(LC_ALL=C grep -a -o -E "$path_re" "$file" | sort -u || true)"
    if [[ -n "$hits" ]]; then
      count="$(printf '%s\n' "$hits" | wc -l | tr -d ' ')"
      echo "⚠️  $file: $count unique build-host path(s), e.g."
      printf '%s\n' "$hits" | sed -n '1,5p' | sed 's/^/     /'
      path_hits=$((path_hits + count))
    fi
    hits="$(LC_ALL=C grep -a -o -E "$secret_re" "$file" | sort -u || true)"
    if [[ -n "$hits" ]]; then
      count="$(printf '%s\n' "$hits" | wc -l | tr -d ' ')"
      # Print only a prefix: the scan must not re-leak what it found.
      echo "❌ $file: $count secret-shaped string(s):"
      printf '%s\n' "$hits" | cut -c1-8 | sed 's/$/…/; s/^/     /'
      secret_hits=$((secret_hits + count))
    fi
  done
  if [[ "$secret_hits" -gt 0 ]]; then
    echo "❌ leak scan: secret-shaped strings in $# file(s). Do not ship."
    return 1
  fi
  if [[ "$path_hits" -gt 0 && "$report_only" != "true" ]]; then
    echo "❌ leak scan: $path_hits build-host path(s) in $# file(s)."
    return 1
  fi
  if [[ "$path_hits" -gt 0 ]]; then
    echo "⚠️  leak scan (report-only): $path_hits build-host path(s) in $# file(s); no secrets."
    return 0
  fi
  echo "✅ leak scan: $# file(s) clean"
}

self_test() {
  local dir rc failed=0
  dir="$(mktemp -d)"
  trap 'rm -rf "$dir"' RETURN
  printf 'plain bytes\0src/lib.rs\0' > "$dir/clean.wasm"
  printf 'x\0/Users/alice/.cargo/registry/src/foo.rs\0' > "$dir/path.wasm"
  printf 'x\0AKIA%s\0' "ABCDEFGHIJKLMNOP" > "$dir/secret.wasm"
  expect() {
    local want="$1" label="$2"
    shift 2
    rc=0
    scan "$@" >/dev/null 2>&1 || rc=$?
    if [[ "$rc" -eq "$want" ]]; then
      echo "  ok  $label → $rc"
    else
      echo "  ❌  $label → $rc, expected $want"
      failed=1
    fi
  }
  expect 0 "clean file" false "$dir/clean.wasm"
  expect 1 "path, strict" false "$dir/path.wasm"
  expect 0 "path, report-only" true "$dir/path.wasm"
  expect 1 "secret, strict" false "$dir/secret.wasm"
  expect 1 "secret, report-only" true "$dir/secret.wasm"
  expect 1 "one leak among clean files" false "$dir/clean.wasm" "$dir/path.wasm"
  expect 2 "missing file" false "$dir/nope.wasm"
  expect 2 "zero files" false
  [[ "$failed" -eq 0 ]] || { echo "❌ self-test failed"; return 1; }
  echo "✅ self-test: 8/8"
}

case "${1:-}" in
  --self-test) self_test ;;
  --report-only) shift; scan true "$@" ;;
  -*) echo "usage: $0 [--report-only] <file>... | --self-test" >&2; exit 2 ;;
  *) scan false "$@" ;;
esac
