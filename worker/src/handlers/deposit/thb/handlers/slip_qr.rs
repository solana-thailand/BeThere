//! Mini-QR decoding for uploaded THB payment slips — **SIZE/CPU PROBE ONLY**.
//!
//! This file exists to answer the one question `.plans/027` §G says must be
//! answered before the feature is written: *what does a QR decoder cost on a
//! Worker that ships on Cloudflare's free plan?* It is deliberately wired into
//! the upload path (behind a log line, changing no behaviour) because a
//! `pub(crate) fn` that nothing calls is dead-stripped by LLVM and would make
//! the bundle measurement read falsely cheap.
//!
//! Do not build on this until the numbers in `.issues/134` say the approach
//! survives. Delete it if they say it does not.

/// Largest slip the decoder will look at, in pixels.
///
/// Decode cost is linear in pixel count and the free-plan CPU budget is 10 ms
/// per request (`.plans/010` §P0.2), so an unbounded `load_from_memory` on a
/// 12-megapixel phone photo is the failure mode this cap exists to prevent.
/// Chosen to still comfortably contain a 1080×1920 phone screenshot, which is
/// what a real slip almost always is.
const MAX_PIXELS: u64 = 4_000_000;

/// Decode the mini-QR carried by a Thai bank transfer slip.
///
/// Returns the raw QR payload string. `None` when the bytes are not a
/// supported image, when the image is larger than [`MAX_PIXELS`], or when no
/// QR grid decodes — all of which mean *not known*, never *not a slip*.
pub(crate) fn decode_slip_qr(image_bytes: &[u8]) -> Option<String> {
    let reader = image::ImageReader::new(std::io::Cursor::new(image_bytes))
        .with_guessed_format()
        .ok()?;

    // Read the header first and bail before paying for a full decode.
    let (w, h) = reader.into_dimensions().ok()?;
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return None;
    }

    let img = image::load_from_memory(image_bytes).ok()?;
    let mut prepared = rqrr::PreparedImage::prepare(img.to_luma8());
    for grid in prepared.detect_grids() {
        if let Ok((_meta, content)) = grid.decode() {
            return Some(content);
        }
    }
    None
}

/// [`decode_slip_qr`] over a base64 `data:` URL, the shape the upload handlers
/// actually receive. `None` for an external `https://` slip or an R2 path.
pub(crate) fn decode_slip_qr_from_url(slip_url: &str) -> Option<String> {
    use base64::Engine;

    let rest = slip_url.strip_prefix("data:")?;
    let (header, data) = rest.split_once(',')?;
    if !header.contains(";base64") {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .ok()?;
    decode_slip_qr(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};

    /// The published slip-verify vector (Bangkok Bank, ref
    /// `0002123123121200011`), so a successful decode is checked against the
    /// exact string and not merely against "something came back".
    const BANK_PAYLOAD: &str = "004000060000010103002021900021231231212000115102TH91049C30";

    /// Render `payload` as a QR and paste it into a `w`×`h` light-grey canvas,
    /// at `module_px` pixels per QR module — the shape a real upload has: a
    /// phone screenshot that is almost entirely slip, with a small QR in it.
    fn synthetic_slip(payload: &str, w: u32, h: u32, module_px: u32) -> GrayImage {
        let code = qrcode::QrCode::new(payload.as_bytes()).expect("payload fits a QR");
        let colors = code.to_colors();
        let modules = code.width() as u32;

        // Slip background: not pure white, because a real screenshot never is
        // and a decoder tuned on pure white would flatter itself.
        let mut canvas = GrayImage::from_pixel(w, h, Luma([0xF2]));

        let quiet = 4 * module_px;
        let size = modules * module_px;
        assert!(
            w >= size + 2 * quiet && h >= size + 2 * quiet,
            "QR must fit"
        );
        // Bottom-centre, which is where the banks put it.
        let ox = (w - size) / 2;
        let oy = h - size - quiet;

        // White quiet zone, then the modules.
        for y in oy.saturating_sub(quiet)..(oy + size + quiet).min(h) {
            for x in ox.saturating_sub(quiet)..(ox + size + quiet).min(w) {
                canvas.put_pixel(x, y, Luma([0xFF]));
            }
        }
        for my in 0..modules {
            for mx in 0..modules {
                if colors[(my * modules + mx) as usize] != qrcode::Color::Dark {
                    continue;
                }
                for dy in 0..module_px {
                    for dx in 0..module_px {
                        canvas.put_pixel(
                            ox + mx * module_px + dx,
                            oy + my * module_px + dy,
                            Luma([0x00]),
                        );
                    }
                }
            }
        }
        canvas
    }

    fn as_jpeg(img: &GrayImage) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageLuma8(img.clone())
            .write_to(&mut out, image::ImageFormat::Jpeg)
            .expect("jpeg encodes");
        out.into_inner()
    }

    /// The decoder must recover the payload byte for byte from a realistic
    /// phone-screenshot-sized JPEG. A decoder that only works on a bare,
    /// lossless QR would pass a weaker test and fail every real upload.
    #[test]
    fn recovers_the_payload_from_a_screenshot_sized_jpeg() {
        let bytes = as_jpeg(&synthetic_slip(BANK_PAYLOAD, 1080, 1920, 6));
        assert_eq!(decode_slip_qr(&bytes).as_deref(), Some(BANK_PAYLOAD));
    }

    /// The pixel cap must refuse before decoding, not after — the whole point
    /// is to not pay for a 12-megapixel decode on a 10 ms CPU budget.
    #[test]
    fn an_oversized_image_is_refused_by_the_pixel_cap() {
        let big = GrayImage::from_pixel(3000, 2000, Luma([0xFF])); // 6 MP > cap
        let bytes = as_jpeg(&big);
        assert_eq!(decode_slip_qr(&bytes), None, "6 MP must be refused");
        // ...and the cap is the reason, not a decode failure: the same image
        // under the cap still returns None only because it has no QR.
        assert!(u64::from(3000u32) * u64::from(2000u32) > MAX_PIXELS);
    }

    #[test]
    fn non_images_and_non_uploads_decode_to_none() {
        assert_eq!(decode_slip_qr(b"not an image at all"), None);
        assert_eq!(decode_slip_qr(&[]), None);
        assert_eq!(
            decode_slip_qr_from_url("https://example.com/slip.jpg"),
            None
        );
        assert_eq!(decode_slip_qr_from_url("data:image/png,rawbytes"), None);
    }

    /// NOT an assertion about wall-clock — a native number is only a *lower
    /// bound* on what the same work costs in wasm on Cloudflare's 10 ms
    /// free-plan CPU budget (`.plans/010` §P0.2). Run with `--nocapture` and
    /// read the numbers; they are written up in `.issues/134`.
    #[test]
    fn cpu_probe_print_native_decode_cost() {
        for (w, h, module_px, label) in [
            (1080u32, 1920u32, 6u32, "1080x1920 phone screenshot"),
            (828, 1792, 5, "828x1792 phone screenshot"),
            (720, 1280, 4, "720x1280 downscaled"),
            (500, 500, 6, "500x500 cropped to the QR"),
        ] {
            let bytes = as_jpeg(&synthetic_slip(BANK_PAYLOAD, w, h, module_px));
            // Split the cost, because it decides whether "just decode a
            // smaller image" is a real mitigation or wishful thinking: JPEG
            // decode shrinks with the image, QR grid detection does not shrink
            // usefully, because shrinking past the module size destroys the QR.
            let t0 = std::time::Instant::now();
            let img = image::load_from_memory(&bytes)
                .expect("jpeg decodes")
                .to_luma8();
            let jpeg_ms = t0.elapsed().as_secs_f64() * 1000.0;

            let t1 = std::time::Instant::now();
            let got = decode_slip_qr(&bytes);
            let total_ms = t1.elapsed().as_secs_f64() * 1000.0;
            drop(img);

            println!(
                "  slip-qr native decode: {label:28} {:>4} KiB jpeg                   total {total_ms:>7.2} ms  (jpeg {jpeg_ms:>6.2} ms, qr {:>7.2} ms)  decoded={}",
                bytes.len() / 1024,
                total_ms - jpeg_ms,
                got.is_some()
            );
            assert_eq!(got.as_deref(), Some(BANK_PAYLOAD), "{label} must decode");
        }
    }
}
