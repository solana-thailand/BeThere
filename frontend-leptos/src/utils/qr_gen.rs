//! QR code image generation using the `qrcode` Rust crate.
//! Replaces the QRious JS library + CDN dependency.
//!
//! Generates a minimal PNG from the QR pixel matrix without pulling in
//! the full `image` crate. Adapted from the Solana Pay topup tool.

use std::io::Write;

use flate2::{write::ZlibEncoder, Compression};
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

    // Deflate-compress the raw scanlines.
    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
    zlib.write_all(&raw).ok()?;
    let compressed = zlib.finish().ok()?;

    // Build minimal PNG: signature + IHDR + IDAT + IEND.
    let mut out = Vec::new();
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]); // 8-bit grayscale, deflate, no filter, no interlace.
    write_png_chunk(&mut out, b"IHDR", &ihdr);
    write_png_chunk(&mut out, b"IDAT", &compressed);
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

/// Write a PNG chunk: length + type + data + CRC32.
fn write_png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);

    let mut hasher = crc32fast::Hasher::new();
    hasher.update(kind);
    hasher.update(data);
    out.extend_from_slice(&hasher.finalize().to_be_bytes());
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
}
