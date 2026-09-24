/**
 * Camera QR scanner module.
 *
 * Provides camera access, QR code detection (BarcodeDetector or jsQR fallback),
 * and result polling for the Leptos WASM frontend.
 *
 * Imported via `#[wasm_bindgen(module = "/js/scanner.js")]` in scanner.rs.
 * This avoids `js_sys::eval()` which would require `'unsafe-eval'` in CSP.
 *
 * State is communicated via window globals:
 * - `window.__qrResult`      — detected QR code string (or null)
 * - `window.__cameraError`   — error message string (or null)
 * - `window.__scannerActive` — boolean, true while scanner is running
 * - `window.__cameraStream`  — active MediaStream reference
 */

/**
 * Load QR libraries via dynamic import.
 *
 * Uses dynamic import() instead of static import so the browser doesn't
 * require lazy_assets.js to exist when scanner.js is first loaded.
 * This prevents MIME-type errors on pages that don't use the scanner.
 *
 * @returns {Promise<void>}
 */
async function _loadQrLibraries() {
  var { loadQrLibraries } = await import("./lazy_assets.js");
  return loadQrLibraries();
}

/**
 * Preload QR libraries so they are ready when the scanner starts.
 *
 * Call this when the Scanner page mounts (before calling startCamera)
 * to avoid a delay between camera start and QR library availability.
 *
 * @returns {Promise<void>}
 */
export function preloadQrLibraries() {
  return _loadQrLibraries();
}

/**
 * Start the camera and QR scanning loop.
 *
 * Requests camera access (rear-facing preferred), waits for #scanner-video
 * to be both present AND visible in the DOM, attaches the stream, and
 * starts a QR detection loop (BarcodeDetector API or jsQR canvas fallback).
 *
 * Results are stored in `window.__qrResult`; errors in `window.__cameraError`.
 * The Rust side polls these via checkQrResult() / checkCameraError().
 */
export function startCamera() {
  // Guard: skip if scanner is already active (prevents double-start race)
  if (window.__scannerActive) {
    console.log("[scanner] already active, skipping startCamera");
    return;
  }

  // Reset state
  window.__cameraError = null;
  window.__qrResult = null;
  window.__scannerActive = true;

  (async function () {
    try {
      // Fetch the decoder IN PARALLEL with the camera prompt. It used to be
      // awaited first, so the camera waited on a CDN round trip, and a slow or
      // blocked jsdelivr on venue Wi-Fi failed the whole scanner — even on
      // browsers with a native BarcodeDetector that never use jsQR. jsQR is
      // now same-origin (lazy_assets.js), but a failed load is still only
      // fatal below, on the one branch that needs jsQR.
      var decoderReady = _loadQrLibraries().catch(function (e) {
        console.warn("[scanner] jsQR fallback failed to load:", e);
      });
      console.log("[scanner] requesting camera access...");
      var stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: "environment" },
      });
      console.log(
        "[scanner] camera stream obtained, waiting for visible video element...",
      );

      // Wait for video element to exist AND be visible (parent not display:none).
      // Keeps checking while __scannerActive is true — no fixed timeout.
      // This handles DOM races where Leptos hasn't rendered the video yet,
      // or the video div is temporarily hidden during state transitions.
      var video = null;
      while (window.__scannerActive) {
        video = document.getElementById("scanner-video");
        if (video) {
          var el = video;
          var visible = true;
          while (el) {
            var style = window.getComputedStyle(el);
            if (style.display === "none" || style.visibility === "hidden") {
              visible = false;
              break;
            }
            el = el.parentElement;
          }
          if (visible) break;
          video = null; // not yet visible, keep waiting
        }
        await new Promise(function (r) {
          setTimeout(r, 200);
        });
      }

      // If scanner was stopped externally while waiting, clean up and exit.
      if (!video) {
        stream.getTracks().forEach(function (t) {
          t.stop();
        });
        console.log("[scanner] camera stopped while waiting for video element");
        return;
      }

      console.log("[scanner] video element visible, attaching stream");
      video.srcObject = stream;
      await video.play();
      window.__cameraStream = stream;
      console.log("[scanner] camera playing, starting QR detection");

      // Choose QR detection backend
      var hasBarcodeDetector = "BarcodeDetector" in window;
      if (!hasBarcodeDetector) await decoderReady;
      if (hasBarcodeDetector) {
        console.log("[scanner] using BarcodeDetector API");
        var detector = new BarcodeDetector({ formats: ["qr_code"] });
        (async function scanLoop() {
          while (window.__scannerActive) {
            try {
              if (video.readyState >= 2) {
                var results = await detector.detect(video);
                if (results.length > 0 && !window.__qrResult) {
                  window.__qrResult = results[0].rawValue;
                }
              }
            } catch {
              // detection error — ignore and retry
            }
            await new Promise(function (r) {
              setTimeout(r, 300);
            });
          }
        })();
      } else if (typeof jsQR === "function") {
        console.log("[scanner] using jsQR fallback");
        // This branch is every iPhone (Safari has no BarcodeDetector), so it
        // runs for a whole door shift. willReadFrequently keeps the canvas on
        // the CPU so getImageData is not a GPU readback per frame, and the
        // canvas is only resized when the video size changes (assigning
        // width/height reallocates and clears it, even to the same value).
        var canvas = document.createElement("canvas");
        var ctx = canvas.getContext("2d", { willReadFrequently: true });
        (async function scanLoop() {
          while (window.__scannerActive) {
            try {
              if (video.readyState >= 2) {
                if (
                  canvas.width !== video.videoWidth ||
                  canvas.height !== video.videoHeight
                ) {
                  canvas.width = video.videoWidth;
                  canvas.height = video.videoHeight;
                }
                ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
                var imageData = ctx.getImageData(
                  0,
                  0,
                  canvas.width,
                  canvas.height,
                );
                // Tickets are dark-on-light (domain qr::generator), so the
                // default "attemptBoth" second, inverted pass is pure cost.
                var code = jsQR(
                  imageData.data,
                  imageData.width,
                  imageData.height,
                  { inversionAttempts: "dontInvert" },
                );
                if (code && !window.__qrResult) {
                  window.__qrResult = code.data;
                }
              }
            } catch {
              // detection error — ignore and retry
            }
            await new Promise(function (r) {
              setTimeout(r, 300);
            });
          }
        })();
      } else {
        window.__cameraError =
          "QR scanning requires a modern browser. Please use Chrome, Edge, Safari, or Firefox.";
      }
    } catch (e) {
      console.error("[scanner] camera error:", e);
      window.__cameraError =
        e.message ||
        "Camera access denied. Please allow camera access and retry.";
    }
  })();
}

/**
 * Stop the camera stream and QR scanning loop.
 *
 * Stops all media tracks, clears scanning state, and detaches the stream
 * from the video element.
 */
export function stopCamera() {
  window.__scannerActive = false;
  window.__qrResult = null;
  if (window.__cameraStream) {
    window.__cameraStream.getTracks().forEach(function (t) {
      t.stop();
    });
    window.__cameraStream = null;
  }
  var video = document.getElementById("scanner-video");
  if (video) video.srcObject = null;
}

/**
 * Poll for a detected QR code value.
 *
 * Returns the raw QR string if one was detected since the last poll,
 * then clears it. Returns null if no QR code has been detected.
 *
 * @returns {string|null}
 */
export function checkQrResult() {
  var r = window.__qrResult;
  window.__qrResult = null;
  return r || null;
}

/**
 * Poll for camera errors set by the scanning loop.
 *
 * Returns an error message string if an error occurred, or null.
 * The error persists until the next startCamera() call.
 *
 * @returns {string|null}
 */
export function checkCameraError() {
  return window.__cameraError || null;
}

/**
 * Check if the scanner is currently active.
 *
 * Returns true between startCamera() and stopCamera() calls.
 *
 * @returns {boolean}
 */
export function isScannerActive() {
  return !!window.__scannerActive;
}
