/**
 * PromptPay QR generation module.
 * Generates EMVCo QR string for Thai PromptPay payments.
 *
 * Reference: Thailand QR Payment Standard (EMVCo)
 * Tags must be in ascending numerical order for bank apps to accept.
 */

/**
 * Calculate CRC16-CCITT checksum (as required by EMVCo/Thai QR standard).
 */
function crc16(str) {
  let crc = 0xffff;
  for (let i = 0; i < str.length; i++) {
    crc ^= str.charCodeAt(i) << 8;
    for (let j = 0; j < 8; j++) {
      if (crc & 0x8000) {
        crc = (crc << 1) ^ 0x1021;
      } else {
        crc = crc << 1;
      }
    }
    crc &= 0xffff;
  }
  return crc.toString(16).toUpperCase().padStart(4, "0");
}

/**
 * Build an EMVCo TLV (Tag-Length-Value) field.
 */
function tlv(tag, value) {
  const len = value.length.toString().padStart(2, "0");
  return tag + len + value;
}

/**
 * Generate a PromptPay QR string.
 *
 * @param {string} promptpayId - Thai phone number (e.g., "0812345678") or national ID
 * @param {number} amount - Amount in THB (e.g., 500)
 * @param {string} [reference] - Optional reference label (e.g., event name, attendee ID).
 *   Encoded as EMVCo Tag 62 sub-tag 01 (Bill Number). Max 25 chars recommended.
 *   Displayed as "Reference" in most Thai banking apps when scanning the QR.
 * @returns {string|null} EMVCo QR string ready for QR encoding, or null if invalid
 */
export function generatePromptPayQr(promptpayId, amount, reference) {
  if (!promptpayId || promptpayId.trim() === "") {
    return null;
  }

  // Sanitize: keep only digits
  const cleanId = promptpayId.replace(/\D/g, "");

  // Format the target and determine sub-tag based on dtinth/promptpay-qr logic:
  //   - eWallet (>=15 digits): tag 03, value as-is
  //   - National ID (13 digits): tag 02, value as-is
  //   - Phone (<13 digits): tag 01, strip leading 0, prepend 66, pad to 13 chars
  let idTag;
  let formattedId;

  if (cleanId.length >= 15) {
    // eWallet ID
    idTag = "03";
    formattedId = cleanId;
  } else if (cleanId.length >= 13) {
    // National ID (13 digits)
    idTag = "02";
    formattedId = cleanId;
  } else {
    // Phone number: strip leading 0, prepend 66, left-pad with zeros to 13 chars
    idTag = "01";
    formattedId = ("0000000000000" + cleanId.replace(/^0/, "66")).slice(-13);
  }

  // Build Merchant Account Info (Tag 29)
  const merchantAccountInfo =
    tlv("00", "A000000677010111") + // AID for PromptPay (not Bill Payment)
    tlv(idTag, formattedId);

  // Point of Initiation: 11 = dynamic (with amount), 12 = static (no amount)
  const hasAmount = amount && amount > 0;
  const pointOfInitiation = hasAmount ? "11" : "12";

  // Build payload in strict ascending tag order per EMVCo spec.
  // Tag 00 — Payload Format Indicator
  // Tag 01 — Point of Initiation Method
  // Tag 29 — Merchant Account Information (PromptPay)
  // Tag 53 — Transaction Currency (764 = THB)
  // Tag 54 — Transaction Amount (only when amount > 0)
  // Tag 58 — Country Code (TH)
  let payload = "";
  payload += tlv("00", "01"); // Payload Format Indicator
  payload += tlv("01", pointOfInitiation); // Point of Initiation
  payload += tlv("29", merchantAccountInfo); // Merchant Account Info
  payload += tlv("53", "764"); // Currency: THB

  if (hasAmount) {
    payload += tlv("54", amount.toFixed(2)); // Transaction Amount
  }

  payload += tlv("58", "TH"); // Country Code

  // Tag 62 — Additional Data Field Template (optional reference/note)
  // Sub-tag 01: Bill Number / Reference Label (shown as "Reference" in banking apps)
  // Must appear before Tag 63 (CRC) for ascending tag order.
  if (reference && reference.trim() !== "") {
    // Truncate to 25 chars for broad bank compatibility
    const ref = reference.trim().substring(0, 25);
    payload += tlv("62", tlv("01", ref));
  }

  // Add CRC placeholder, then calculate checksum
  payload += "6304"; // CRC tag (63) + length (04)
  const checksum = crc16(payload);
  payload += checksum;

  return payload;
}
