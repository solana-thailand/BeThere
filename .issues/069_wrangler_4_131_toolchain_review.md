# Issue 069 — Wrangler 4.131.1 Toolchain Review

**Priority:** P2 maintenance
**Status:** Deferred pending normal dependency-age policy
**Scope:** local deployment tooling only; no Worker runtime behaviour change

## Context

An attempted update from Wrangler 4.99.0 to 4.131.1 changes the lockfile by
hundreds of lines. Its resolution selects `miniflare@5.20260911.0-alpha`, a
new workerd release, and related platform packages. It required adding those
fresh packages to `minimumReleaseAgeExclude` only to complete the installation.

That bypass is not an acceptable reason to run or commit the update. The
generated `worker/package.json`, `worker/pnpm-lock.yaml`,
`worker/pnpm-workspace.yaml`, and root `.pnpm-store/` must remain outside
unrelated application commits. The 4.131.1 CLI must not be used to deploy a
release until this issue is resolved.

## Decision

- Keep the committed 4.99.0 toolchain as the release baseline.
- Do not add fresh packages to `minimumReleaseAgeExclude` merely to upgrade
  Wrangler.
- Do not deploy an application change with an uncommitted toolchain update.
- Treat `.pnpm-store/` as generated local state; it is never source material.

## Completion criteria

- [ ] The desired Wrangler release and all resolved transitive packages satisfy
  the repository's normal release-age policy without new exclusions.
- [ ] Regenerate the lockfile from the committed workspace configuration and
  review the resulting dependency diff, especially Miniflare/workerd versions.
- [ ] Run `pnpm exec wrangler --version`, the normal Worker checks, and a
  staging deploy smoke check using the committed lockfile.
- [ ] Commit the package manifest, lockfile, and only any justified workspace
  policy change in a dedicated commit.
- [ ] Remove generated local package-manager state or ensure it remains
  untracked before opening the next application change.

## References

- [Operator Handover](../docs/operator-handover.md)
- [Staging Deploy Runbook](../docs/staging_deploy_runbook.md)
- `worker/pnpm-workspace.yaml`
