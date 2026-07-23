/**
 * BeThere service worker — minimal PWA shell caching.
 *
 * Strategy:
 *  - Network-first for /api/* AND for HTML navigations. The SPA shell
 *    (index.html) references content-hashed WASM/JS that changes every build,
 *    so it must always be fetched fresh — a stale cached index.html whose
 *    <script src=...> points at no-longer-cached asset filenames produces a
 *    blank page (only fixed by a hard refresh).
 *  - Cache-first for static assets (WASM, JS, CSS, fonts, images) — they are
 *    content-hashed by trunk so cached versions are always valid for their URL.
 *  - Stale-while-revalidate for everything else (manifest, icons, etc.).
 *
 * Scope is intentionally minimal: this exists to satisfy PWA installability
 * criteria and give a usable offline shell, NOT to make BeThere fully
 * offline-capable. Wallet flows always require network.
 */

// Bump this on every deploy that ships a new WASM build. The activate handler
// evicts any cache whose name doesn't start with the current version, so old
// stale index.html / WASM entries from a previous build are purged on the next
// SW activation. Without this, stale-while-revalidate can serve an old
// index.html whose hashed <script src=...> references no-longer-existing WASM,
// producing a blank page until a hard refresh.
//
// v3 (2026-06): also made navigation/API fetches use `cache: "no-store"` so the
// SW never serves a browser-HTTP-cached (stale) index.html. See _headers for
// the matching edge-cache fix (no-store on the shell).
//
// v4 (2026-07): split API from navigation strategy. /api/* responses were
// being cached in SHELL_CACHE by networkFirst — combined with today's
// multi-deploy window this caused stale `deposit_enabled: false` to be
// re-served on reload, forcing a hard refresh to recover. API responses are
// now network-only (never cached, never served from cache on failure).
// Bumping the version also evicts any pre-existing stale API entries from
// SHELL_CACHE on activation.
var CACHE_VERSION = "bethere-v4";
var SHELL_CACHE = CACHE_VERSION + "-shell";
var ASSET_CACHE = CACHE_VERSION + "-assets";

// Files that constitute the SPA shell. The hashed filenames change per build,
// so we cache these by URL prefix at fetch time rather than hard-coding.
var SHELL_URLS = ["/", "/index.html", "/manifest.json"];

self.addEventListener("install", function (event) {
  console.log("[sw] install");
  event.waitUntil(
    caches
      .open(SHELL_CACHE)
      .then(function (cache) {
        // addAll fails atomically if any request fails; we tolerate missing
        // entries because the SPA shell can be populated lazily on first fetch.
        return Promise.all(
          SHELL_URLS.map(function (url) {
            return cache.add(url).catch(function (e) {
              console.warn("[sw] could not pre-cache", url, e);
            });
          }),
        );
      })
      .then(function () {
        return self.skipWaiting();
      }),
  );
});

self.addEventListener("activate", function (event) {
  console.log("[sw] activate");
  event.waitUntil(
    caches
      .keys()
      .then(function (keys) {
        return Promise.all(
          keys
            .filter(function (k) {
              return k !== SHELL_CACHE && k !== ASSET_CACHE;
            })
            .map(function (k) {
              console.log("[sw] evicting old cache", k);
              return caches.delete(k);
            }),
        );
      })
      .then(function () {
        return self.clients.claim();
      }),
  );
});

self.addEventListener("fetch", function (event) {
  var req = event.request;

  // Only handle GET; ignore non-GET (POST/PUT/DELETE) and browser extensions.
  if (req.method !== "GET") return;
  var url = new URL(req.url);

  // Skip cross-origin requests (fonts load from fonts.googleapis.com etc.
  // — let the browser handle them via normal HTTP cache).
  if (url.origin !== self.location.origin) return;

  // /api/* — network-only. API responses are always dynamic (deposit status,
  // attendee lists, auth state, etc.) and must NEVER be cached or re-served
  // from cache. Caching them caused a real-world stale-data bug: a 200 with
  // `deposit_enabled: false` from before a deploy was re-served on reload,
  // forcing users to hard-refresh. The backend already emits
  // `Cache-Control: no-store`, but that header does not stop the SW from
  // caching — only network-only strategy prevents it.
  if (url.pathname.startsWith("/api/")) {
    event.respondWith(networkOnly(req));
    return;
  }

  // Hashed static assets — cache-first (URLs are immutable per build)
  if (
    url.pathname.startsWith("/event-checkin-frontend-") ||
    url.pathname.startsWith("/snippets/") ||
    url.pathname.startsWith("/style-") ||
    url.pathname.endsWith(".wasm") ||
    url.pathname.endsWith(".css")
  ) {
    event.respondWith(cacheFirst(ASSET_CACHE, req));
    return;
  }

  // HTML navigations (full page loads via <a href> or window.location.href):
  // network-first. The SPA shell (index.html) references content-hashed WASM/JS
  // that changes every build, so serving a stale cached index.html yields a
  // blank page (old HTML references old asset filenames that may no longer be
  // cached). Network-first guarantees the latest index.html; falls back to
  // cache (or the generic SPA shell) when offline.
  if (req.mode === "navigate" || req.destination === "document") {
    event.respondWith(networkFirst(req));
    return;
  }

  // Everything else (manifest, icons, fonts, etc.) — stale-while-revalidate
  event.respondWith(staleWhileRevalidate(SHELL_CACHE, req));
});

// Network-only strategy for /api/* requests.
//
// Distinct from networkFirst because:
//   1. Never caches the response (API data is always dynamic and the backend
//      already emits Cache-Control: no-store — caching here would risk
//      re-serving pre-deploy state).
//   2. Never falls back to cache on network failure (a stale API response
//      is worse than no response — the calling code renders an error state,
//      which is the correct behavior when the network is down).
function networkOnly(req) {
  return fetch(req, { cache: "no-store" });
}

function networkFirst(req) {
  // `cache: "no-store"` bypasses the browser HTTP cache entirely, forcing a
  // fresh round-trip to the edge for navigations and API calls. The edge
  // serves index.html with `Cache-Control: no-store` (see _headers), so this
  // guarantees the SW always sees the current index.html — never a stale one
  // whose hashed <script> references have been purged by the latest deploy
  // (the blank-page-after-deploy bug).
  var isNav = req.mode === "navigate" || req.destination === "document";
  return fetch(req, { cache: "no-store" })
    .then(function (res) {
      // Only cache successful responses. A 3xx/4xx/5xx must NOT shadow the
      // network — otherwise a transient error response gets cached and served
      // as a stale blank page on the next navigation.
      if (res && res.status === 200 && res.type !== "opaqueredirect") {
        var clone = res.clone();
        caches.open(SHELL_CACHE).then(function (cache) {
          cache.put(req, clone);
        });
      }
      if (isNav) {
        console.log(
          "[sw] nav served from network:",
          new URL(req.url).pathname,
          "status=" + (res && res.status),
        );
      }
      return res;
    })
    .catch(function (err) {
      // Offline — try cache, then fall back to SPA shell
      console.warn(
        "[sw] nav fetch failed, falling back to cache:",
        new URL(req.url).pathname,
        err,
      );
      return caches.match(req).then(function (cached) {
        return cached || caches.match("/index.html");
      });
    });
}

function cacheFirst(cacheName, req) {
  return caches.match(req).then(function (cached) {
    return (
      cached ||
      fetch(req).then(function (res) {
        if (res && res.status === 200) {
          var clone = res.clone();
          caches.open(cacheName).then(function (cache) {
            cache.put(req, clone);
          });
        }
        return res;
      })
    );
  });
}

function staleWhileRevalidate(cacheName, req) {
  return caches.open(cacheName).then(function (cache) {
    return cache.match(req).then(function (cached) {
      var fetchPromise = fetch(req)
        .then(function (res) {
          if (res && res.status === 200) {
            cache.put(req, res.clone());
          }
          return res;
        })
        .catch(function () {
          // Offline and no fresh response — return cached or SPA shell
          return cached || caches.match("/index.html");
        });
      return cached || fetchPromise;
    });
  });
}
