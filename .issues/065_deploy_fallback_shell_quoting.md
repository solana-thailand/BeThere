# 065 — Remove shell interpolation from the production PUT fallback

Status: open

## Problem

`worker/deploy.sh` embeds sizeable Python programs inside double-quoted
`python3 -c` arguments. Shell expansion remains active inside those strings, so
backticks or future `$()` text can execute before Python starts. ShellCheck
currently reports this in comments within the metadata program.

The standard Wrangler deployment path does not enter this fallback, but the
fallback is production-capable and therefore must not interpret Python source
as shell syntax.

## Plan

1. Move the asset-manifest/upload and metadata generators into tracked Python
   modules with explicit CLI arguments or environment inputs.
2. Pass tokens through environment variables without printing them.
3. Add unit tests for MIME mapping and metadata binding completeness.
4. Run ShellCheck over `worker/deploy.sh` in CI.
5. Exercise the fallback against an isolated Worker before enabling it for
   production again.
