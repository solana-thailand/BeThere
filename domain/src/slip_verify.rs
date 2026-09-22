//! Parser for the mini-QR carried by Thai bank transfer slips.
//!
//! Every Thai mobile-banking app stamps a small QR onto the transfer slip it
//! generates. It is not a payment QR — it holds the *bank transaction
//! reference*, the handle a bank's Open API will later resolve into "did this
//! transfer actually happen, for how much, to whom". That reference is the
//! only thing on a slip that a forger cannot invent, which is why
//! `.issues/130` names it as the real answer to "anyone can upload any image"
//! and why `handlers/.../slip_fingerprint.rs` describes itself as a stopgap.
//!
//! This module is deliberately only the *string* half: payload → reference. It
//! has no image dependency, so it costs nothing, and it lives in `domain` so
//! that the worker and the frontend parse the payload with the same code no
//! matter which of them ends up holding the image decoder (`.issues/134`).
//! Wherever the decode happens, the server must re-parse what it is handed —
//! a reference supplied by a client is a claim, not a fact.
//!
//! # Wire format
//!
//! The payload is EMVCo tag-length-value, the same grammar as PromptPay, with
//! a slip-specific tag layout:
//!
//! ```text
//! 00 40 0006000001 0103002 02190002123123121200011   root template
//!       └ 00: API type   └ 01: sending bank  └ 02: transaction reference
//! 51 02 TH                                           country
//! 91 04 9C30                                         CRC-16/CCITT-FALSE
//! ```
//!
//! TrueMoney stamps a different sub-layout under the same root tag and writes
//! its CRC in lowercase hex; both are handled.
//!
//! # What this does NOT establish
//!
//! A valid parse means "this image carried a well-formed slip QR". It does not
//! mean the transfer happened, or that the person uploading it made it. Only
//! resolving the reference against the bank answers that, and that needs a
//! vendor account in the owner's name (`.issues/129` §7).

use core::fmt;

/// A bank transaction reference recovered from a slip mini-QR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlipReference {
    /// The three-digit BOT code of the bank that issued the slip (`002` =
    /// Bangkok Bank, `014` = SCB, …). `None` for the TrueMoney variant, which
    /// carries no bank code because there is no sending bank.
    pub sending_bank: Option<String>,
    /// The reference itself. This is the value that must carry a schema-level
    /// `UNIQUE` — `.issues/127` is the record of what a code-level uniqueness
    /// check costs when it is the only thing holding the invariant.
    pub trans_ref: String,
}

impl SlipReference {
    /// A single stable string for storage and for the `UNIQUE` index.
    ///
    /// The bank code is part of the key because a transaction reference is
    /// only unique *within* the issuing bank; two banks may well mint the same
    /// digits. Joining them here rather than at each call site keeps the
    /// stored form and the index agreeing by construction.
    pub fn storage_key(&self) -> String {
        match &self.sending_bank {
            Some(bank) => format!("{bank}:{}", self.trans_ref),
            None => format!("truemoney:{}", self.trans_ref),
        }
    }
}

/// Why a QR payload is not a slip reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlipVerifyError {
    /// The payload is not well-formed EMVCo TLV — a truncated value, a
    /// non-numeric length, or trailing bytes.
    Malformed,
    /// Well-formed TLV, but not a slip QR: the root template or the CRC tag is
    /// missing. A PromptPay *payment* QR lands here, which is the common case
    /// — people photograph the payment QR as often as the slip.
    NotASlipQr,
    /// The CRC does not match the payload. Either the QR decoded with an
    /// undetected error, or the string was edited.
    BadChecksum,
    /// A slip QR whose root template carries no usable transaction reference.
    NoReference,
}

impl fmt::Display for SlipVerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::Malformed => "not well-formed EMVCo TLV",
            Self::NotASlipQr => "not a bank slip QR",
            Self::BadChecksum => "slip QR checksum mismatch",
            Self::NoReference => "slip QR carries no transaction reference",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for SlipVerifyError {}

/// Root tag holding the slip template.
const TAG_TEMPLATE: &str = "00";
/// Root tag holding the CRC. Slips put it at 91; payment QRs use 63.
const TAG_CRC: &str = "91";

/// Parse a decoded mini-QR payload into its transaction reference.
///
/// The CRC is verified before anything is returned. That is not ceremony: QR
/// error correction can hand back a *plausible* wrong string on a blurry
/// photograph, and a wrong reference written into a `UNIQUE` column is worse
/// than no reference — it burns a slot another attendee's real slip needs.
pub fn parse_slip_verify(payload: &str) -> Result<SlipReference, SlipVerifyError> {
    // EMVCo lengths are two ASCII digits counting *characters*; a payload with
    // any non-ASCII byte is not one, and rejecting it up front lets the rest of
    // this file index by byte without ever splitting a character.
    if !payload.is_ascii() {
        return Err(SlipVerifyError::Malformed);
    }

    let fields = parse_tlv(payload)?;

    let crc = fields
        .iter()
        .find(|(tag, _, _)| *tag == TAG_CRC)
        .ok_or(SlipVerifyError::NotASlipQr)?;
    verify_crc(payload, crc)?;

    let (_, template, _) = fields
        .iter()
        .find(|(tag, _, _)| *tag == TAG_TEMPLATE)
        .ok_or(SlipVerifyError::NotASlipQr)?;

    parse_template(template)
}

/// The sub-layout under the root template, which differs by issuer.
fn parse_template(template: &str) -> Result<SlipReference, SlipVerifyError> {
    let subs = parse_tlv(template)?;
    let sub = |tag: &str| {
        subs.iter()
            .find(|(t, _, _)| *t == tag)
            .map(|(_, value, _)| *value)
    };

    // TrueMoney marks itself with a two-character sub-00; the bank variant's
    // sub-00 is the six-character API-type marker. Discriminating on the
    // marker rather than on which tags happen to be present keeps a malformed
    // bank payload from being silently read as a TrueMoney one.
    match sub(TAG_TEMPLATE) {
        Some("000001") => {
            let trans_ref = sub("02").ok_or(SlipVerifyError::NoReference)?;
            if trans_ref.is_empty() {
                return Err(SlipVerifyError::NoReference);
            }
            Ok(SlipReference {
                sending_bank: sub("01").filter(|b| !b.is_empty()).map(str::to_string),
                trans_ref: trans_ref.to_string(),
            })
        }
        // TrueMoney: 02 = event type, 03 = transaction id, 04 = DDMMYYYY.
        Some("01") => {
            let trans_ref = sub("03").ok_or(SlipVerifyError::NoReference)?;
            if trans_ref.is_empty() {
                return Err(SlipVerifyError::NoReference);
            }
            Ok(SlipReference {
                sending_bank: None,
                trans_ref: trans_ref.to_string(),
            })
        }
        _ => Err(SlipVerifyError::NotASlipQr),
    }
}

/// One TLV field: tag, value, and the byte offset at which the tag started.
type Field<'a> = (&'a str, &'a str, usize);

/// Walk a TLV string end to end.
///
/// Any leftover byte is an error rather than a stopping point: a parser that
/// stops early would accept a payload with a corrupt tail and report the
/// reference it found before the corruption.
fn parse_tlv(input: &str) -> Result<Vec<Field<'_>>, SlipVerifyError> {
    let bytes = input.as_bytes();
    let mut fields = Vec::new();
    let mut i = 0usize;

    while i < bytes.len() {
        let start = i;
        // tag(2) + length(2) must both be present.
        if i + 4 > bytes.len() {
            return Err(SlipVerifyError::Malformed);
        }
        let tag = &input[i..i + 2];
        let len: usize = input[i + 2..i + 4]
            .parse()
            .map_err(|_| SlipVerifyError::Malformed)?;
        i += 4;
        if i + len > bytes.len() {
            return Err(SlipVerifyError::Malformed);
        }
        fields.push((tag, &input[i..i + len], start));
        i += len;
    }

    Ok(fields)
}

/// CRC-16/CCITT-FALSE over everything up to and including the CRC tag's own
/// tag+length header, compared case-insensitively.
///
/// TrueMoney writes the digest in lowercase and the banks in uppercase, so the
/// comparison is on the parsed number, not on the text.
fn verify_crc(payload: &str, crc_field: &Field<'_>) -> Result<(), SlipVerifyError> {
    let (_, value, start) = *crc_field;
    if value.len() != 4 {
        return Err(SlipVerifyError::Malformed);
    }
    // The CRC covers the payload up to the start of its own value, i.e.
    // including the literal "9104".
    let covered = &payload[..start + 4];
    let expected = u16::from_str_radix(value, 16).map_err(|_| SlipVerifyError::BadChecksum)?;
    match crc16_ccitt_false(covered.as_bytes()) == expected {
        true => Ok(()),
        false => Err(SlipVerifyError::BadChecksum),
    }
}

/// CRC-16/CCITT-FALSE: polynomial 0x1021, init 0xFFFF, no reflection, no final
/// xor. The digest EMVCo specifies for tag 63, which slips reuse at tag 91.
fn crc16_ccitt_false(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in data {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = match crc & 0x8000 {
                0 => crc << 1,
                _ => (crc << 1) ^ 0x1021,
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Published vector for the bank variant: Bangkok Bank (`002`), reference
    /// `0002123123121200011`. If the TLV walk or the CRC ever drifts, this is
    /// the test that says so.
    const BANK_PAYLOAD: &str = "004000060000010103002021900021231231212000115102TH91049C30";
    /// Published vector for the TrueMoney variant — different sub-layout, no
    /// country tag, lowercase CRC.
    const TRUEMONEY_PAYLOAD: &str = "00480002010102010203P2P0313TXN00012345670408250120249104b425";

    #[test]
    fn parses_the_bank_variant() {
        let parsed = parse_slip_verify(BANK_PAYLOAD).expect("bank slip parses");
        assert_eq!(parsed.sending_bank.as_deref(), Some("002"));
        assert_eq!(parsed.trans_ref, "0002123123121200011");
        assert_eq!(parsed.storage_key(), "002:0002123123121200011");
    }

    /// The lowercase CRC is the thing that breaks a naive string comparison,
    /// so this vector earns its place next to the bank one.
    #[test]
    fn parses_the_truemoney_variant() {
        let parsed = parse_slip_verify(TRUEMONEY_PAYLOAD).expect("truemoney slip parses");
        assert_eq!(parsed.sending_bank, None);
        assert_eq!(parsed.trans_ref, "TXN0001234567");
        assert_eq!(parsed.storage_key(), "truemoney:TXN0001234567");
    }

    /// The reason the CRC is checked at all: QR error correction can return a
    /// plausible but wrong string, and a wrong reference in a UNIQUE column
    /// burns the slot the real slip needs.
    #[test]
    fn a_single_edited_digit_is_rejected_not_parsed() {
        let tampered = BANK_PAYLOAD.replace("0002123123121200011", "0002123123121200012");
        assert_eq!(tampered.len(), BANK_PAYLOAD.len(), "same length, one digit");
        assert_eq!(
            parse_slip_verify(&tampered),
            Err(SlipVerifyError::BadChecksum)
        );
    }

    /// The common false positive: people photograph the *payment* QR, not the
    /// slip. A PromptPay payment payload is valid TLV and must still be
    /// refused — it carries no transaction reference, only a payee.
    #[test]
    fn a_promptpay_payment_qr_is_not_a_slip() {
        // CRC at tag 63, no tag 91 — valid EMVCo, wrong document.
        let payment = "00020101021129370016A000000677010111011300660000000005802TH53037645802TH\
                       6304A1B2";
        assert_eq!(parse_slip_verify(payment), Err(SlipVerifyError::NotASlipQr));
    }

    #[test]
    fn malformed_payloads_do_not_panic_and_do_not_parse() {
        for bad in [
            "",                                                                // empty
            "00",                                                              // tag with no length
            "0099",     // length past the end
            "00xx0000", // non-numeric length
            "004000060000010103002021900021231231212000115102TH91049C30EXTRA", // trailing bytes
            "00450006000001010300202190002123123121200011", // no CRC tag at all
        ] {
            assert!(
                parse_slip_verify(bad).is_err(),
                "must reject {bad:?}, not parse it"
            );
        }
    }

    /// Byte indexing in the TLV walk is only safe because non-ASCII is refused
    /// up front. A multi-byte payload must be rejected, never sliced.
    #[test]
    fn non_ascii_is_refused_before_any_byte_indexing() {
        assert_eq!(
            parse_slip_verify("00๐๔0000"),
            Err(SlipVerifyError::Malformed)
        );
    }

    /// Guards the digest itself against being reimplemented with the wrong
    /// init or a reflected polynomial — both produce a self-consistent but
    /// non-standard CRC that would reject every real slip.
    #[test]
    fn crc16_matches_the_published_check_value() {
        // CRC-16/CCITT-FALSE check value for "123456789" is 0x29B1.
        assert_eq!(crc16_ccitt_false(b"123456789"), 0x29B1);
    }
}
