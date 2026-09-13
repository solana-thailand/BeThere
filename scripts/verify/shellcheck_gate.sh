#!/usr/bin/env bash
# ShellCheck gate for every tracked shell script.
#
# Fail-CLOSED: it checks everything under the repo and subtracts an explicit
# deferral list, so a newly added script is gated automatically instead of
# being silently skipped. Issue #065 exists because a shell-quoting defect in
# worker/deploy.sh went unnoticed for months.
#
# Usage: bash scripts/verify/shellcheck_gate.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

# Deferral list — EMPTY, and it should stay that way. It held six e2e scripts
# whose SC2034 unused variables each marked an assertion that was written and
# never wired up; all six are resolved in
# .issues/072_e2e_script_dead_assertions.md, so every script in the repo is now
# gated. Never add an entry here to silence a new finding: fix the script, or
# suppress the single line with a `# shellcheck disable=` that states why.
DEFERRED=()

is_deferred() {
  local candidate="$1" entry
  # ${arr[@]+"${arr[@]}"} keeps an empty array from tripping `set -u` on the
  # bash 3.2 that ships with macOS.
  for entry in ${DEFERRED[@]+"${DEFERRED[@]}"}; do
    [ "$candidate" = "$entry" ] && return 0
  done
  return 1
}

targets=()
while IFS= read -r script; do
  script="${script#./}"
  is_deferred "$script" || targets+=("$script")
done < <(find . -name '*.sh' -type f -not -path './node_modules/*' -not -path '*/node_modules/*' -not -path './target/*' | sort)

if [ "${#targets[@]}" -eq 0 ]; then
  echo "❌ No shell scripts found — the gate would pass vacuously."
  exit 1
fi

# Print the version. ShellCheck codes move between releases (a trap-invoked
# function is SC2317 before 0.11 and SC2329 from 0.11), so a gate that is green
# locally can be red in CI for no reason visible in the diff. CI pins the
# version; this line makes any mismatch obvious in the log.
echo "🔍 ShellCheck $(shellcheck --version | awk '/^version:/ {print $2}') over ${#targets[@]} script(s); ${#DEFERRED[@]} deferred."
# `-x` follows `source`d files. `scripts/e2e/lib/` (Issue 076) is sourced
# through "$SCRIPT_DIR/...", which ShellCheck cannot resolve statically; each
# call site carries a `# shellcheck source=` directive, and without -x every one
# of them raises SC1091 and the shared variables read as unassigned.
# `--source-path=SCRIPTDIR` resolves those directives relative to the sourcing
# script rather than this gate's CWD (the repo root).
shellcheck -x --source-path=SCRIPTDIR "${targets[@]}"
echo "✅ ShellCheck clean."
