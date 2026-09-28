#!/usr/bin/env bash
# staging_parity.sh — the production deploy gate: prod only gets code staging runs.
#
# Usage: bash scripts/verify/staging_parity.sh "<staging deployment message>"
#   The message is what `deploy.sh` writes on every deploy, `git:<sha>`, read
#   back by `npx wrangler deployments status --env staging --json`
#   (annotations."workers/message").
#
# Passes when the working tree is clean and HEAD has the same git TREE as the
# commit staging is running. Trees, not commits: a release merge on `main`
# is a different commit from the `develop` commit staging ran, with the same
# content.
#
# Why this replaced the flow-harness preflight (.issues/141): that gate could
# never pass, so every prod deploy used --force and the gate checked nothing.
# "The exact code has been running on staging" is what those --force reasons
# asserted by hand; this makes the machine check it.
#
# Exit: 0 parity, 1 no parity (reason on stderr), 2 usage.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 \"git:<sha>\"" >&2
  exit 2
fi
message="$1"

fail() {
  echo "❌ staging parity: $1" >&2
  exit 1
}

case "$message" in
  git:*+dirty) fail "staging was deployed from a working tree with uncommitted changes (${message}), so no commit matches what it runs. Redeploy staging from a clean tree." ;;
  git:*) sha="${message#git:}" ;;
  *) fail "staging's current deployment has no git provenance (message: '${message}'). Deploy staging with worker/deploy.sh staging first." ;;
esac

git cat-file -e "${sha}^{commit}" 2>/dev/null \
  || fail "staging runs ${sha}, which this clone does not have. Run git fetch."

git diff --quiet HEAD -- \
  || fail "the working tree has uncommitted changes, so HEAD is not what would ship."

staging_tree=$(git rev-parse "${sha}^{tree}")
head_tree=$(git rev-parse "HEAD^{tree}")
[ "$staging_tree" = "$head_tree" ] \
  || fail "staging runs ${sha:0:8}, whose tree differs from HEAD $(git rev-parse --short HEAD). Deploy this code to staging first."

echo "✅ staging parity: HEAD $(git rev-parse --short HEAD) has the same tree as staging's ${sha:0:8}."
