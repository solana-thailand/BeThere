/**
 * Clipboard utility.
 *
 * Only export: `copyToClipboard()`, used by Rust modules via
 * `#[wasm_bindgen(module = "/js/clipboard.js")]`. QR codes are drawn in Rust
 * (`src/utils/qr_gen.rs`), so the old QRious helpers are gone.
 *
 * Uses wasm_bindgen module imports instead of `js_sys::eval()`
 * to avoid requiring `'unsafe-eval'` in CSP.
 */

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
