//! QR code image generation using the `qrcode` Rust crate.
//! Replaces the QRious JS library + CDN dependency.
//!
//! Generates a minimal PNG from the QR pixel matrix without pulling in
//! the full `image` crate, `flate2`, or `crc32fast`.
//!
//! Uses stored (uncompressed) DEFLATE blocks inside IDAT — QR PNGs are
//! small enough that compression barely helps, and this eliminates the
//! `miniz_oxide` + `simd-adler32` + `adler2` dependency tree (~200 KB WASM).

use qrcode::{types::Color as QrColor, QrCode};

/// Quiet zone width in modules (standard is 4).
const QUIET_ZONE_MODULES: u32 = 4;

/// Generate a QR code as a base64 PNG data URL.
///
/// Returns `None` if the input is too long for a QR code or encoding fails.
pub fn generate_qr_data_url(data: &str, size: u32) -> Option<String> {
    let code = QrCode::new(data.as_bytes()).ok()?;
    let modules = code.width() as u32;

    // Calculate pixels per module to hit the requested size.
    let total_modules = modules + QUIET_ZONE_MODULES * 2;
    let pixels_per_module = (size / total_modules).max(1);

    let png_bytes = encode_qr_png(&code, pixels_per_module)?;
    let b64 = base64_encode(&png_bytes);
    Some(format!("data:image/png;base64,{b64}"))
}

/// Encode a QR code to PNG bytes with the given pixel scale.
fn encode_qr_png(code: &QrCode, pixels_per_module: u32) -> Option<Vec<u8>> {
    let modules = code.width() as u32;
    let size = (modules + QUIET_ZONE_MODULES * 2) * pixels_per_module;
    let raw = qr_scanlines(code, size, pixels_per_module);

    // Wrap raw scanlines in a stored (uncompressed) DEFLATE block.
    // Format: 0x01 (final block, stored) + LEN (LE u16) + NLEN (LE u16) + data
    // This avoids pulling in miniz_oxide / flate2 for tiny QR images.
    let raw_len = raw.len();
    if raw_len > 65535 {
        // Would need multiple blocks — QR images should never be this large.
        return None;
    }
    let mut deflated = Vec::with_capacity(raw_len + 6);
    deflated.push(0x01); // BFINAL=1 (final block), BTYPE=00 (stored)
    deflated.extend_from_slice(&(raw_len as u16).to_le_bytes());
    deflated.extend_from_slice(&(raw_len as u16 ^ 0xFFFF).to_le_bytes());
    deflated.extend_from_slice(&raw);

    // Wrap DEFLATE in zlib (RFC 1950): CMF + FLG + DEFLATE + Adler-32
    // PNG IDAT requires zlib, not raw DEFLATE.
    let mut zlib = Vec::with_capacity(deflated.len() + 6);
    zlib.push(0x78); // CMF: deflate, window 32K
    zlib.push(0x01); // FLG: no dict, check (0x78*256+0x01) % 31 == 0
    zlib.extend_from_slice(&deflated);

    // Adler-32 checksum of the uncompressed data
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for &byte in &raw {
        s1 = (s1 + byte as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    let adler32 = (s2 << 16) | s1;
    zlib.extend_from_slice(&adler32.to_be_bytes());

    // Build minimal PNG: signature + IHDR + IDAT + IEND.
    let mut out = Vec::with_capacity(zlib.len() + 64);
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]); // 8-bit grayscale, deflate, no filter, no interlace.
    write_png_chunk(&mut out, b"IHDR", &ihdr);
    write_png_chunk(&mut out, b"IDAT", &zlib);
    write_png_chunk(&mut out, b"IEND", &[]);

    Some(out)
}

/// Build raw grayscale scanlines (one filter-byte prefix per row).
fn qr_scanlines(code: &QrCode, size: u32, pixels_per_module: u32) -> Vec<u8> {
    let modules = code.width() as i32;
    let quiet = QUIET_ZONE_MODULES as i32;
    let mut raw = Vec::with_capacity(((size + 1) * size) as usize);

    for y in 0..size {
        raw.push(0); // PNG filter type: none.
        let module_y = (y / pixels_per_module) as i32 - quiet;
        for x in 0..size {
            let module_x = (x / pixels_per_module) as i32 - quiet;
            let dark = module_x >= 0
                && module_x < modules
                && module_y >= 0
                && module_y < modules
                && code[(module_x as usize, module_y as usize)] != QrColor::Light;
            raw.push(if dark { 0 } else { 255 });
        }
    }

    raw
}

/// CRC32 lookup table (PNG uses CRC-32/ISO-HDLC).
static CRC_TABLE: [u32; 256] = generate_crc_table();

/// Compile-time CRC32 table generation.
const fn generate_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// Compute CRC32 for PNG chunk (type + data).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = CRC_TABLE[index] ^ (crc >> 8);
    }
    crc ^ 0xFFFFFFFF
}

/// Write a PNG chunk: length + type + data + CRC32.
fn write_png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());

    // CRC covers type + data
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    let crc = crc32(&crc_input);

    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Base64-encode without pulling in the `base64` crate.
/// Uses a lookup table — sufficient for the small PNGs we produce.
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push(TABLE[(n & 0x3F) as usize] as char);
        i += 3;
    }
    if data.len() - i == 2 {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push('=');
    } else if data.len() - i == 1 {
        let n = (data[i] as u32) << 16;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_qr_data_url_returns_valid_prefix() {
        let result = generate_qr_data_url("hello", 256);
        assert!(result.is_some());
        let url = result.unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn test_generate_qr_data_url_empty_string() {
        let result = generate_qr_data_url("", 256);
        assert!(result.is_some());
    }

    #[test]
    fn test_png_has_valid_signature() {
        let code = QrCode::new(b"test").unwrap();
        let png = encode_qr_png(&code, 4).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(png.windows(4).any(|w| w == b"IHDR"));
        assert!(png.windows(4).any(|w| w == b"IDAT"));
        assert!(png.windows(4).any(|w| w == b"IEND"));
    }

    #[test]
    fn test_png_valid_crc() {
        let code = QrCode::new(b"test").unwrap();
        let png = encode_qr_png(&code, 4).unwrap();
        // Verify each chunk has valid CRC by parsing manually.
        let mut pos = 8; // skip PNG signature
        while pos < png.len() {
            let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
            let kind = &png[pos + 4..pos + 8];
            let data = &png[pos + 8..pos + 8 + len];
            let stored_crc =
                u32::from_be_bytes(png[pos + 8 + len..pos + 12 + len].try_into().unwrap());

            let mut crc_input = Vec::with_capacity(4 + len);
            crc_input.extend_from_slice(kind);
            crc_input.extend_from_slice(data);
            let expected_crc = crc32(&crc_input);

            assert_eq!(
                stored_crc,
                expected_crc,
                "CRC mismatch for chunk {}",
                String::from_utf8_lossy(kind)
            );
            pos += 12 + len;
        }
    }

    #[test]
    fn test_base64_encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn test_base64_encode_known() {
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode(b"test"), "dGVzdA==");
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }

    #[test]
    fn test_png_is_square() {
        let code = QrCode::new(b"https://example.com").unwrap();
        let png = encode_qr_png(&code, 4).unwrap();
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!(width, height);
    }

    #[test]
    fn test_zlib_header_and_adler32() {
        let code = QrCode::new(b"test").unwrap();
        let png = encode_qr_png(&code, 4).unwrap();
        // Find IDAT chunk data
        let mut pos = 8;
        while pos < png.len() {
            let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
            let kind = &png[pos + 4..pos + 8];
            if kind == b"IDAT" {
                let idat = &png[pos + 8..pos + 8 + len];
                // Zlib header: CMF=0x78 (deflate, window 32K), FLG=0x01
                assert_eq!(idat[0], 0x78, "CMF should be 0x78");
                assert_eq!(idat[1], 0x01, "FLG should be 0x01");
                // Check header: (CMF*256 + FLG) % 31 == 0
                assert_eq!((0x78 * 256 + 0x01) % 31, 0, "zlib header check");
                // Stored DEFLATE block starts after zlib header
                assert_eq!(idat[2], 0x01, "BFINAL=1, BTYPE=00 (stored)");
                let stored_len = u16::from_le_bytes([idat[3], idat[4]]);
                let nlen = u16::from_le_bytes([idat[5], idat[6]]);
                assert_eq!(
                    stored_len ^ 0xFFFF,
                    nlen,
                    "NLEN should be one's complement of LEN"
                );
                // Last 4 bytes should be Adler-32 (big-endian), must be non-zero
                let adler_bytes = &idat[idat.len() - 4..];
                let adler = u32::from_be_bytes(adler_bytes.try_into().unwrap());
                assert_ne!(adler, 0, "Adler-32 should be non-zero");
                break;
            }
            pos += 12 + len;
        }
    }
}
