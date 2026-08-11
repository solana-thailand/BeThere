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

  // Global callback the widget invokes with the signed user object.
  window.onBeThereTelegramAuth = async function (user) {
    try {
      const resp = await fetch("/api/auth/telegram/verify", {
        method: "POST",
        headers: { "content-type": "application/json" },
        credentials: "include",
        body: JSON.stringify(user),
      });
      if (resp.ok) {
        window.location.reload();
      } else {
        let msg = "Telegram verification failed. Please try again.";
        try {
          const body = await resp.json();
          if (body && body.error) msg = body.error;
        } catch (_) {}
        console.error("[telegram_widget] verify failed:", resp.status);
        window.alert(msg);
      }
    } catch (e) {
      console.error("[telegram_widget] verify error:", e);
      window.alert("Could not reach the server to verify Telegram. Please try again.");
    }
  };

  const s = document.createElement("script");
  s.async = true;
  s.src = "https://telegram.org/js/telegram-widget.js?22";
  s.setAttribute("data-telegram-login", botUsername);
  s.setAttribute("data-size", "large");
  s.setAttribute("data-radius", "10");
  s.setAttribute("data-onauth", "onBeThereTelegramAuth(user)");
  s.setAttribute("data-request-access", "write");
  container.appendChild(s);
}
