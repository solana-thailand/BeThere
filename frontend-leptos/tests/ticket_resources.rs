//! The ticket's Resources card: which community links it lists.

use event_checkin_frontend::api::CommunityLink;
use event_checkin_frontend::pages::ticket::resources::learning_resources;

fn link(platform: &str, url: &str) -> CommunityLink {
    CommunityLink {
        platform: platform.into(),
        url: url.into(),
        label: String::new(),
    }
}

#[test]
fn lists_the_four_resource_kinds_in_the_organizers_order() {
    let links = [
        link("download", "https://example.com/d"),
        link("discord", "https://discord.gg/x"),
        link("slides", "https://example.com/s"),
        link("guide", "https://example.com/g"),
        link("source", "https://example.com/c"),
        link("resource", "https://example.com/r"),
    ];
    let kinds: Vec<_> = learning_resources(&links)
        .into_iter()
        .map(|l| l.platform)
        .collect();
    assert_eq!(kinds, ["download", "slides", "source", "resource"]);
}

#[test]
fn only_https_links_are_listed() {
    let links = [
        link("slides", "javascript:alert(1)"),
        link("slides", "http://example.com/s"),
        link("slides", ""),
        link("slides", " https://example.com/ok"),
    ];
    let urls: Vec<_> = learning_resources(&links)
        .into_iter()
        .map(|l| l.url)
        .collect();
    assert_eq!(urls, [" https://example.com/ok"]);
}
