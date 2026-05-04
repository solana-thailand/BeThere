/**
 * File upload helper module.
 * Reads files from input elements as data URLs for upload.
 */

/**
 * Read a file as base64 data URL.
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

    // Limit to 5MB
    if (file.size > 5 * 1024 * 1024) {
      reject(new Error('File size exceeds 5MB limit'));
      return;
    }

    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(new Error('Failed to read file'));
    reader.readAsDataURL(file);
  });
}
