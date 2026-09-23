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
    promises.push(_loadScript("https://cdn.jsdelivr.net/npm/jsqr@1.4.0/dist/jsQR.min.js"));
  }

  return Promise.all(promises).then(function () {});
}

/**
 * Dynamically inject a <script> tag and return a Promise that resolves on load.
 *
 * @param {string} src - The script URL to load.
 * @returns {Promise<void>}
 */
function _loadScript(src) {
  return new Promise(function (resolve, reject) {
    var script = document.createElement("script");
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
