//! The OCR-text parser (`.plans/035` §3.1). The fixtures are the raw text
//! tesseract.js returned for the synthetic slips of `.benchmarks/001`, plus
//! the misreads that bench found. The property that matters most: a wrong
//! figure is worse than none.

use chrono::{DateTime, TimeZone, Utc};
use event_checkin_domain::slip_ocr::{MAX_OCR_TEXT_BYTES, facts_from_ocr_text};

fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> Option<DateTime<Utc>> {
    Some(Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap())
}

// KBank-style layout, `tha` data (bench slip s00). Note the decomposed sara am
// in "จํานวน" and the misread masked accounts.
const KBANK: &str = "-   ๐   ๕\nโอนเงินสําเร็จ\n17 เม.ย. 69 13:59 น.\nใแล. )0หพพ 005ธ\n\
ธ.กสิกรไทย >0๐๐-%- 8829-%\nนาง นก ทองคํา\nพร้อมเพย์ ๓๐๐9๐๐๕๐-9237\nเลขที่รายการ:\n\
0745596885681158089973\nจํานวน:\n300.00 บาท\nค่าธรรมเนียม: 0.00 บาท\nรผาทธา1๐ 15ร51 รม๒\n";

// SCB-style (s01): four-digit BE year, amount on the label line.
const SCB: &str = "โอนเงินสําเร็จ\n6 ธ.ค. 2569 - 01:23\nรหัสอ้างอิง: 809578776359558\n\
จาก นาย สมชาย ใจดี\n2๐๐-9๐๐ 7063-%\nไปยัง นาย สมชาย ใจดี\n2๐๐-๐๐-9316\n\
จํานวนเงิน                                                                   1,500.00\n";

// BBL-style (s02): the amount on the line after its label, a comma after the year.
const BBL: &str = "รายการสําเร็จ\n\nจํานวนเงิน (บาท)\n\n1,500.00\n\nค่าธรรมเนียม (บาท) 0.00\n\n\
วันที่ทํารายการ 2 ส.ค. 69, 11:12\n\nหมายเลขอ้างอิง 027205497026443\n";

#[test]
fn kbank_layout_reads_amount_and_bangkok_time_as_utc() {
    let facts = facts_from_ocr_text(KBANK);
    assert_eq!(facts.amount_satang, Some(30_000));
    // 13:59 Bangkok = 06:59 UTC; BE 2569 = 2026.
    assert_eq!(facts.transferred_at, utc(2026, 4, 17, 6, 59));
    assert_eq!(facts.bank_ref, None);
    assert_eq!(facts.receiver_account, None);
}

#[test]
fn scb_layout_four_digit_year_crosses_midnight_in_utc() {
    let facts = facts_from_ocr_text(SCB);
    assert_eq!(facts.amount_satang, Some(150_000));
    assert_eq!(facts.transferred_at, utc(2026, 12, 5, 18, 23));
}

#[test]
fn bbl_layout_amount_on_the_line_after_its_label() {
    let facts = facts_from_ocr_text(BBL);
    assert_eq!(facts.amount_satang, Some(150_000));
    assert_eq!(facts.transferred_at, utc(2026, 8, 2, 4, 12));
}

#[test]
fn eng_data_without_thai_labels_falls_back_to_the_single_figure() {
    // `eng` data turns every Thai word to Latin noise (bench s00, eng).
    let text = "TawWiudise\n17 wea. 69 13:59 u.\nMR. JOHN DOE\nlanfisrins:\n\
0745596885681 15BOR9973\naway:\n300.00 un\nAsssNLAa: O.0O uM\n";
    let facts = facts_from_ocr_text(text);
    assert_eq!(facts.amount_satang, Some(30_000));
    assert_eq!(facts.transferred_at, None, "no Thai month, so no date");
}

#[test]
fn misread_thousands_separator_is_folded_not_truncated() {
    // Bench: "1,500.00" came back as "1.500.00" and a naive scan took 500.00.
    let facts = facts_from_ocr_text("จํานวนเงิน 1.500.00\n");
    assert_eq!(facts.amount_satang, Some(150_000));
}

#[test]
fn a_figure_inside_a_longer_run_is_not_lifted_out() {
    for line in [
        "จำนวน 91500.00",
        "จำนวน 1,50.00",
        "จำนวน 500.000",
        "จำนวน 500.00.",
        "จำนวน 12,3456.00",
    ] {
        assert_eq!(facts_from_ocr_text(line).amount_satang, None, "{line}");
    }
}

#[test]
fn fee_line_and_zero_are_never_the_amount() {
    let text = "ค่าธรรมเนียม: 25.00 บาท\nจำนวน: 0.00 บาท\n";
    assert_eq!(facts_from_ocr_text(text).amount_satang, None);
    assert_eq!(facts_from_ocr_text("Fee 10.00\n").amount_satang, None);
}

#[test]
fn two_unlabelled_figures_are_ambiguous() {
    assert_eq!(facts_from_ocr_text("500.00\n300.00\n").amount_satang, None);
    assert_eq!(
        facts_from_ocr_text("500.00\n500.00\n").amount_satang,
        Some(50_000)
    );
}

#[test]
fn an_amount_is_never_read_as_a_time() {
    // With "." accepted as a time separator, 20.00 THB would read as 20:00.
    let text = "12 ก.ย. 69 20.00\nจำนวน 20.00 บาท\n";
    let facts = facts_from_ocr_text(text);
    assert_eq!(facts.transferred_at, None);
    assert_eq!(facts.amount_satang, Some(2_000));
}

#[test]
fn date_needs_a_year_and_a_time_on_the_same_line() {
    assert_eq!(facts_from_ocr_text("17 เม.ย. 13:59\n").transferred_at, None);
    assert_eq!(
        facts_from_ocr_text("17 เม.ย. 69\n13:59\n").transferred_at,
        None
    );
    assert_eq!(
        facts_from_ocr_text("31 ก.พ. 69 10:00\n").transferred_at,
        None
    );
    assert_eq!(
        facts_from_ocr_text("17 เม.ย. 69 24:10\n").transferred_at,
        None
    );
    assert_eq!(
        facts_from_ocr_text("117 เม.ย. 69 10:00\n").transferred_at,
        None
    );
}

#[test]
fn month_dots_are_optional_but_letters_are_not() {
    assert_eq!(
        facts_from_ocr_text("3 มีค 69 09:05\n").transferred_at,
        utc(2026, 3, 3, 2, 5)
    );
    // "มค" must not match inside "มีค": that would be January.
    assert_ne!(
        facts_from_ocr_text("3 มีค 69 09:05\n").transferred_at,
        utc(2026, 1, 3, 2, 5)
    );
    assert_eq!(facts_from_ocr_text("3 มxค 69 09:05\n").transferred_at, None);
}

#[test]
fn text_over_the_cap_parses_to_nothing() {
    let mut text = String::from("จำนวน 500.00 บาท\n");
    text.push_str(&"x".repeat(MAX_OCR_TEXT_BYTES));
    assert_eq!(facts_from_ocr_text(&text), Default::default());
}

#[test]
fn decomposed_sara_am_label_still_picks_the_labelled_figure() {
    // Tesseract writes "จํานวน" (U+0E4D U+0E32). Unfolded, the label is missed,
    // and with a second figure on the slip the amount would be ambiguous.
    let text = "จ\u{0E4D}\u{0E32}นวน 500.00\nยอดคงเหลือ 1,200.00\n";
    assert_eq!(facts_from_ocr_text(text).amount_satang, Some(50_000));
}
