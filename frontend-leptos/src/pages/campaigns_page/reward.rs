//! NFT reward config: the persisted JSON shape and its live preview card.

use leptos::prelude::*;

use crate::icons::{Icon, IconName};

/// Build the `reward_config` JSON object from the individual form fields.
///
/// Shared by the save path and the preview card so the card previews exactly
/// what will be persisted. Blank fields are written through as `""` rather than
/// omitted — that is the historical shape of this column, and
/// `domain::models::campaign` treats blank as unset when resolving defaults.
///
/// The three minted fields are keyed off the domain constants rather than
/// string literals: renaming one there is then a compile-time change here, so
/// the form and the mint resolver cannot silently drift apart. The other three
/// are stored for the organizer's reference and never reach the mint request.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_reward_config(
    name: &str,
    symbol: &str,
    description: &str,
    image_url: &str,
    metadata_uri: &str,
    collection_mint: &str,
) -> serde_json::Value {
    use event_checkin_domain::models::campaign as reward;

    serde_json::json!({
        reward::KEY_NAME: name,
        reward::KEY_DESCRIPTION: description,
        reward::KEY_IMAGE_URL: image_url,
        "symbol": symbol,
        "metadata_uri": metadata_uri,
        "collection_mint": collection_mint,
    })
}

/// Live "what gets minted" card for the NFT reward section (plan 016 P2.2).
///
/// Resolves through `domain::models::campaign::resolve_reward` — the very
/// function the worker's mint path calls — rather than re-deriving the defaults
/// here, so the card cannot drift from what actually mints.
///
/// Only the three fields that reach the mint request are shown. Symbol,
/// Metadata URI and Collection Mint are stored on the campaign but are not part
/// of the minted metadata, so previewing them would imply otherwise.
pub(super) fn nft_preview_card(title: &str, rc: &serde_json::Value) -> AnyView {
    use event_checkin_domain::models::campaign as reward;

    let resolved = reward::resolve_reward(title, rc);
    // Both textual defaults interpolate the title. With no title typed yet they
    // read as " - Campaign Complete", which looks like a bug rather than a
    // default — prompt for the title instead.
    let needs_title = title.trim().is_empty();
    let name_is_default = reward::reward_config_field(rc, "name").is_none();
    let desc_is_default = reward::reward_config_field(rc, "description").is_none();

    let line =
        |value: String, is_default: bool, prompt: &'static str| match is_default && needs_title {
            true => (prompt.to_string(), true, false),
            false => (value, false, is_default),
        };
    let (name_text, name_pending, name_tagged) = line(
        resolved.name,
        name_is_default,
        "Set a title to preview the minted name",
    );
    let (desc_text, desc_pending, desc_tagged) = line(
        resolved.description,
        desc_is_default,
        "Set a title to preview the minted description",
    );

    let image_url = resolved.image_url;
    let has_image = !image_url.is_empty();
    let default_tag =
        |shown: bool| shown.then(|| view! { <span class="nft-preview-tag">"default"</span> });
    let pending_class = |pending: bool| match pending {
        true => "nft-preview-pending",
        false => "",
    };

    view! {
        <div class="nft-preview-card">
            <div class="nft-preview-media">
                // Placeholder sits underneath; a failed <img> uncovers it.
                <Icon icon=IconName::Trophy class="icon-lg" />
                {has_image
                    .then(|| {
                        view! {
                            <img
                                class="nft-preview-img"
                                src=image_url.clone()
                                alt=""
                                on:error=move |ev| {
                                    // A dead artwork URL should fall back to the
                                    // placeholder, not a broken-image glyph.
                                    use wasm_bindgen::JsCast;
                                    if let Some(el) = ev
                                        .target()
                                        .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                                    {
                                        let _ = el.style().set_property("display", "none");
                                    }
                                }
                            />
                        }
                    })}
            </div>
            <div class="nft-preview-meta">
                <p class=format!("nft-preview-name {}", pending_class(name_pending))>
                    {name_text}
                    {default_tag(name_tagged)}
                </p>
                <p class=format!("nft-preview-desc {}", pending_class(desc_pending))>
                    {desc_text}
                    {default_tag(desc_tagged)}
                </p>
            </div>
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;
    use event_checkin_domain::models::campaign as reward;

    // --- reward_config contract (plan 016 P2.2) ----------------------------

    fn config() -> serde_json::Value {
        build_reward_config("", "BUILDER", "", "", "https://arweave.net/x", "mint111")
    }

    /// The form must write the exact keys the mint resolver reads, or both the
    /// preview and the mint would silently fall back to defaults forever.
    #[test]
    fn build_reward_config_writes_the_resolver_keys() {
        let rc = config();
        assert!(rc.get(reward::KEY_NAME).is_some());
        assert!(rc.get(reward::KEY_DESCRIPTION).is_some());
        assert!(rc.get(reward::KEY_IMAGE_URL).is_some());
    }

    /// Fields stored for the organizer's reference are persisted but must not
    /// reach the minted metadata — the preview card omits them for that reason.
    #[test]
    fn build_reward_config_keeps_unminted_fields_out_of_resolution() {
        let rc = config();
        assert_eq!(rc.get("symbol").and_then(|v| v.as_str()), Some("BUILDER"));
        assert_eq!(
            rc.get("metadata_uri").and_then(|v| v.as_str()),
            Some("https://arweave.net/x")
        );

        let resolved = reward::resolve_reward("My Campaign", &rc);
        assert_eq!(resolved.name, reward::default_reward_name("My Campaign"));
        assert_eq!(
            resolved.description,
            reward::default_reward_description("My Campaign")
        );
        assert_eq!(resolved.image_url, "");
    }

    /// Blank inputs are written as `""`, not omitted — the preview relies on the
    /// resolver treating that as unset.
    #[test]
    fn blank_form_fields_round_trip_to_defaults() {
        let rc = build_reward_config("", "", "", "", "", "");
        assert_eq!(rc.get(reward::KEY_NAME).and_then(|v| v.as_str()), Some(""));

        let resolved = reward::resolve_reward("Devcon", &rc);
        assert_eq!(resolved.name, "Devcon - Campaign Complete");
        assert_eq!(resolved.description, "Completed the Devcon campaign");
    }

    #[test]
    fn filled_form_fields_survive_resolution() {
        let rc = build_reward_config(
            "Builder Badge",
            "BUILDER",
            "You shipped.",
            "https://example.com/i.png",
            "",
            "",
        );
        let resolved = reward::resolve_reward("Devcon", &rc);
        assert_eq!(resolved.name, "Builder Badge");
        assert_eq!(resolved.description, "You shipped.");
        assert_eq!(resolved.image_url, "https://example.com/i.png");
    }
}
