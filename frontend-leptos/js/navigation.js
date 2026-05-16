/**
 * Navigation and localStorage helpers.
 *
 * Provides CSP-safe alternatives to js_sys::eval() for:
 * - Saving/loading registration progress from localStorage
 * - Navigating to different pages (deposit, ticket, etc.)
 * - Reading clipboard text
 *
 * Imported via `#[wasm_bindgen(module = "/js/navigation.js")]` in Rust.
 * This avoids js_sys::eval() which would require 'unsafe-eval' in CSP.
 */

/**
 * Save registration progress to localStorage.
 *
 * @param {string} attendeeId
 * @param {string} eventId
 * @param {string} slug
 */
export function saveProgress(attendeeId, eventId, slug) {
  try {
    var data = JSON.stringify({
      attendee_id: attendeeId,
      event_id: eventId,
      slug: slug,
    });
    localStorage.setItem("bethere_progress", data);
  } catch (e) {
    console.error("[navigation] saveProgress error:", e);
  }
}

/**
 * Load registration progress from localStorage.
 *
 * Returns the stored JSON string, or null if not found.
 *
 * @returns {string|null}
 */
export function loadProgress() {
  try {
    var val = localStorage.getItem("bethere_progress");
    return val || null;
  } catch (e) {
    console.error("[navigation] loadProgress error:", e);
    return null;
  }
}

/**
 * Navigate to a relative URL path.
 *
 * Uses window.location.href assignment for full page navigation.
 *
 * @param {string} path - Relative URL like "/deposit/abc?event_id=xyz"
 */
export function navigateTo(path) {
  window.location.href = path;
}

/**
 * Read text from the clipboard.
 *
 * Returns a Promise that resolves to the clipboard text, or empty string
 * on failure.
 *
 * @returns {Promise<string>}
 */
export function readClipboardText() {
  if (navigator.clipboard && navigator.clipboard.readText) {
    return navigator.clipboard.readText().catch(function () {
      return "";
    });
  }
  return Promise.resolve("");
}
