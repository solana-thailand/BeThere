/**
 * Lazy asset loader for QR libraries.
 *
 * Dynamically loads jsQR (~40 KB) and QRious (~15 KB) only when needed,
 * instead of blocking every page load with synchronous <script> tags.
 *
 * Called from scanner.js and qr_generate.js before using the libraries.
 * Deduplicates loads — multiple callers get the same Promise.
 */

/**
 * Load jsQR and QRious libraries if not already present.
 *
 * @returns {Promise<void>} Resolves when both libraries are available on `window`.
 */
export function loadQrLibraries() {
  if (!window.__qrLibrariesPromise) {
    window.__qrLibrariesPromise = _doLoad();
  }
  return window.__qrLibrariesPromise;
}

/**
 * Internal load implementation.
 * Creates <script> tags for both libraries and resolves when both are loaded.
 * Skips any library already present (e.g. from a previous call or cache).
 *
 * @returns {Promise<void>}
 */
function _doLoad() {
  var promises = [];

  if (typeof jsQR === "undefined") {
    promises.push(_loadScript("https://cdn.jsdelivr.net/npm/jsqr@1.4.0/dist/jsQR.min.js"));
  }

  if (typeof QRious === "undefined") {
    promises.push(_loadScript("https://cdn.jsdelivr.net/npm/qrious@4.0.2/dist/qrious.min.js"));
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
