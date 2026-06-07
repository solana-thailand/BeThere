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

/**
 * Share an event URL using the Web Share API, with clipboard fallback.
 *
 * Returns true if the URL was shared or copied successfully.
 *
 * @param {string} title - Event name for the share dialog.
 * @param {string} url - Full URL to share.
 * @returns {Promise<boolean>}
 */
export function shareEvent(title, url) {
  if (navigator.share) {
    return navigator
      .share({ title: title, url: url })
      .then(function () {
        return true;
      })
      .catch(function () {
        // User cancelled or share failed — fall back to clipboard
        return copyToClipboardFallback(url);
      });
  }
  // No Web Share API — copy to clipboard
  return Promise.resolve(copyToClipboardFallback(url));
}

/**
 * Save developer profile fields to localStorage for auto-fill on future events.
 *
 * @param {string} json - JSON string of { name, fields: { key: value, ... } }
 */
export function saveDevProfile(json) {
  try {
    localStorage.setItem("bt_dev_profile", json);
  } catch (e) {
    console.error("[navigation] saveDevProfile error:", e);
  }
}

/**
 * Load saved developer profile fields from localStorage.
 *
 * @returns {string|null} JSON string or null
 */
export function loadDevProfile() {
  try {
    var val = localStorage.getItem("bt_dev_profile");
    return val || null;
  } catch (e) {
    console.error("[navigation] loadDevProfile error:", e);
    return null;
  }
}

/**
 * Internal: copy URL to clipboard using Clipboard API with textarea fallback.
 *
 * @param {string} text
 * @returns {boolean}
 */
function copyToClipboardFallback(text) {
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(
      function () {
        console.log("[share] copied successfully");
      },
      function (err) {
        console.error("[share] copy failed:", err);
      },
    );
    return true;
  }
  // Textarea fallback
  var textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  try {
    document.execCommand("copy");
    return true;
  } catch (e) {
    console.error("[share] fallback copy failed:", e);
    return false;
  } finally {
    document.body.removeChild(textarea);
  }
}
