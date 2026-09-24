//! Content fingerprinting for uploaded THB payment slips.
//!
//! Nothing about a slip is checked today. Anyone can upload any image to the
//! deposit page, and the organizer catches the ones who did by recognising the
//! person — which is why refunds still require them to sit down with the list
//! and remember who bypassed (`.issues/129`).
//!
//! This is the cheapest thing that makes the *system* remember: hash the bytes.
//! Two people uploading a byte-identical image — the single most common real
//! abuse, forwarding a friend's slip — stop being indistinguishable. It costs
//! no new dependency (`blake3` is already linked through
//! `event-checkin-domain`'s `wire` feature) and no measurable bundle size,
//! which matters: the worker ships on Cloudflare's free plan and the budget is
//! enforced by `scripts/verify/worker_size_budget.sh`.
//!
//! What this deliberately does NOT do:
//!
//! - It does not verify that a payment happened. A convincing forgery with no
//!   matching bank record passes. Only the bank reference carried in the slip's
//!   mini-QR can answer that (`.issues/129` §G), and that needs a vendor
//!   account in the owner's name.
//! - It does not catch a re-screenshotted or re-compressed slip: one different
//!   byte is a different hash. A perceptual hash would, and would also produce
//!   false positives on two genuine transfers of the same amount from the same
//!   bank app. False positives here block a paying attendee at the door, so
//!   exactness is the right trade for the first pass.
//!
//! Because a false accusation is worse than a missed duplicate, the default
//! mode is REPORT, not REJECT: the duplicate is recorded and shown to the
//! organizer, and nobody is stopped from uploading.

/// What the upload handlers do when an uploaded slip matches another
/// attendee's slip byte for byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DuplicateMode {
    /// Hash and store, but do not compare. The escape hatch if the check ever
    /// misbehaves during an event — it keeps the column being populated so no
    /// history is lost while the comparison is off.
    Off,
    /// Hash, store and record the collision; let the upload through. The
    /// default: the organizer gains the signal they currently keep in their
    /// head, and no paying attendee can be blocked by a bug in this file.
    Report,
    /// Refuse the upload. Only appropriate once the report mode has run over a
    /// real event and produced no false positives.
    Reject,
}

impl DuplicateMode {
    /// Parse `THB_SLIP_DUPLICATE_MODE`.
    ///
    /// An unset or unrecognised value is `Report` — the safe middle. It is
    /// deliberately not `Off`: a typo in the variable name must not silently
    /// disable the check (that is how a fail-open control goes inert without
    /// anyone noticing), and it is deliberately not `Reject`, because a typo
    /// must not start turning paying attendees away either.
    pub(crate) fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim).unwrap_or("") {
            "off" => Self::Off,
            "reject" => Self::Reject,
            _ => Self::Report,
        }
    }
}

/// BLAKE3 of the decoded image bytes of a slip data URL, lowercase hex.
///
/// `None` when the input is not a base64 data URL (an external `https://` slip
/// URL, or an already-migrated R2 path) or when the base64 does not decode.
/// `None` means *not known*, never *not a duplicate*.
///
/// Hashes the decoded bytes rather than the data-URL string on purpose: the
/// MIME casing, the `;base64` marker and base64 padding all vary between
/// clients for the same image, so hashing the text would miss exactly the
/// duplicate this exists to catch.
pub(crate) fn slip_fingerprint(slip_url: &str) -> Option<String> {
    use base64::Engine;

    let rest = slip_url.strip_prefix("data:")?;
    let (header, data) = rest.split_once(',')?;
    if !header.contains(";base64") {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(blake3::hash(&bytes).to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_url(mime: &str, bytes: &[u8]) -> String {
        use base64::Engine;
        format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    /// The whole point: the same image, uploaded by two people, must produce
    /// the same fingerprint even though the two data URLs are not identical
    /// strings. If this ever hashed the text instead of the bytes, this test is
    /// what catches it.
    #[test]
    fn the_same_image_hashes_the_same_through_different_data_urls() {
        let image = b"\x89PNG\r\n\x1a\n-pretend-this-is-a-slip";
        let a = slip_fingerprint(&data_url("image/png", image)).expect("png hashes");
        let b = slip_fingerprint(&data_url("image/jpeg", image)).expect("jpeg header hashes");
        assert_eq!(a, b, "the same bytes must fingerprint the same");

        // ...and a single different byte must not.
        let other = b"\x89PNG\r\n\x1a\n-pretend-this-is-a-sliq";
        let c = slip_fingerprint(&data_url("image/png", other)).expect("hashes");
        assert_ne!(a, c, "different bytes must fingerprint differently");
    }

    /// Guards against the fingerprint being something constant — a bug that
    /// would make every slip a duplicate of every other and take the deposit
    /// page down for an entire event.
    #[test]
    fn the_fingerprint_is_not_constant_and_is_hex() {
        let a = slip_fingerprint(&data_url("image/png", b"one")).unwrap();
        let b = slip_fingerprint(&data_url("image/png", b"two")).unwrap();
        assert_ne!(a, b);
        assert_eq!(a.len(), 64, "blake3 hex is 64 chars");
        assert!(
            a.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
        );
    }

    #[test]
    fn non_uploads_have_no_fingerprint() {
        assert_eq!(slip_fingerprint("https://example.com/slip.jpg"), None);
        assert_eq!(slip_fingerprint("/api/storage/slips/evt/att"), None);
        assert_eq!(slip_fingerprint(""), None);
        // Declared as a data URL but not base64 — must not be hashed as text.
        assert_eq!(slip_fingerprint("data:image/png,rawbytes"), None);
        // Base64 that decodes to nothing is not a slip.
        assert_eq!(slip_fingerprint("data:image/png;base64,"), None);
    }

    /// The default must be Report: not Off (a typo in the variable name would
    /// silently disable the check) and not Reject (a typo would start turning
    /// paying attendees away).
    #[test]
    fn mode_defaults_to_report_and_parses_every_value() {
        assert_eq!(DuplicateMode::parse(None), DuplicateMode::Report);
        assert_eq!(DuplicateMode::parse(Some("")), DuplicateMode::Report);
        assert_eq!(
            DuplicateMode::parse(Some("nonsense")),
            DuplicateMode::Report
        );
        assert_eq!(DuplicateMode::parse(Some("REJECT")), DuplicateMode::Report);
        assert_eq!(DuplicateMode::parse(Some("off")), DuplicateMode::Off);
        assert_eq!(DuplicateMode::parse(Some(" off ")), DuplicateMode::Off);
        assert_eq!(DuplicateMode::parse(Some("report")), DuplicateMode::Report);
        assert_eq!(DuplicateMode::parse(Some("reject")), DuplicateMode::Reject);
    }
}
