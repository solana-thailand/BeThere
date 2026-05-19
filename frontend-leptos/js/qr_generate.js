/**
 * QR code image generation module.
 *
 * Uses the QRious library (lazy-loaded by lazy_assets.js) to generate
 * QR code images as base64 data URLs for display in the frontend.
 *
 * Imported via `#[wasm_bindgen(module = "/js/qr_generate.js")]` in Rust.
 * This avoids `js_sys::eval()` which would require `'unsafe-eval'` in CSP.
 *
 * The QRious library creates a canvas element internally and renders
 * the QR code, then exports it as a PNG data URL.
 */

/**
 * Preload QR libraries (jsQR + QRious) so they are ready when needed.
 *
 * Call this on component mount for pages that render QR codes.
 * Deduplicates — safe to call multiple times.
 *
 * @returns {Promise<void>}
 */
export async function preloadQrLibraries() {
  try {
    var { loadQrLibraries } = await import("./lazy_assets.js");
    await loadQrLibraries();
  } catch (e) {
    console.error("[qr_generate] Failed to preload QR libraries:", e);
  }
}

/**
 * Generate a QR code image as a base64 PNG data URL.
 *
 * Requires QRious to be loaded (via preloadQrLibraries or startCamera preload).
 * Returns null if QRious hasn't been loaded yet — callers should ensure
 * preload is called on component mount before QR codes are needed.
 *
 * @param {string} text - The text to encode (e.g. claim URL).
 * @param {number} [size=200] - The size of the QR code in pixels.
 * @returns {string|null} Base64 data URL (e.g. "data:image/png;base64,...")
 *                        or null if QRious is not loaded.
 */
export function generateQrDataUrl(text, size) {
  if (typeof QRious === "undefined") {
    console.error(
      "[qr_generate] QRious library not loaded — call preloadQrLibraries() on mount",
    );
    return null;
  }

  var qrSize = size || 200;

  // padding: 0 — the QR fills the canvas tightly.
  // Visual centering is handled by CSS asymmetric padding on the .qr-wrapper,
  // which compensates for the QR code's bottom-right-heavy data pattern.
  var qr = new QRious({
    value: text,
    size: qrSize,
    level: "M",
    background: "#ffffff",
    foreground: "#000000",
    padding: 0,
  });

  return qr.toDataURL("image/png");
}

/**
 * Copy text to the system clipboard.
 *
 * Uses the Clipboard API with fallback to a temporary textarea element
 * for older browsers.
 *
 * @param {string} text - The text to copy.
 * @returns {boolean} True if copy succeeded, false otherwise.
 */
export function copyToClipboard(text) {
  // Try modern Clipboard API first
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(
      function () {
        console.log("[clipboard] copied successfully");
      },
      function (err) {
        console.error("[clipboard] copy failed:", err);
      },
    );
    return true;
  }

  // Fallback: create temporary textarea
  try {
    var textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.style.position = "fixed";
    textarea.style.left = "-9999px";
    textarea.style.top = "-9999px";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.focus();
    textarea.select();
    var success = document.execCommand("copy");
    document.body.removeChild(textarea);
    return success;
  } catch (e) {
    console.error("[clipboard] fallback copy failed:", e);
    return false;
  }
}
