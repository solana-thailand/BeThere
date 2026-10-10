//! The announcement mail, in the subscriber's language. Pure, so the words
//! and the dates are tested natively (`tests/subscribe_copy.rs`).

use chrono::{FixedOffset, TimeZone};

/// The event as the mail tells it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenEvent {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub start_ms: i64,
    pub location: String,
}

/// Bangkok time, where every event so far has been: the mail names the zone.
fn when(start_ms: i64, thai: bool) -> String {
    let tz = FixedOffset::east_opt(7 * 3600).expect("UTC+7 is a valid offset");
    let Some(t) = tz.timestamp_millis_opt(start_ms).single() else {
        return String::new();
    };
    match thai {
        true => {
            const MONTHS: [&str; 12] = [
                "ม.ค.",
                "ก.พ.",
                "มี.ค.",
                "เม.ย.",
                "พ.ค.",
                "มิ.ย.",
                "ก.ค.",
                "ส.ค.",
                "ก.ย.",
                "ต.ค.",
                "พ.ย.",
                "ธ.ค.",
            ];
            use chrono::Datelike;
            format!(
                "{} {} {} · {} น. (เวลาไทย)",
                t.day(),
                MONTHS[t.month0() as usize],
                t.year() + 543,
                t.format("%H:%M")
            )
        }
        false => format!("{} (Bangkok, GMT+7)", t.format("%-d %b %Y, %H:%M")),
    }
}

/// `(subject, text)` for `event`, in Thai or English. `site` is the origin
/// (no trailing slash); `unsub_token` goes in the footer link.
pub fn announcement(
    event: &OpenEvent,
    thai: bool,
    site: &str,
    unsub_token: &str,
) -> (String, String) {
    let page = format!("{site}/e/{}", event.slug);
    let unsub = format!("{site}/unsubscribe/{unsub_token}");
    let when = when(event.start_ms, thai);
    let place = match event.location.trim() {
        "" => String::new(),
        loc => format!("\n{loc}"),
    };
    match thai {
        true => (
            format!("งานใหม่เปิดแล้ว: {}", event.name),
            format!(
                "{name}\n{when}{place}\n\nลงทะเบียน: {page}\n\nงานฟรี จองที่ด้วยมัดจำ มาถึงแล้วได้คืนเต็ม\n\n—\nคุณได้รับอีเมลนี้เพราะขอให้แจ้งเมื่องานถัดไปเปิด\nยกเลิกการรับ: {unsub}",
                name = event.name
            ),
        ),
        false => (
            format!("Open now: {}", event.name),
            format!(
                "{name}\n{when}{place}\n\nRegister: {page}\n\nFree events. Hold your seat with a deposit, get it all back when you show up.\n\n—\nYou get this because you asked to hear when the next event opens.\nUnsubscribe: {unsub}",
                name = event.name
            ),
        ),
    }
}
