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

// `dist/jsQR.js`, not `jsQR.min.js`: the npm package ships no .min file, and
// jsdelivr minifies that path on the fly (unpkg 404s it), so its bytes are not
// a stable Subresource Integrity target. The hash matches the npm tarball.
// Bumping the version means recomputing it:
//   curl -sL <url> | openssl dgst -sha384 -binary | openssl base64 -A
var JSQR_URL = "https://cdn.jsdelivr.net/npm/jsqr@1.4.0/dist/jsQR.js";
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
