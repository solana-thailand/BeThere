//! Facts from the OCR text of a Thai bank slip (`.plans/035` §3.1).
//!
//! The browser runs the OCR engine and sends the raw text; the server runs
//! this parser, so there is one reading of what the text says, and it is the
//! server's. Same rule as the slip QR (`slip_verify`).
//!
//! The rule that shapes every function here: **a miss is cheap, a wrong
//! figure is not.** An unread amount means the organizer reads the slip, as
//! they do today. A misread amount that happens to equal the deposit passes
//! the amount check. So anything ambiguous stays `None`.
//! `.benchmarks/001` measured the misreads this guards against: "1,500.00"
//! read as "1.500.00", a decomposed sara am, a fee line.
//!
//! No `regex` dependency: this crate also ships to wasm32, and the scans below
//! are a few dozen lines.

use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Utc};

use crate::slip_proposal::{SlipFacts, parse_thb_satang};

/// Longest OCR text accepted. A full slip reads as well under 1 KiB; the cap
/// keeps a hostile body from costing the worker CPU. Longer text parses to
/// nothing.
pub const MAX_OCR_TEXT_BYTES: usize = 4096;

/// Thai short month names as printed on slips, January first.
const THAI_MONTHS: [&str; 12] = [
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

/// Slips print Bangkok time.
const BANGKOK_OFFSET_SECS: i32 = 7 * 3600;

/// Read the amount and the transfer time off a slip's OCR text.
///
/// `bank_ref` and `receiver_account` are always `None`. The reference comes
/// from the QR. The receiver line is not read yet: `.benchmarks/001` found the
/// masked `x`s read as Thai digits and symbols, and no extractor for it has
/// been measured.
pub fn facts_from_ocr_text(text: &str) -> SlipFacts {
    if text.len() > MAX_OCR_TEXT_BYTES {
        return SlipFacts::default();
    }
    let text = fold_sara_am(text);
    let lines: Vec<&str> = text.lines().collect();
    SlipFacts {
        amount_satang: amount_satang(&lines),
        transferred_at: transferred_at(&lines),
        ..SlipFacts::default()
    }
}

/// Tesseract emits ำ as nikhahit + sara aa (U+0E4D U+0E32), so a label
/// written with ำ (U+0E33) would never match without this.
fn fold_sara_am(text: &str) -> String {
    text.replace("\u{0E4D}\u{0E32}", "\u{0E33}")
}

fn is_fee_line(line: &str) -> bool {
    line.contains("ธรรมเนียม") || line.to_ascii_lowercase().contains("fee")
}

fn is_amount_label(line: &str) -> bool {
    line.contains("จำนวน")
}

/// The transfer amount, in satang.
///
/// 1. A non-zero figure on a line that names baht or the amount, or on the
///    line right after the amount label. Fee lines are skipped.
/// 2. Otherwise, only if exactly one distinct non-zero figure appears outside
///    fee lines. Two candidates means we don't know which, so `None`.
fn amount_satang(lines: &[&str]) -> Option<u64> {
    let labelled = lines.iter().enumerate().find_map(|(i, line)| {
        let near_label = line.contains("บาท")
            || is_amount_label(line)
            || i.checked_sub(1)
                .is_some_and(|prev| is_amount_label(lines[prev]));
        match near_label && !is_fee_line(line) {
            true => money_figures(line).find(|satang| *satang != 0),
            false => None,
        }
    });
    if labelled.is_some() {
        return labelled;
    }
    let mut distinct = lines
        .iter()
        .filter(|line| !is_fee_line(line))
        .flat_map(|line| money_figures(line))
        .filter(|satang| *satang != 0);
    let first = distinct.next()?;
    match distinct.all(|other| other == first) {
        true => Some(first),
        false => None,
    }
}

/// Every well-formed money figure on a line, in satang.
///
/// A figure is a maximal run of digits, `,` and `.` that is exactly
/// `d{1,3}([,.]ddd)*.dd`. Taking the maximal run is what keeps "500.00" from
/// being lifted out of "1.500.00" or out of a longer digit string.
fn money_figures(line: &str) -> impl Iterator<Item = u64> + '_ {
    line.split(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.'))
        .filter(|run| is_money_shape(run))
        .filter_map(|run| {
            // Every separator but the last is a thousands mark: OCR reads
            // "1,500.00" as "1.500.00" often enough to matter.
            let (whole, cents) = run.split_at(run.len() - 3);
            let whole: String = whole.chars().filter(char::is_ascii_digit).collect();
            parse_thb_satang(&format!("{whole}{cents}"))
        })
}

fn is_money_shape(run: &str) -> bool {
    let bytes = run.as_bytes();
    let Some((head, &[b'.', c1, c2])) = bytes.split_last_chunk::<3>() else {
        return false;
    };
    if !(c1.is_ascii_digit() && c2.is_ascii_digit()) {
        return false;
    }
    let mut groups = head.split(|b| *b == b',' || *b == b'.');
    let Some(lead) = groups.next() else {
        return false;
    };
    (1..=3).contains(&lead.len())
        && lead.iter().all(u8::is_ascii_digit)
        && groups.all(|g| g.len() == 3 && g.iter().all(u8::is_ascii_digit))
}

/// The transfer time: a Thai date (`17 เม.ย. 69`, `6 ธ.ค. 2569`) and an
/// `HH:MM` after it **on the same line**. A date without a year, or a time
/// found only on another line, gives `None` rather than a guess.
fn transferred_at(lines: &[&str]) -> Option<DateTime<Utc>> {
    lines.iter().find_map(|line| {
        let chars: Vec<char> = line.chars().collect();
        let (date, rest) = thai_date(&chars)?;
        let (hour, minute) = clock_time(rest)?;
        let local = date.and_hms_opt(hour, minute, 0)?;
        let offset = FixedOffset::east_opt(BANGKOK_OFFSET_SECS)?;
        offset
            .from_local_datetime(&local)
            .single()
            .map(|t| t.with_timezone(&Utc))
    })
}

/// The first `<day> <month> <year>` on the line, and the text after it.
fn thai_date(chars: &[char]) -> Option<(NaiveDate, &[char])> {
    (0..chars.len()).find_map(|start| {
        let (month, after_month) = THAI_MONTHS
            .iter()
            .zip(1u32..)
            .find_map(|(name, month)| match_month(chars, start, name).map(|end| (month, end)))?;
        let day = digits_before(chars, start)?;
        let (year, after_year) = digits_after(chars, after_month)?;
        let year = gregorian_year(year)?;
        let date = NaiveDate::from_ymd_opt(year, month, day.value)?;
        Some((date, &chars[after_year..]))
    })
}

/// Match a month name at `start`, with each `.` optional (OCR drops them).
/// Returns the index just past the match.
fn match_month(chars: &[char], start: usize, name: &str) -> Option<usize> {
    let mut at = start;
    for want in name.chars().filter(|c| *c != '.') {
        match chars.get(at) {
            Some(c) if *c == want => at += 1,
            _ => return None,
        }
        if chars.get(at) == Some(&'.') {
            at += 1;
        }
    }
    Some(at)
}

struct Number {
    value: u32,
    digits: usize,
}

/// A 1–2 digit day right before `end` (spaces allowed), not part of a longer
/// number.
fn digits_before(chars: &[char], end: usize) -> Option<Number> {
    let mut stop = end;
    while stop > 0 && chars[stop - 1] == ' ' {
        stop -= 1;
    }
    let mut begin = stop;
    while begin > 0 && chars[begin - 1].is_ascii_digit() {
        begin -= 1;
    }
    let number = to_number(&chars[begin..stop])?;
    match number.digits {
        1 | 2 => Some(number),
        _ => None,
    }
}

/// A number right after `start` (spaces allowed), and the index past it.
fn digits_after(chars: &[char], start: usize) -> Option<(Number, usize)> {
    let mut begin = start;
    while chars.get(begin) == Some(&' ') {
        begin += 1;
    }
    let mut end = begin;
    while chars.get(end).is_some_and(char::is_ascii_digit) {
        end += 1;
    }
    Some((to_number(&chars[begin..end])?, end))
}

fn to_number(digits: &[char]) -> Option<Number> {
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    let value = digits
        .iter()
        .try_fold(0u32, |acc, c| c.to_digit(10).map(|d| acc * 10 + d))?;
    Some(Number {
        value,
        digits: digits.len(),
    })
}

/// Slips print the Buddhist-era year, as `69` or `2569`. A four-digit year
/// starting with 20 is taken as already Gregorian.
fn gregorian_year(year: Number) -> Option<i32> {
    let value = i32::try_from(year.value).ok()?;
    match (year.digits, value) {
        (2, yy) => Some(2500 + yy - 543),
        (4, be @ 2500..=2599) => Some(be - 543),
        (4, ce @ 2000..=2099) => Some(ce),
        _ => None,
    }
}

/// The first `HH:MM` that is not part of a longer number. Only `:` counts:
/// with `.` allowed, an amount such as "20.00" would read as a time.
fn clock_time(chars: &[char]) -> Option<(u32, u32)> {
    chars.windows(5).enumerate().find_map(|(i, w)| {
        let digit = |c: char| c.to_digit(10);
        let bounded = !i
            .checked_sub(1)
            .and_then(|prev| chars.get(prev))
            .is_some_and(char::is_ascii_digit)
            && !chars.get(i + 5).is_some_and(char::is_ascii_digit);
        if w[2] != ':' || !bounded {
            return None;
        }
        let hour = digit(w[0])? * 10 + digit(w[1])?;
        let minute = digit(w[3])? * 10 + digit(w[4])?;
        (hour < 24 && minute < 60).then_some((hour, minute))
    })
}
