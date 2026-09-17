# 114 — R2 objects cached as `public` regardless of what they are

**Status:** fixed on `fix/114-r2-cache-policy`

## Found

`storage::serve_r2_object` hard-coded `Cache-Control: public, max-age=86400`
for every prefix it serves:

| Route | Router | Content | Was |
|---|---|---|---|
| `/api/storage/slips/{event}/{attendee}` | staff-only | THB payment slip (PII, financial) | `public, max-age=86400` |
| `/api/storage/refunds/{event}/{attendee}` | staff-only | refund transfer receipt | `public, max-age=86400` |
| `/api/storage/posters/{event}` | public | marketing poster | `public, max-age=86400`, no `ETag` |
| `/api/storage/badges/{event}` | public | badge SVG | `public, max-age=86400`, no `ETag` |

1. **Security.** RFC 9111 §3.5: `public` is the directive that lets a shared
   cache store a response to a request carrying `Authorization`. Any proxy
   honouring it could serve a staff member's view of a slip to someone else,
   and the browser keeps it on disk for a day after sign-out.
2. **Perf.** Posters carried no validator. RTM#6's poster is a 3,921,002-byte
   PNG, so every client downloaded it again in full once `max-age` lapsed.

## Fix

- `storage::Visibility { Public, Private }`, named by each route — the only
  code that knows what the bytes are — so a future private prefix cannot
  inherit `public` by default.
  - `Private` → `private, no-store`, no validator.
  - `Public` → `public, max-age=86400` plus R2's own `httpEtag` (no hashing).
- `get_bytes` split into `get_object` (locate, read `httpEtag`) and
  `read_body`, so a matching `If-None-Match` returns **304 before the body is
  copied into WASM memory**.
- `if_none_match_hits`: weak comparison, comma lists, `*` (RFC 9110 §13.1.2).

## Tests

`worker/tests/r2_cache_policy.rs` — the policy values, the `If-None-Match`
matcher, and a source guard that slips/refunds name `Private`, posters/badges
name `Public`, and no second `public, max-age` literal exists.

## Verify after deploy

```sh
curl -sI https://<host>/api/storage/posters/<event_id>          # etag present
curl -sI -H 'If-None-Match: <etag>' .../posters/<event_id>      # 304
curl -sI -H 'Authorization: Bearer <staff>' .../slips/<e>/<a>   # private, no-store
```

## Not in scope

The poster's size itself (3.9 MB PNG on the landing page) — `.issues/108`.
