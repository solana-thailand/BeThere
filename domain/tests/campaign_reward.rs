//! Campaign reward resolution tests (plan 016 P2.2).
//!
//! `resolve_reward` is the single source of truth for what a campaign reward
//! mints: the worker's mint path and the admin create-form preview card both
//! call it. These tests pin the behaviour both sides depend on — in particular
//! the blank-is-unset rule, which is what makes the preview honest.

use event_checkin_domain::models::campaign::{
    ResolvedReward, default_reward_description, default_reward_name, resolve_reward,
    resolve_reward_str, reward_config_field,
};
use serde_json::json;

const TITLE: &str = "Solana Hacker Series";

#[test]
fn configured_values_win_over_defaults() {
    let config = json!({
        "name": "Builder Badge",
        "description": "You shipped.",
        "image_url": "https://example.com/badge.png",
    });
    assert_eq!(
        resolve_reward(TITLE, &config),
        ResolvedReward {
            name: "Builder Badge".to_string(),
            description: "You shipped.".to_string(),
            image_url: "https://example.com/badge.png".to_string(),
        }
    );
}

#[test]
fn missing_keys_fall_back_to_title_defaults() {
    let resolved = resolve_reward(TITLE, &json!({}));
    assert_eq!(resolved.name, "Solana Hacker Series - Campaign Complete");
    assert_eq!(resolved.description, "Completed the Solana Hacker Series campaign");
    assert_eq!(resolved.image_url, "");
}

/// The regression this whole module exists for: the admin form serialises
/// untouched fields as `""`, not as absent keys. Before this logic was shared,
/// a blank name minted an NFT literally called "" while the form's hint
/// promised the title-based default.
#[test]
fn empty_strings_are_treated_as_unset() {
    let config = json!({ "name": "", "description": "", "image_url": "" });
    let resolved = resolve_reward(TITLE, &config);
    assert_eq!(resolved.name, default_reward_name(TITLE));
    assert_eq!(resolved.description, default_reward_description(TITLE));
    assert_eq!(resolved.image_url, "");
}

#[test]
fn whitespace_only_strings_are_treated_as_unset() {
    let config = json!({ "name": "   ", "description": "\t\n" });
    let resolved = resolve_reward(TITLE, &config);
    assert_eq!(resolved.name, default_reward_name(TITLE));
    assert_eq!(resolved.description, default_reward_description(TITLE));
}

#[test]
fn configured_values_are_trimmed() {
    let config = json!({ "name": "  Builder Badge  " });
    assert_eq!(resolve_reward(TITLE, &config).name, "Builder Badge");
}

#[test]
fn non_string_values_are_treated_as_unset() {
    let config = json!({ "name": 42, "description": ["a"], "image_url": null });
    let resolved = resolve_reward(TITLE, &config);
    assert_eq!(resolved.name, default_reward_name(TITLE));
    assert_eq!(resolved.description, default_reward_description(TITLE));
    assert_eq!(resolved.image_url, "");
}

#[test]
fn field_reader_reports_unset_consistently() {
    let config = json!({ "set": "x", "blank": "", "spaces": " ", "number": 1 });
    assert_eq!(reward_config_field(&config, "set"), Some("x"));
    assert_eq!(reward_config_field(&config, "blank"), None);
    assert_eq!(reward_config_field(&config, "spaces"), None);
    assert_eq!(reward_config_field(&config, "number"), None);
    assert_eq!(reward_config_field(&config, "absent"), None);
}

#[test]
fn title_is_interpolated_into_both_defaults() {
    assert_eq!(default_reward_name("Devcon"), "Devcon - Campaign Complete");
    assert_eq!(
        default_reward_description("Devcon"),
        "Completed the Devcon campaign"
    );
}

#[test]
fn empty_title_still_produces_the_default_shape() {
    // Not a useful NFT name, but it must not panic or produce something wilder
    // than the template — the campaign title is required at create time anyway.
    assert_eq!(default_reward_name(""), " - Campaign Complete");
}

/// The stored column is a JSON *string*; an operator can hand-edit it.
#[test]
fn unparseable_config_resolves_to_defaults() {
    let resolved = resolve_reward_str(TITLE, "{not json");
    assert_eq!(resolved.name, default_reward_name(TITLE));
    assert_eq!(resolved.description, default_reward_description(TITLE));
    assert_eq!(resolved.image_url, "");
}

#[test]
fn empty_config_string_resolves_to_defaults() {
    assert_eq!(resolve_reward_str(TITLE, ""), resolve_reward(TITLE, &json!(null)));
}

#[test]
fn json_null_config_resolves_to_defaults() {
    let resolved = resolve_reward(TITLE, &json!(null));
    assert_eq!(resolved.name, default_reward_name(TITLE));
}

// ---------------------------------------------------------------------------
// Key contract
// ---------------------------------------------------------------------------

/// The admin form builds `reward_config` from these same constants, so a
/// rename propagates by compilation rather than by convention. Pinning the
/// literal values here makes a rename a deliberate, visible act — the stored
/// column is persisted data, and changing a key silently orphans every
/// campaign already saved with the old one.
#[test]
fn reward_config_keys_are_stable() {
    use event_checkin_domain::models::campaign::{KEY_DESCRIPTION, KEY_IMAGE_URL, KEY_NAME};
    assert_eq!(KEY_NAME, "name");
    assert_eq!(KEY_DESCRIPTION, "description");
    assert_eq!(KEY_IMAGE_URL, "image_url");
}

/// Resolution must read exactly the keys the constants name — a resolver that
/// looked elsewhere would silently return all-defaults forever.
#[test]
fn resolver_reads_the_named_keys() {
    use event_checkin_domain::models::campaign::{KEY_DESCRIPTION, KEY_IMAGE_URL, KEY_NAME};
    let config = json!({
        KEY_NAME: "N",
        KEY_DESCRIPTION: "D",
        KEY_IMAGE_URL: "https://example.com/i.png",
    });
    let resolved = resolve_reward(TITLE, &config);
    assert_eq!(resolved.name, "N");
    assert_eq!(resolved.description, "D");
    assert_eq!(resolved.image_url, "https://example.com/i.png");
}

/// Fields the admin form stores for reference must not leak into the resolved
/// mint metadata — the preview card shows only what actually mints.
#[test]
fn unminted_fields_do_not_affect_resolution() {
    let config = json!({
        "symbol": "BUILDER",
        "metadata_uri": "https://arweave.net/x",
        "collection_mint": "So11111111111111111111111111111111111111112",
    });
    let resolved = resolve_reward(TITLE, &config);
    assert_eq!(resolved.name, default_reward_name(TITLE));
    assert_eq!(resolved.description, default_reward_description(TITLE));
    assert_eq!(resolved.image_url, "");
}
