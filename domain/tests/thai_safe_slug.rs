//! Thai-safe slugs (build plan Phase 1.0): the ASCII names keep the slug they
//! always had; a name with too little ASCII gets a stable hash, never "".

use event_checkin_domain::slug::Slug;

fn ascii(s: &str) -> Slug {
    Slug::Ascii(s.to_string())
}

#[test]
fn ascii_names_keep_the_old_slug() {
    for (name, want) in [
        (
            "Solana x AI Builders: The Road to Mainnet #4",
            "solana-x-ai-builders-the-road-to-mainnet-4",
        ),
        ("  My   Event!! ", "my-event"),
        ("rtm_6 (Bangkok)", "rtm-6-bangkok"),
        ("AI", "ai"),
        ("---", ""),
    ] {
        let got = Slug::from_text(name);
        match want.is_empty() {
            true => assert!(matches!(got, Slug::Hashed(_)), "{name:?} → {got:?}"),
            false => assert_eq!(got, ascii(want), "{name:?}"),
        }
    }
}

#[test]
fn blank_stays_blank_so_callers_still_reject_it() {
    assert_eq!(Slug::from_text(""), ascii(""));
    assert_eq!(Slug::from_text("   \t"), ascii(""));
}

#[test]
fn all_thai_name_gets_a_prefixed_hash_not_an_empty_id() {
    let slug = Slug::from_text("เวิร์กช็อปภาษาไทย").or_prefixed("event");
    let body = slug.strip_prefix("event-").expect("prefixed");
    assert_eq!(body.len(), 6, "{slug}");
    assert!(
        body.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()),
        "{slug}"
    );
}

#[test]
fn thai_name_with_a_stub_of_ascii_is_hashed() {
    // "ครั้งที่ 1" keeps only "1": shorter than the minimum, so hashed.
    assert!(matches!(Slug::from_text("ครั้งที่ 1"), Slug::Hashed(_)));
}

#[test]
fn mixed_name_with_enough_ascii_keeps_the_ascii() {
    assert_eq!(Slug::from_text("เวิร์กช็อป Rust ครั้งที่ 1"), ascii("rust-1"));
}

#[test]
fn emoji_only_name_is_hashed() {
    assert!(matches!(Slug::from_text("🎉🎉"), Slug::Hashed(_)));
}

#[test]
fn hash_is_stable_and_tells_names_apart() {
    let a = Slug::from_text("งานรวมพลชาว Solana").into_inner();
    let b = Slug::from_text("งานรวมพลชาว Solana").into_inner();
    let c = Slug::from_text("งานรวมพลชาว Rust").into_inner();
    assert_eq!(a, b);
    assert_ne!(a, c);
    // Case and outer whitespace do not change the key.
    assert_eq!(
        Slug::from_text("  ทดสอบ ").into_inner(),
        Slug::from_text("ทดสอบ").into_inner()
    );
}

#[test]
fn hash_value_is_pinned_across_builds() {
    // Feedback series keys are stored in D1 under this body; a change to the
    // hash would orphan them. Pinned from the first build (2026-10-06).
    assert_eq!(Slug::from_text("ทดสอบ").into_inner(), PINNED);
}

const PINNED: &str = "fbocmi";
