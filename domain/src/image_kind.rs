//! Identify an uploaded image by its leading bytes, not by its label.
//!
//! A data URL's `data:image/png;base64,` prefix is whatever the client wrote.
//! Checking only that label lets any file through as long as it claims to be
//! an image (ISO 27001 A.8.7, `.plans/029`). The signatures below come from the
//! format specs and are the only ones the slip flow accepts:
//!
//! | Format | Bytes |
//! |---|---|
//! | JPEG | `FF D8 FF` (SOI marker, then the first segment marker) |
//! | PNG  | `89 50 4E 47 0D 0A 1A 0A` |
//! | WebP | `RIFF` `<u32 size>` `WEBP` |
//!
//! Only [`SNIFF_LEN`] bytes are needed, so the caller can decode just the start
//! of a large base64 payload.

/// The bytes needed to recognise every accepted format (WebP needs 12).
pub const SNIFF_LEN: usize = 12;

/// Image formats accepted for payment-slip uploads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Jpeg,
    Png,
    Webp,
}

impl ImageKind {
    /// Recognise the format from the first bytes of the file.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0xFF, 0xD8, 0xFF, ..] => Some(Self::Jpeg),
            [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some(Self::Png),
            // RIFF, a 4-byte chunk size, then the WEBP form type.
            _ if bytes.get(..4) == Some(b"RIFF".as_slice())
                && bytes.get(8..12) == Some(b"WEBP".as_slice()) =>
            {
                Some(Self::Webp)
            }
            _ => None,
        }
    }

    /// Map a declared MIME type onto a kind (`image/jpg` is a common alias).
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime.trim().to_ascii_lowercase().as_str() {
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            "image/webp" => Some(Self::Webp),
            _ => None,
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
        }
    }

    /// File extension used for the R2 key; serving maps it back to the MIME.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }
}
