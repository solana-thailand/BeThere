//! The EN/TH catalog (`locales/`, .plans/037 §2).
//!
//! - Both locales carry the same keys. `leptos_i18n` falls back to EN for a
//!   key missing from TH and only warns at build time, so a TH reader would
//!   silently get English.
//! - The `/feedback` option labels in TH are the stored answer values. The
//!   answers are the DevRel Google Form's Thai wording, which the Phase 1
//!   report compares against. The UI language may change what the reader
//!   sees, never what gets stored.
//! - The switch shows on attendee pages only.

use std::collections::BTreeSet;
use std::path::PathBuf;

use event_checkin_frontend::i18n::{Locale, td_string};
use event_checkin_frontend::locale::{date_tag, is_attendee_path, parse_locale};

fn catalog(locale: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("locales/{locale}.json"));
    let text = std::fs::read_to_string(&path).expect("locale file");
    serde_json::from_str(&text).expect("locale JSON")
}

fn keys(value: &serde_json::Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                keys(child, &format!("{prefix}{key}."), out);
            }
        }
        _ => {
            out.insert(prefix.trim_end_matches('.').to_string());
        }
    }
}

#[test]
fn en_and_th_have_the_same_keys() {
    let (mut en, mut th) = (BTreeSet::new(), BTreeSet::new());
    keys(&catalog("en"), "", &mut en);
    keys(&catalog("th"), "", &mut th);
    let only_en: Vec<_> = en.difference(&th).collect();
    let only_th: Vec<_> = th.difference(&en).collect();
    assert!(
        only_en.is_empty() && only_th.is_empty(),
        "EN only: {only_en:?}; TH only: {only_th:?}"
    );
}

#[test]
fn th_feedback_labels_are_the_stored_values() {
    let feedback = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/pages/public/feedback.rs"),
    )
    .expect("feedback.rs");

    let thai = Locale::th;
    for (label, stored) in [
        (td_string!(thai, feedback.scale.low), "ไม่พึงพอใจ"),
        (td_string!(thai, feedback.scale.mid), "พึงพอใจ"),
        (td_string!(thai, feedback.scale.high), "พึงพอใจมาก"),
        (td_string!(thai, feedback.watched.live), "ได้ดูสด"),
        (td_string!(thai, feedback.watched.replay), "ดูย้อนหลัง"),
        (td_string!(thai, feedback.watched.timing), "ไม่ได้ดู — ติดเวลา"),
        (
            td_string!(thai, feedback.watched.topic),
            "ไม่ได้ดู — หัวข้อไม่ตรงที่สนใจ",
        ),
        (td_string!(thai, feedback.watched.language), "ไม่ได้ดู — ภาษา"),
        (
            td_string!(thai, feedback.watched.unaware),
            "ไม่ได้ดู — ไม่รู้ว่าไลฟ์แล้ว",
        ),
        (
            td_string!(thai, feedback.latent.join),
            "อยากให้จัดต่อ และจะเข้าร่วม",
        ),
        (
            td_string!(thai, feedback.latent.replay),
            "อยากให้จัดต่อ แต่จะดูย้อนหลังเอา",
        ),
        (td_string!(thai, feedback.latent.neutral), "เฉย ๆ"),
        (
            td_string!(thai, feedback.latent.unaware),
            "ไม่เคยดู และไม่ทราบว่ามีซีรีส์นี้",
        ),
    ] {
        assert_eq!(label, stored, "TH label drifted from the stored value");
        assert!(
            feedback.contains(&format!("\"{stored}\"")),
            "feedback.rs no longer stores {stored:?}"
        );
    }
}

#[test]
fn en_feedback_labels_are_english() {
    let english = Locale::en;
    for label in [
        td_string!(english, feedback.scale.low),
        td_string!(english, feedback.watched.live),
        td_string!(english, feedback.latent.join),
        td_string!(english, feedback.dim.content),
    ] {
        assert!(
            !label
                .chars()
                .any(|c| ('\u{0E00}'..='\u{0E7F}').contains(&c)),
            "{label:?} is Thai in the EN catalog"
        );
    }
}

#[test]
fn switch_shows_on_attendee_pages_only() {
    for path in [
        "/",
        "/login",
        "/e/rtm-6",
        "/deposit/att-1",
        "/ticket/att-1",
        "/claim/tok",
        "/discover",
        "/feedback",
        "/privacy",
    ] {
        assert!(is_attendee_path(path), "{path} should be bilingual");
    }
    for path in ["/admin", "/staff", "/dashboard/live", "/events/e1/summary"] {
        assert!(!is_attendee_path(path), "{path} stays English");
    }
}

#[test]
fn stored_picks_parse_and_dates_follow_the_locale() {
    assert_eq!(parse_locale("th"), Some(Locale::th));
    assert_eq!(parse_locale("en"), Some(Locale::en));
    assert_eq!(parse_locale("fr"), None);
    assert_eq!(date_tag(Locale::en), "en-GB");
    assert_eq!(date_tag(Locale::th), "th-TH");
}
