/**
 * File upload helper module.
 * Reads files from input elements as data URLs for upload.
 */

/**
 * Read a file as base64 data URL.
 * Limits file size to 3MB to avoid 413 Payload Too Large errors
 * (base64 encoding adds ~33% overhead, so 3MB file → ~4MB data URL,
 *  which stays well under typical server limits).
 * @param {HTMLInputElement} fileInput - The file input element
 * @returns {Promise<string|null>} Base64 data URL string, or null if no file selected
 */
export function readFileAsDataUrl(fileInput) {
  return new Promise((resolve, reject) => {
    if (!fileInput || !fileInput.files || fileInput.files.length === 0) {
      resolve(null);
      return;
    }
    const file = fileInput.files[0];

    // Limit to 3MB (base64 ~4MB, safe for server)
    if (file.size > 3 * 1024 * 1024) {
      reject(
        new Error(
          "File size exceeds 3MB limit. Please resize or compress your image and try again.",
        ),
      );
      return;
    }

    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(new Error("Failed to read file"));
    reader.readAsDataURL(file);
  });
}
