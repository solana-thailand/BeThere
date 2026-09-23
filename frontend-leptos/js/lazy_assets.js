/**
 * Lazy loader for the jsQR fallback decoder.
 *
 * Browsers with the native BarcodeDetector (Chrome/Edge/Android) never run
 * jsQR, so they no longer download it. QRious used to be loaded here too; it
 * has been dead since the Rust QR generator replaced it (src/utils/qr_gen.rs),
 * and loading it made every scanner start wait on a second CDN fetch.
 *
 * Called from scanner.js. Deduplicates loads — multiple callers get the same
 * Promise.
 */

// Self-hosted: `vendor/jsqr-1.4.0.js` is `dist/jsQR.js` from the npm tarball,
// byte for byte, copied into dist/ by index.html's copy-file. It used to come
// from the jsdelivr CDN, which put a third-party round trip (and a CDN that
// venue Wi-Fi sometimes blocks) on every iOS scan, since iOS Safari has no
// BarcodeDetector. Same origin, so it is also cached by the service worker.
// The integrity pin is unchanged and worker/tests/vendored_jsqr_integrity.rs
// checks it against the vendored bytes. Bumping the version means replacing
// the file, renaming it (it is cached immutable) and recomputing:
//   openssl dgst -sha384 -binary vendor/jsqr-<v>.js | openssl base64 -A
var JSQR_URL = "/jsqr-1.4.0.js";
var JSQR_INTEGRITY =
  "sha384-b5Ya4Bq3qCyz39m2ISh+4DxjAIljdeFwK/BsXLuj9gugaNwAcj/ia15fxNZL9Nlx";

/**
 * Load jsQR if this browser needs it and it is not already present.
 *
 * @returns {Promise<void>} Resolves when the scanner has a decoder available.
 */
export function loadQrLibraries() {
  if (!window.__qrLibrariesPromise) {
    window.__qrLibrariesPromise = _doLoad();
  }
  return window.__qrLibrariesPromise;
}

/**
 * Internal load implementation.
 * Injects the jsQR <script> only when there is no native BarcodeDetector.
 *
 * @returns {Promise<void>}
 */
function _doLoad() {
  var promises = [];

  if (!("BarcodeDetector" in window) && typeof jsQR === "undefined") {
    promises.push(_loadScript(JSQR_URL, JSQR_INTEGRITY));
  }

  return Promise.all(promises).then(function () {});
}

/**
 * Dynamically inject a <script> tag and return a Promise that resolves on load.
 *
 * @param {string} src - The script URL to load.
 * @param {string} integrity - SRI hash; a CDN serving other bytes fails the load.
 * @returns {Promise<void>}
 */
function _loadScript(src, integrity) {
  return new Promise(function (resolve, reject) {
    var script = document.createElement("script");
    script.integrity = integrity;
    script.crossOrigin = "anonymous";
    script.src = src;
    script.onload = function () {
      resolve();
    };
    script.onerror = function () {
      reject(new Error("Failed to load script: " + src));
    };
    document.head.appendChild(script);
  });
}
