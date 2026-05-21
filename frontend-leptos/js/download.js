/**
 * Download a data URL as a file.
 * @param {string} dataUrl - The data URL (e.g. base64 SVG/PNG).
 * @param {string} filename - The filename to save as.
 */
export function downloadDataUrl(dataUrl, filename) {
    const link = document.createElement('a');
    link.href = dataUrl;
    link.download = filename;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
}
