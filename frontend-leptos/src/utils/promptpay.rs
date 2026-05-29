//! PromptPay EMVCo QR string generation.
//!
//! Generates QR payload strings compatible with Thai banking apps.
//! Reference: Thailand QR Payment Standard (EMVCo).
//! Tags must be in ascending numerical order for bank apps to accept.

/// Calculate CRC16-CCITT checksum as required by EMVCo/Thai QR standard.
///
/// Returns an uppercase hex string, zero-padded to 4 characters.
fn crc16(data: &str) -> String {
    let mut crc: u32 = 0xFFFF;
    for byte in data.bytes() {
        crc ^= (byte as u32) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
        crc &= 0xFFFF;
    }
    format!("{:04X}", crc)
}

/// Build an EMVCo TLV (Tag-Length-Value) field.
fn tlv(tag: &str, value: &str) -> String {
    format!("{tag}{:02}{value}", value.len())
}

/// Generate a PromptPay EMVCo QR string.
///
/// Returns `None` if `promptpay_id` is empty or contains no digits.
///
/// # Arguments
/// * `promptpay_id` — Thai phone number (e.g. "0812345678") or national ID
/// * `amount` — Amount in THB (e.g. 500.0). Use 0.0 for static QR.
/// * `reference` — Optional reference label, shown as "Reference" in banking apps (max 25 chars)
pub fn generate_promptpay_qr(promptpay_id: &str, amount: f64, reference: &str) -> Option<String> {
    if promptpay_id.trim().is_empty() {
        return None;
    }

    // Sanitize: keep only digits
    let clean_id: String = promptpay_id
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    if clean_id.is_empty() {
        return None;
    }

    // Determine sub-tag and format based on ID length:
    //   eWallet (>=15 digits): tag 03, value as-is
    //   National ID (13 digits): tag 02, value as-is
    //   Phone (<13 digits): tag 01, strip leading 0, prepend 66, pad to 13
    let (id_tag, formatted_id) = if clean_id.len() >= 15 {
        ("03", clean_id.clone())
    } else if clean_id.len() >= 13 {
        ("02", clean_id.clone())
    } else {
        // Phone: strip leading 0, prepend 66, left-pad to 13 chars
        let stripped = clean_id.strip_prefix('0').unwrap_or(&clean_id);
        let padded = format!("{:0>13}", format!("66{stripped}"));
        ("01", padded)
    };

    // Build Merchant Account Info (Tag 29)
    let merchant_account_info = format!(
        "{}{}",
        tlv("00", "A000000677010111"),
        tlv(id_tag, &formatted_id)
    );

    // Point of Initiation: 11 = dynamic (with amount), 12 = static (no amount)
    let has_amount = amount > 0.0;
    let point_of_initiation = if has_amount { "11" } else { "12" };

    // Build payload in strict ascending tag order per EMVCo spec.
    let mut payload = String::new();
    payload.push_str(&tlv("00", "01")); // Payload Format Indicator
    payload.push_str(&tlv("01", point_of_initiation)); // Point of Initiation
    payload.push_str(&tlv("29", &merchant_account_info)); // Merchant Account Info
    payload.push_str(&tlv("53", "764")); // Currency: THB

    if has_amount {
        payload.push_str(&tlv("54", &format!("{:.2}", amount))); // Transaction Amount
    }

    payload.push_str(&tlv("58", "TH")); // Country Code

    // Tag 62 — Additional Data Field Template (optional reference/note)
    if !reference.trim().is_empty() {
        let ref_truncated: String = reference.trim().chars().take(25).collect();
        payload.push_str(&tlv("62", &tlv("01", &ref_truncated)));
    }

    // Append CRC: Tag 63, length 04, then checksum
    payload.push_str("6304");
    let checksum = crc16(&payload);
    payload.push_str(&checksum);

    Some(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16_empty() {
        assert_eq!(crc16(""), "FFFF");
    }

    #[test]
    fn test_crc16_known() {
        // Verify CRC16-CCITT against known test vector
        let result = crc16("123456789");
        assert_eq!(result, "29B1");
    }

    #[test]
    fn test_tlv() {
        assert_eq!(tlv("00", "01"), "000201");
        assert_eq!(tlv("29", "A000000677010111"), "2916A000000677010111");
    }

    #[test]
    fn test_empty_id_returns_none() {
        assert!(generate_promptpay_qr("", 100.0, "").is_none());
        assert!(generate_promptpay_qr("   ", 100.0, "").is_none());
        assert!(generate_promptpay_qr("abc", 100.0, "").is_none());
    }

    #[test]
    fn test_phone_number() {
        let result = generate_promptpay_qr("0812345678", 100.0, "").unwrap();
        // Sub-tag 01 (phone) with length 13, left-padded: 0066812345678
        assert!(result.contains("01130066812345678"));
    }

    #[test]
    fn test_national_id() {
        let result = generate_promptpay_qr("1234567890123", 500.0, "").unwrap();
        // Should use tag 02 for 13-digit national ID
        assert!(result.contains("0213")); // sub-tag 02 with length 13
        assert!(result.contains("1234567890123"));
    }

    #[test]
    fn test_static_qr_no_amount() {
        let result = generate_promptpay_qr("0812345678", 0.0, "").unwrap();
        assert!(result.contains("010212")); // Tag 01, length 02, value "12" (static)
        assert!(!result.contains("5406")); // No Tag 54 (amount)
    }

    #[test]
    fn test_dynamic_qr_with_amount() {
        let result = generate_promptpay_qr("0812345678", 500.0, "").unwrap();
        assert!(result.contains("010211")); // Tag 01, length 02, value "11" (dynamic)
        assert!(result.contains("5406500.00")); // Tag 54 with amount
    }

    #[test]
    fn test_reference_truncated() {
        let long_ref = "A".repeat(30);
        let result = generate_promptpay_qr("0812345678", 100.0, &long_ref).unwrap();
        // Tag 62 contains sub-tag 01 with 25-char truncated ref
        // Inner TLV: "01" + "25" + 25×'A' = 29 chars, so outer: "62" + "29" + inner = tag 62 length 29
        let tag_62_start = result.find("6229").expect("Tag 62 should be present");
        let tag_62 = &result[tag_62_start..tag_62_start + 4 + 29];
        assert!(tag_62.starts_with("62290125"));
    }

    #[test]
    fn test_starts_with_payload_format() {
        let result = generate_promptpay_qr("0812345678", 100.0, "").unwrap();
        assert!(result.starts_with("000201")); // Tag 00 = "01"
    }

    #[test]
    fn test_ends_with_crc() {
        let result = generate_promptpay_qr("0812345678", 100.0, "").unwrap();
        // Last 4 chars should be uppercase hex CRC
        let crc_part = &result[result.len() - 4..];
        assert!(crc_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_tag_order_ascending() {
        let result = generate_promptpay_qr("1234567890123", 100.0, "ref").unwrap();
        let tag_positions: Vec<usize> = ["00", "01", "29", "53", "54", "58", "62", "63"]
            .iter()
            .map(|tag| result.find(tag).unwrap_or(usize::MAX))
            .collect();
        for i in 1..tag_positions.len() {
            assert!(
                tag_positions[i] > tag_positions[i - 1],
                "Tags not in ascending order: {:?}",
                tag_positions
            );
        }
    }
}
