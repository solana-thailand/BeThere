/**
 * Read the mini-QR off a bank transfer slip image, in the browser.
 *
 * `.plans/033` W1: the QR holds the bank's transaction reference. Decoding it
 * here costs the worker no CPU (`.issues/134` rejected decoding in the worker
 * on the free plan's 10 ms cap). The text is sent with the upload and the
 * server re-parses it with the `domain` slip parser, CRC included, so what is
 * decoded here is a claim, never a fact.
 *
 * Never rejects: a slip without a readable QR is normal and must not stop the
 * upload. Resolves to the decoded text or null.
 */

// Longest side the image is scaled to before jsQR. Phone screenshots are
// ~2400 px tall; the mini-QR survives this scale and jsQR stays well under a
// second on a phone.
var MAX_SIDE = 1600;

/**
 * @param {string} dataUrl - The slip image as a data: URL.
 * @returns {Promise<string|null>}
 */
export function decodeSlipQr(dataUrl) {
  return _loadImage(dataUrl)
    .then(function (img) {
      return _detectNative(img).then(function (text) {
        return text !== null ? text : _detectJsQr(img);
      });
    })
    .catch(function (e) {
      console.warn("[slip_qr] no QR read:", e);
      return null;
    });
}

function _loadImage(dataUrl) {
  return new Promise(function (resolve, reject) {
    if (typeof dataUrl !== "string" || dataUrl.indexOf("data:image/") !== 0) {
      reject(new Error("not an image data URL"));
      return;
    }
    var img = new Image();
    img.onload = function () {
      resolve(img);
    };
    img.onerror = function () {
      reject(new Error("image did not decode"));
    };
    img.src = dataUrl;
  });
}

function _detectNative(img) {
  if (!("BarcodeDetector" in window)) {
    return Promise.resolve(null);
  }
  try {
    var detector = new window.BarcodeDetector({ formats: ["qr_code"] });
    return detector
      .detect(img)
      .then(function (codes) {
        return codes.length > 0 ? codes[0].rawValue : null;
      })
      .catch(function () {
        return null;
      });
  } catch (_e) {
    return Promise.resolve(null);
  }
}

// Dynamic, like scanner.js: lazy_assets.js reaches dist/ only through the
// build.sh copy, and a static import of a missing module would stop the whole
// app from booting instead of just this reader.
function _loadJsQr() {
  return import("./lazy_assets.js").then(function (m) {
    return m.loadJsQr();
  });
}

function _detectJsQr(img) {
  return _loadJsQr().then(function () {
    var scale = Math.min(1, MAX_SIDE / Math.max(img.naturalWidth, img.naturalHeight));
    var w = Math.max(1, Math.round(img.naturalWidth * scale));
    var h = Math.max(1, Math.round(img.naturalHeight * scale));
    var canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    var ctx = canvas.getContext("2d", { willReadFrequently: true });
    ctx.drawImage(img, 0, 0, w, h);
    var pixels = ctx.getImageData(0, 0, w, h);
    // Slips print a dark QR on a light background.
    var code = jsQR(pixels.data, w, h, { inversionAttempts: "dontInvert" });
    return code ? code.data : null;
  });
}
