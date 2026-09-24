//! Slip uploads are checked by content, not by label (`.plans/029` §2, A.8.7).
//!
//! `validate_slip_url` used to trust the `data:image/png;base64,` prefix, so
//! any file was accepted as long as the client called it an image.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use event_checkin_domain::image_kind::ImageKind;
use event_checkin_worker::storage::sniff_base64_image;

fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

#[test]
fn real_image_payloads_are_recognised() {
    let jpeg = [
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0, 1, 1, 1,
    ];
    let png = [
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, b'I', b'H',
    ];
    let webp = *b"RIFF\x24\x00\x00\x00WEBPVP8 \x00\x00";
    assert_eq!(sniff_base64_image(&b64(&jpeg)), Some(ImageKind::Jpeg));
    assert_eq!(sniff_base64_image(&b64(&png)), Some(ImageKind::Png));
    assert_eq!(sniff_base64_image(&b64(&webp)), Some(ImageKind::Webp));
}

#[test]
fn only_the_prefix_is_decoded() {
    // A valid image head followed by bytes that are not base64 at all: the
    // sniffer must not decode (or reject) the multi-megabyte tail.
    let head = b64(&[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D,
    ]);
    let payload = format!("{head}!!!!not-base64-tail!!!!");
    assert_eq!(sniff_base64_image(&payload), Some(ImageKind::Png));
}

#[test]
fn non_images_labelled_as_images_are_rejected() {
    for body in [
        b"<svg xmlns='http://www.w3.org/2000/svg' onload='alert(1)'/>".as_slice(),
        b"<!DOCTYPE html><script>alert(1)</script>",
        b"%PDF-1.7\n%\xe2\xe3\xcf\xd3",
        b"MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00",
    ] {
        assert_eq!(sniff_base64_image(&b64(body)), None, "{body:?}");
    }
}

#[test]
fn malformed_or_empty_payloads_are_rejected() {
    for payload in ["", "aaa", "@@@@@@@@@@@@@@@@", "/9i/", "iVBORw0KGgg="] {
        assert_eq!(sniff_base64_image(payload), None, "{payload:?}");
    }
}

#[test]
fn bare_signatures_are_enough() {
    // The sniffer decides the format, not whether the image is well formed:
    // `/9j/` is exactly FF D8 FF and `iVBORw0KGgo=` is the 8-byte PNG header
    // (both are fixtures in slip_admin_upload.rs). Size limits and the slip
    // QR check are separate concerns.
    assert_eq!(sniff_base64_image("/9j/"), Some(ImageKind::Jpeg));
    assert_eq!(sniff_base64_image("iVBORw0KGgo="), Some(ImageKind::Png));
}

#[test]
fn validate_slip_url_checks_content_not_just_the_label() {
    // Both slip endpoints (attendee and admin) go through validate_slip_url,
    // and it is crate-private, so pin the call instead of the behaviour.
    let src = include_str!("../src/handlers/deposit/thb/handlers/slip_upload.rs");
    let body = src
        .split("pub(crate) fn validate_slip_url")
        .nth(1)
        .expect("validate_slip_url exists");
    let body = &body[..body.find("\n}\n").expect("function end")];
    assert!(
        body.contains("sniff_base64_image(data)"),
        "validate_slip_url must sniff the decoded bytes of data URLs"
    );
    let admin = include_str!("../src/handlers/deposit/thb/handlers/slip_admin_upload.rs");
    assert!(admin.contains("slip_upload::validate_slip_url(&body.slip_url)?"));
}
