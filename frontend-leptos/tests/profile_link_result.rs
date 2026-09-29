//! `/profile`'s link-callback banner (`?email_link=` / `?linked=` / `?error=`)
//! is parsed once and translated per locale, with Thai text for every outcome.

use event_checkin_frontend::i18n::Locale;
use event_checkin_frontend::pages::profile_link_result::LinkResult;

fn query<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.to_string())
    }
}

#[test]
fn email_link_wins_then_linked_then_error() {
    let all = [
        ("error", "github_denied"),
        ("linked", "github"),
        ("email_link", "linked"),
    ];
    assert_eq!(
        LinkResult::from_query(query(&all)),
        Some(LinkResult::Email("linked".into()))
    );
    let two = [("error", "github_denied"), ("linked", "github")];
    assert_eq!(
        LinkResult::from_query(query(&two)),
        Some(LinkResult::Linked("github".into()))
    );
    assert_eq!(LinkResult::from_query(query(&[("next", "/")])), None);
}

#[test]
fn outcomes_keep_their_success_flag_in_both_locales() {
    let cases = [
        (LinkResult::Email("linked".into()), true),
        (LinkResult::Email("already".into()), true),
        (LinkResult::Email("conflict".into()), false),
        (LinkResult::Email("nonsense".into()), false),
        (LinkResult::Linked("github".into()), true),
        (LinkResult::Error("telegram_expired".into()), false),
    ];
    for (result, ok) in cases {
        for locale in [Locale::en, Locale::th] {
            assert_eq!(result.message(locale).0, ok, "{result:?} {locale:?}");
        }
    }
}

#[test]
fn every_outcome_has_thai_text_distinct_from_english() {
    let results = [
        "linked",
        "already",
        "same",
        "conflict",
        "session_mismatch",
        "needs_google",
        "expired",
        "cancelled",
        "other",
    ]
    .map(|r| LinkResult::Email(r.into()))
    .into_iter()
    .chain(["github", "telegram", "discord"].map(|p| LinkResult::Linked(p.into())))
    .chain(
        [
            "github_denied",
            "github_state_expired",
            "github_invalid_state",
            "github_token_failed",
            "github_user_failed",
            "db_unavailable",
            "telegram_invalid",
            "telegram_expired",
            "telegram_unconfigured",
            "weird_code",
        ]
        .map(|c| LinkResult::Error(c.into())),
    );
    for result in results {
        let en = result.message(Locale::en).1;
        let th = result.message(Locale::th).1;
        assert_ne!(en, th, "{result:?} has no Thai text");
        assert!(!th.contains('{'), "{result:?} left a placeholder: {th}");
    }
}

#[test]
fn unknown_provider_and_code_are_filled_in() {
    let linked = LinkResult::Linked("discord".into()).message(Locale::th).1;
    assert!(linked.contains("discord"), "{linked}");
    let failed = LinkResult::Error("weird_code".into()).message(Locale::en).1;
    assert_eq!(
        failed,
        "Account linking failed (weird_code). Please try again."
    );
}
