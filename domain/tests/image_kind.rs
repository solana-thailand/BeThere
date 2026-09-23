//! Magic-byte recognition for slip uploads (`.plans/029` §2, ISO 27001 A.8.7).

use event_checkin_domain::image_kind::{ImageKind, SNIFF_LEN};

const JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01,
];
const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
];
const WEBP: &[u8] = b"RIFF\x24\x00\x00\x00WEBPVP8 ";

#[test]
fn real_signatures_are_recognised() {
    assert_eq!(ImageKind::sniff(JPEG), Some(ImageKind::Jpeg));
    assert_eq!(ImageKind::sniff(PNG), Some(ImageKind::Png));
    assert_eq!(ImageKind::sniff(WEBP), Some(ImageKind::Webp));
}

#[test]
fn sniff_len_covers_every_signature() {
    for sample in [JPEG, PNG, WEBP] {
        assert!(ImageKind::sniff(&sample[..SNIFF_LEN]).is_some());
    }
}

#[test]
fn non_images_and_near_misses_are_rejected() {
    let cases: &[&[u8]] = &[
        b"",
        b"<svg xmlns=\"http://www.w3.org/2000/svg\">",
        b"<!DOCTYPE html><script>",
        b"%PDF-1.7\n",
        b"PK\x03\x04zipfile!",
        b"GIF89a\x01\x00\x01\x00",
        // A RIFF container that is not WebP (WAV audio).
        b"RIFF\x24\x00\x00\x00WAVEfmt ",
        // Truncated signatures.
        &[0xFF, 0xD8],
        &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A],
        b"RIFF\x24\x00\x00\x00WEB",
    ];
    for bytes in cases {
        assert_eq!(ImageKind::sniff(bytes), None, "{bytes:?}");
    }
}

#[test]
fn declared_mime_maps_onto_kind() {
    assert_eq!(ImageKind::from_mime("image/jpeg"), Some(ImageKind::Jpeg));
    assert_eq!(ImageKind::from_mime("image/jpg"), Some(ImageKind::Jpeg));
    assert_eq!(ImageKind::from_mime(" IMAGE/PNG "), Some(ImageKind::Png));
    assert_eq!(ImageKind::from_mime("image/webp"), Some(ImageKind::Webp));
    assert_eq!(ImageKind::from_mime("image/svg+xml"), None);
    assert_eq!(ImageKind::from_mime("image/heic"), None);
}

#[test]
fn mime_and_extension_round_trip() {
    for kind in [ImageKind::Jpeg, ImageKind::Png, ImageKind::Webp] {
        assert_eq!(ImageKind::from_mime(kind.mime()), Some(kind));
    }
    assert_eq!(ImageKind::Jpeg.extension(), "jpg");
    assert_eq!(ImageKind::Png.extension(), "png");
    assert_eq!(ImageKind::Webp.extension(), "webp");
}
