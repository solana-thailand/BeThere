//! Share-card rules shared by the upload route and the `/e/{slug}` splice
//! (`.issues/183`).

use event_checkin_domain::og_card::{
    OG_HEIGHT, OG_MAX_BYTES, OG_WIDTH, OgPngError, check_og_png, event_summary, event_when,
    png_dimensions,
};

/// The first 24 bytes of a PNG: signature, IHDR length, `IHDR`, width, height.
fn png_head(width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    out.extend_from_slice(&13u32.to_be_bytes());
    out.extend_from_slice(b"IHDR");
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&[8, 6, 0, 0, 0]);
    out
}

#[test]
fn a_card_sized_png_passes() {
    assert_eq!(png_dimensions(&png_head(1200, 630)), Some((1200, 630)));
    assert_eq!(check_og_png(&png_head(OG_WIDTH, OG_HEIGHT)), Ok(()));
}

#[test]
fn the_label_is_not_trusted() {
    // A JPEG, an SVG and a truncated PNG all fail on their bytes.
    assert_eq!(
        check_og_png(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]),
        Err(OgPngError::NotPng)
    );
    assert_eq!(
        check_og_png(b"<svg xmlns=\"x\"></svg>"),
        Err(OgPngError::NotPng)
    );
    assert_eq!(
        check_og_png(&png_head(1200, 630)[..20]),
        Err(OgPngError::NotPng)
    );
    // Signature right, first chunk not IHDR.
    let mut wrong_chunk = png_head(1200, 630);
    wrong_chunk[12..16].copy_from_slice(b"IDAT");
    assert_eq!(check_og_png(&wrong_chunk), Err(OgPngError::NotPng));
}

#[test]
fn other_sizes_and_empty_bodies_fail() {
    assert_eq!(
        check_og_png(&png_head(630, 1200)),
        Err(OgPngError::WrongSize {
            width: 630,
            height: 1200
        })
    );
    assert_eq!(check_og_png(&[]), Err(OgPngError::Empty));
}

#[test]
fn the_byte_cap_holds() {
    let mut big = png_head(1200, 630);
    big.resize(OG_MAX_BYTES + 1, 0);
    assert_eq!(
        check_og_png(&big),
        Err(OgPngError::TooLarge {
            bytes: OG_MAX_BYTES + 1
        })
    );
    big.truncate(OG_MAX_BYTES);
    assert_eq!(check_og_png(&big), Ok(()));
}

#[test]
fn error_text_has_no_url_scheme() {
    // The error redactor turns "://" into [redacted-url].
    for e in [
        OgPngError::Empty,
        OgPngError::NotPng,
        OgPngError::TooLarge { bytes: 9 },
        OgPngError::WrongSize {
            width: 1,
            height: 2,
        },
    ] {
        assert!(!e.to_string().contains("://"), "{e}");
    }
}

#[test]
fn dates_are_bangkok_time() {
    // 2026-10-12 11:00 UTC is 18:00 in Bangkok.
    assert_eq!(
        event_when(1_791_802_800_000, false),
        "Mon 12 Oct 2026 · 18:00 (UTC+7)"
    );
    // 20:00 UTC rolls over to the next day.
    assert_eq!(
        event_when(1_791_835_200_000, false),
        "Tue 13 Oct 2026 · 03:00 (UTC+7)"
    );
}

#[test]
fn tba_and_missing_dates_say_so() {
    assert_eq!(event_when(1_791_802_800_000, true), "Date to be announced");
    assert_eq!(event_when(0, false), "Date to be announced");
}

#[test]
fn summary_joins_a_tidy_location() {
    assert_eq!(
        event_summary(0, true, "  Bangkok \n  Co-working "),
        "Date to be announced · Bangkok Co-working"
    );
    assert_eq!(event_summary(0, true, "   "), "Date to be announced");
}
