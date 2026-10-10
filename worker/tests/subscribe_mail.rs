//! The announcement mail (.plans/045 R4.12): what Gmail receives is valid
//! RFC 5322 in UTF-8 with one-click unsubscribe, nothing can inject a header,
//! the words name the event, its Bangkok time and both links, and the cron
//! that drives it is the one wrangler.toml schedules.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use event_checkin_worker::mail::message::{Mail, gmail_raw, header_word, rfc5322};
use event_checkin_worker::subscribers::copy::{OpenEvent, announcement};
use event_checkin_worker::subscribers::db::is_token;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn mail<'a>(to: &'a str, subject: &'a str, text: &'a str, unsub: &'a str) -> Mail<'a> {
    Mail {
        from_name: "BeThere",
        from_email: "bethere.sol@gmail.com",
        to,
        subject,
        text,
        unsubscribe_url: unsub,
    }
}

fn event() -> OpenEvent {
    OpenEvent {
        id: "rtm-7".into(),
        name: "Road to Mainnet #7".into(),
        slug: "rtm-7".into(),
        // 2026-11-01 06:00 UTC = 13:00 in Bangkok
        start_ms: 1_793_512_800_000,
        location: "Bangkok".into(),
    }
}

#[test]
fn message_is_utf8_with_one_click_unsubscribe() {
    let m = mail(
        "a@example.com",
        "งานใหม่เปิดแล้ว",
        "สวัสดี\nline two",
        "https://x.example/api/unsubscribe/t",
    );
    let text = rfc5322(&m).unwrap();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    assert!(head.contains("From: BeThere <bethere.sol@gmail.com>"));
    assert!(head.contains("To: <a@example.com>"));
    assert!(head.contains(&format!(
        "Subject: =?UTF-8?B?{}?=",
        STANDARD.encode("งานใหม่เปิดแล้ว")
    )));
    assert!(head.contains("Content-Type: text/plain; charset=UTF-8"));
    assert!(head.contains("List-Unsubscribe: <https://x.example/api/unsubscribe/t>"));
    assert!(head.contains("List-Unsubscribe-Post: List-Unsubscribe=One-Click"));
    // every header line ends CRLF, no bare LF
    assert!(!head.replace("\r\n", "").contains('\n'));
    // the body decodes back, with CRLF line ends, and wraps at 76
    assert!(body.split("\r\n").all(|l| l.len() <= 76));
    let decoded = STANDARD.decode(body.replace("\r\n", "")).unwrap();
    assert_eq!(String::from_utf8(decoded).unwrap(), "สวัสดี\r\nline two");
    // what Gmail gets is the same text, base64url without padding
    let raw = gmail_raw(&m).unwrap();
    assert!(!raw.contains(['+', '/', '=']));
    assert_eq!(URL_SAFE_NO_PAD.decode(raw).unwrap(), text.as_bytes());
}

#[test]
fn nothing_injects_a_header() {
    let unsub = "https://x.example/u";
    assert!(
        rfc5322(&mail(
            "a@example.com\r\nBcc: b@example.com",
            "s",
            "t",
            unsub
        ))
        .is_none()
    );
    assert!(rfc5322(&mail("a@example.com", "s", "t", "https://x\nBcc: b")).is_none());
    // a subject with a line break is encoded, not split into a new header
    let text = rfc5322(&mail(
        "a@example.com",
        "hi\r\nBcc: b@example.com",
        "t",
        unsub,
    ))
    .unwrap();
    assert!(!text.contains("\r\nBcc:"));
    assert_eq!(header_word("plain"), "plain");
}

#[test]
fn announcement_names_the_event_time_and_links() {
    let (subject, text) = announcement(&event(), false, "https://bethere.example", TOKEN);
    assert_eq!(subject, "Open now: Road to Mainnet #7");
    assert!(
        text.contains("1 Nov 2026, 13:00 (Bangkok, GMT+7)"),
        "{text}"
    );
    assert!(text.contains("Bangkok"));
    assert!(text.contains("https://bethere.example/e/rtm-7"));
    assert!(text.contains(&format!("https://bethere.example/unsubscribe/{TOKEN}")));

    let (subject, text) = announcement(&event(), true, "https://bethere.example", TOKEN);
    assert_eq!(subject, "งานใหม่เปิดแล้ว: Road to Mainnet #7");
    assert!(text.contains("1 พ.ย. 2569 · 13:00 น. (เวลาไทย)"), "{text}");
    assert!(text.contains("ยกเลิกการรับ"));
}

#[test]
fn only_a_minted_token_reaches_sql() {
    assert!(is_token(TOKEN));
    assert!(!is_token(&TOKEN.to_uppercase()));
    assert!(!is_token(&TOKEN[1..]));
    assert!(!is_token("' OR 1=1 --"));
}

#[test]
fn the_hourly_cron_is_scheduled_and_claims_before_sending() {
    let root = env!("CARGO_MANIFEST_DIR");
    let toml = std::fs::read_to_string(format!("{root}/wrangler.toml")).unwrap();
    let lib = std::fs::read_to_string(format!("{root}/src/lib.rs")).unwrap();
    let cron = lib
        .split("pub const ANNOUNCE_CRON: &str = \"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .expect("ANNOUNCE_CRON");
    assert!(
        toml.contains(&format!("\"{cron}\"")),
        "wrangler.toml must schedule {cron}"
    );
    // The claim (the idempotency key) is written before the send.
    let job = std::fs::read_to_string(format!("{root}/src/subscribers/announce.rs")).unwrap();
    let claim = job.find("db::claim(").expect("claim");
    let send = job.find("gmail.send(").expect("send");
    assert!(claim < send);
}
