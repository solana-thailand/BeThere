/**
 * PromptPay QR generation module.
 * Generates EMVCo QR string for Thai PromptPay payments.
 */

/**
 * Calculate CRC16-CCITT checksum (as required by EMVCo/Thai QR standard).
 */
function crc16(str) {
  let crc = 0xFFFF;
  for (let i = 0; i < str.length; i++) {
    crc ^= str.charCodeAt(i) << 8;
    for (let j = 0; j < 8; j++) {
      if (crc & 0x8000) {
        crc = (crc << 1) ^ 0x1021;
      } else {
        crc = crc << 1;
      }
    }
    crc &= 0xFFFF;
  }
  return crc.toString(16).toUpperCase().padStart(4, '0');
}

/**
 * Build an EMVCo TLV tag.
 */
function tlv(tag, value) {
  const len = value.length.toString().padStart(2, '0');
  return tag + len + value;
}

/**
 * Generate a PromptPay QR string.
 * @param {string} promptpayId - Thai phone number (e.g., "0812345678") or national ID
 * @param {number} amount - Amount in THB (e.g., 500)
 * @returns {string|null} EMVCo QR string ready for QR encoding, or null if invalid
 */
export function generatePromptPayQr(promptpayId, amount) {
  if (!promptpayId || promptpayId.trim() === '') {
    return null;
  }

  // Clean the ID — remove dashes, spaces
  const cleanId = promptpayId.replace(/[-\s]/g, '');

  // Determine sub-tag: 01 = phone, 02 = national ID (13 digits starting with 1-8)
  let idTag = '01'; // default to phone
  if (cleanId.length === 13 && /^[1-8]/.test(cleanId)) {
    idTag = '02'; // national ID
  } else if (cleanId.length === 10) {
    idTag = '01'; // phone
  }

  // Build Merchant Account Info (Tag 29)
  const merchantAccountInfo =
    tlv('00', '0000000000000000') + // GUI ID for PromptPay
    tlv(idTag, cleanId);             // Phone or National ID

  // Build payload without CRC
  let payload = '';
  payload += tlv('00', '01');        // Payload Format Indicator
  payload += tlv('01', '12');        // Point of Initiation (12 = static if no amount, 11 = dynamic)
  payload += tlv('29', merchantAccountInfo); // Merchant Account Info

  if (amount && amount > 0) {
    payload += tlv('54', amount.toFixed(2)); // Transaction Amount
    // Change point of initiation to dynamic
    payload = payload.replace(tlv('01', '12'), tlv('01', '11'));
  }

  payload += tlv('58', 'TH');        // Country Code
  payload += tlv('53', '764');       // Currency (THB)

  // Add CRC placeholder, then calculate
  payload += '6304';                 // CRC tag + length 4
  const checksum = crc16(payload);
  payload += checksum;

  return payload;
}
