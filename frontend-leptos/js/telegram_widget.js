/**
 * Telegram Login Widget interop.
 *
 * Telegram's login button is a third-party <script> that injects an iframe.
 * We mount it into a container and, on successful auth, POST the signed user
 * payload to /api/auth/telegram/verify (the worker verifies the HMAC with the
 * bot token). On success we reload so the profile re-fetches with the verified
 * handle.
 *
 * Requires CSP: script-src https://telegram.org; frame-src https://oauth.telegram.org
 *
 * Imported via `#[wasm_bindgen(module = "/js/telegram_widget.js")]`.
 */

/**
 * Mount the Telegram Login Widget button into `containerId` for `botUsername`.
 * Idempotent — a second call for an already-mounted container is a no-op.
 *
 * @param {string} containerId - id of the element to render the button into
 * @param {string} botUsername - bot username without '@'
 */
export function mountTelegramWidget(containerId, botUsername) {
  if (!botUsername) {
    console.warn("[telegram_widget] no bot username configured");
    return;
  }
  // The container is rendered by the WASM view; it may not exist the instant
  // this is called, so retry briefly.
  let attempts = 0;
  const attempt = () => {
    const container = document.getElementById(containerId);
    if (!container) {
      if (attempts++ < 25) {
        setTimeout(attempt, 100);
      } else {
        console.warn("[telegram_widget] container not found after retries:", containerId);
      }
      return;
    }
    if (container.dataset.tgMounted === "1") {
      return;
    }
    console.log("[telegram_widget] mounting widget for bot:", botUsername);
    doMount(container, botUsername);
  };
  attempt();
}

function doMount(container, botUsername) {
  container.dataset.tgMounted = "1";

  // Use the REDIRECT flow (data-auth-url), not data-onauth. The onauth callback
  // is parsed by the widget via eval(), which our CSP forbids (no unsafe-eval).
  // With data-auth-url, Telegram redirects the top window back to our callback
  // endpoint with the signed params — no eval needed.
  const authUrl = window.location.origin + "/api/auth/telegram/callback";

  const s = document.createElement("script");
  s.async = true;
  s.src = "https://telegram.org/js/telegram-widget.js?22";
  s.setAttribute("data-telegram-login", botUsername);
  s.setAttribute("data-size", "large");
  s.setAttribute("data-radius", "10");
  s.setAttribute("data-auth-url", authUrl);
  s.setAttribute("data-request-access", "write");
  container.appendChild(s);
}
