//! One plain-text mail as Gmail's `users.messages.send` wants it: RFC 5322,
//! UTF-8 throughout (Thai subjects as RFC 2047 encoded words, the body as
//! base64), with one-click unsubscribe headers (RFC 8058), then base64url.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};

/// What a mail says and where its unsubscribe goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mail<'a> {
    pub from_name: &'a str,
    pub from_email: &'a str,
    pub to: &'a str,
    pub subject: &'a str,
    pub text: &'a str,
    /// The one-click unsubscribe endpoint (POST), for `List-Unsubscribe`.
    pub unsubscribe_url: &'a str,
}

/// An RFC 2047 encoded word: ASCII text as is, anything else as UTF-8 base64.
pub fn header_word(text: &str) -> String {
    match text.is_ascii() && !text.contains(['\r', '\n']) {
        true => text.to_string(),
        false => format!("=?UTF-8?B?{}?=", STANDARD.encode(text)),
    }
}

/// Header values may not carry a line break (header injection): one in an
/// address or URL drops the mail rather than sending something else.
fn single_line(value: &str) -> Option<&str> {
    (!value.contains(['\r', '\n'])).then_some(value)
}

/// The message as RFC 5322 text with CRLF line ends, or `None` when an
/// address or URL would break a header line.
pub fn rfc5322(mail: &Mail<'_>) -> Option<String> {
    let from = single_line(mail.from_email)?;
    let to = single_line(mail.to)?;
    let unsub = single_line(mail.unsubscribe_url)?;
    let body = STANDARD.encode(mail.text.replace("\r\n", "\n").replace('\n', "\r\n"));
    let wrapped: Vec<&str> = body
        .as_bytes()
        .chunks(76)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect();
    Some(
        [
            format!("From: {} <{from}>", header_word(mail.from_name)),
            format!("To: <{to}>"),
            format!("Subject: {}", header_word(mail.subject)),
            "MIME-Version: 1.0".to_string(),
            "Content-Type: text/plain; charset=UTF-8".to_string(),
            "Content-Transfer-Encoding: base64".to_string(),
            format!("List-Unsubscribe: <{unsub}>"),
            "List-Unsubscribe-Post: List-Unsubscribe=One-Click".to_string(),
            String::new(),
            wrapped.join("\r\n"),
        ]
        .join("\r\n"),
    )
}

/// The `raw` field of `users.messages.send`: the message, base64url.
pub fn gmail_raw(mail: &Mail<'_>) -> Option<String> {
    rfc5322(mail).map(|m| URL_SAFE_NO_PAD.encode(m))
}
