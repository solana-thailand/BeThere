//! Campaign reward metadata resolution (shared SSOT).
//!
//! A campaign's `reward_config` is stored as free-form JSON, and every field in
//! it is optional. Deciding what a reward *actually* mints therefore takes a
//! resolution step, and that step has two callers that must never disagree:
//!
//!   - `worker`: the mint path in `handlers::campaigns::claim_campaign_reward`,
//!   - `frontend-leptos`: the create-form preview card (plan 016 P2.2), which
//!     promises the organizer "this is what will be minted".
//!
//! A preview that mirrored the worker's logic would be a lie waiting to happen,
//! so the logic lives here and both sides call it.

use serde_json::Value;

/// `reward_config` key holding the NFT name.
pub const KEY_NAME: &str = "name";
/// `reward_config` key holding the NFT description.
pub const KEY_DESCRIPTION: &str = "description";
/// `reward_config` key holding the NFT artwork URL.
pub const KEY_IMAGE_URL: &str = "image_url";

/// NFT name used when the campaign's `reward_config` leaves `name` blank.
pub fn default_reward_name(title: &str) -> String {
    format!("{title} - Campaign Complete")
}

/// NFT description used when `reward_config` leaves `description` blank.
pub fn default_reward_description(title: &str) -> String {
    format!("Completed the {title} campaign")
}

/// Read an optional string out of a `reward_config` object.
///
/// A missing key, a non-string value, and a blank or whitespace-only string are
/// all treated alike as "not configured". Collapsing the blank case matters:
/// the admin form serialises untouched fields as `""` rather than omitting
/// them, so a `.get(key).and_then(as_str)` check alone would read an empty
/// string as a deliberate choice and mint an NFT with no name.
///
/// The returned value is trimmed — leading/trailing whitespace in a stored
/// name is never meaningful.
pub fn reward_config_field<'a>(config: &'a Value, key: &str) -> Option<&'a str> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// The metadata a campaign reward will mint with, after defaults are applied.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedReward {
    pub name: String,
    pub description: String,
    /// Empty when unset — the mint request omits the image from metadata rather
    /// than substituting a placeholder, so there is no default to apply.
    pub image_url: String,
}

/// Resolve what a campaign reward mints, applying defaults for blank fields.
///
/// `title` is the campaign title, which both textual defaults interpolate.
pub fn resolve_reward(title: &str, config: &Value) -> ResolvedReward {
    ResolvedReward {
        name: reward_config_field(config, KEY_NAME)
            .map(str::to_string)
            .unwrap_or_else(|| default_reward_name(title)),
        description: reward_config_field(config, KEY_DESCRIPTION)
            .map(str::to_string)
            .unwrap_or_else(|| default_reward_description(title)),
        image_url: reward_config_field(config, KEY_IMAGE_URL)
            .unwrap_or_default()
            .to_string(),
    }
}

/// Resolve from a `reward_config` JSON *string* as stored on the campaign row.
///
/// Unparseable JSON resolves to all-defaults rather than erroring: the mint
/// path must not fail because an operator hand-edited the column.
pub fn resolve_reward_str(title: &str, config_json: &str) -> ResolvedReward {
    let config: Value = serde_json::from_str(config_json).unwrap_or(Value::Null);
    resolve_reward(title, &config)
}
