/**
 * Save a QR code image to device.
 *
 * On desktop browsers: triggers a file download via <a download>.
 * On iOS Safari: opens the image in a new tab so the user can
 *   long-press → "Save to Photos" (iOS doesn't support <a download> for data URLs).
 *
 * @param {string} dataUrl - The data URL (e.g. base64 PNG from QRious).
 * @param {string} filename - The filename to save as (desktop only).
 */
export function downloadDataUrl(dataUrl, filename) {
  // Detect iOS (iPhone/iPad/iPod) — these don't support <a download> for data URLs
  const isIos = /iPad|iPhone|iPod/.test(navigator.userAgent);

  if (isIos) {
    // Open as a full-page image — user can long-press → "Save to Photos"
    const win = window.open("", "_blank");
    if (win) {
      win.document.write(
        "<!DOCTYPE html><html><head><title>" +
          (filename || "QR Code") +
          "</title><style>" +
          "body{margin:0;display:flex;justify-content:center;align-items:center;min-height:100vh;background:#111;}" +
          "img{max-width:90vw;max-height:90vh;border-radius:12px;}" +
          "p{position:fixed;bottom:1rem;left:50%;transform:translateX(-50%);color:#999;font-family:system-ui;font-size:0.85rem;text-align:center;}" +
          "</style></head><body>" +
          '<img src="' +
          dataUrl +
          '" alt="QR Code" />' +
          "<p>Long press the image → Save to Photos</p>" +
          "</body></html>",
      );
      win.document.close();
    }
  } else {
    // Desktop / Android — standard download
    const link = document.createElement("a");
    link.href = dataUrl;
    link.download = filename;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  }
}
