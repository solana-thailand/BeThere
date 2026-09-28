/**
 * Cloudflare Turnstile interop (.issues/170).
 *
 * The api.js script is loaded on first use only, so a page that never touches
 * a gated form never contacts challenges.cloudflare.com.
 *
 * Requires CSP: script-src and frame-src https://challenges.cloudflare.com
 *
 * Imported via `#[wasm_bindgen(module = "/js/turnstile.js")]`.
 */

const API_URL = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";

let loading = null;

function loadApi() {
  if (window.turnstile) return Promise.resolve(window.turnstile);
  if (!loading) {
    loading = new Promise((resolve, reject) => {
      const script = document.createElement("script");
      script.src = API_URL;
      script.async = true;
      script.onload = () => resolve(window.turnstile);
      script.onerror = () => {
        loading = null;
        reject(new Error("turnstile script failed to load"));
      };
      document.head.appendChild(script);
    });
  }
  return loading;
}

/**
 * Render a widget into `container`. `onToken(token)` fires on a solve;
 * `onClear()` fires when the token expires or the widget errors, so the form
 * disables submit again. Resolves to the widget id, or "" on failure.
 */
export async function renderTurnstile(container, siteKey, language, onToken, onClear) {
  try {
    const turnstile = await loadApi();
    return turnstile.render(container, {
      sitekey: siteKey,
      language,
      theme: "dark",
      size: "flexible",
      callback: (token) => onToken(token),
      "expired-callback": () => onClear(),
      "error-callback": () => onClear(),
    });
  } catch (error) {
    console.warn("[turnstile]", error);
    onClear();
    return "";
  }
}

/** Get a fresh token after the previous one was spent. */
export function resetTurnstile(widgetId) {
  if (widgetId && window.turnstile) window.turnstile.reset(widgetId);
}
