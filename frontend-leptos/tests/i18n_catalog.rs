//! The EN/TH catalog (`locales/{en,th}/<namespace>.json`, .plans/037 §2).
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
use event_checkin_frontend::locale::{date_tag, date_tag_on, is_attendee_path, parse_locale};

/// Every namespace file of one locale, keyed by namespace, as one JSON object.
fn catalog(locale: &str) -> serde_json::Value {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("locales")
        .join(locale);
    let mut all = serde_json::Map::new();
    for entry in std::fs::read_dir(&dir).expect("locale dir").flatten() {
        let path = entry.path();
        let ns = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("file name");
        let text = std::fs::read_to_string(&path).expect("namespace file");
        all.insert(
            ns.to_string(),
            serde_json::from_str(&text).expect("namespace JSON"),
        );
    }
    serde_json::Value::Object(all)
}

/// Namespaces registered in `[package.metadata.leptos-i18n]`.
fn registered_namespaces() -> BTreeSet<String> {
    let manifest =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .expect("Cargo.toml");
    let line = manifest
        .lines()
        .find(|l| l.starts_with("namespaces = "))
        .expect("namespaces line");
    line.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

#[test]
fn every_namespace_file_is_registered_and_present_in_both_locales() {
    let registered = registered_namespaces();
    for locale in ["en", "th"] {
        let on_disk: BTreeSet<String> = catalog(locale)
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            on_disk, registered,
            "locales/{locale}/ vs Cargo.toml namespaces"
        );
    }
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
        "/data-privacy",
        "/past-events",
        "/faq",
        "/profile",
        "/events/rtm-6/recap",
        "/events/rtm-6/post-event-register",
    ] {
        assert!(is_attendee_path(path), "{path} should be bilingual");
    }
    for path in [
        "/admin",
        "/staff",
        "/dashboard/live",
        "/events/e1/summary",
        "/events/e1/pr-pack",
    ] {
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

/// A Thai pick dates attendee pages in Buddhist era but never the
/// English-only staff pages (.issues/192: BE year beside English text).
#[test]
fn english_only_pages_keep_gregorian_dates_under_a_thai_pick() {
    assert_eq!(date_tag_on(Locale::th, "/ticket/a1"), "th-TH");
    assert_eq!(date_tag_on(Locale::th, "/"), "th-TH");
    for path in ["/admin", "/staff", "/dashboard/live", "/events/e1/summary"] {
        assert_eq!(date_tag_on(Locale::th, path), "en-GB", "{path}");
    }
    assert_eq!(date_tag_on(Locale::en, "/ticket/a1"), "en-GB");
}
