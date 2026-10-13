//! Loading skeletons (.plans/046 Phase 2): the shape of the page that is on
//! its way, in place of a spinner and a "Loading…" line, so a slow venue
//! connection reads as content arriving. One builder, a layout per page.
//!
//! The bones reuse the event page's `.pe-skel-*` styles (style-19); inside the
//! site frame they are square paper-2 blocks (style-24), and they hold still
//! under `prefers-reduced-motion`.

use leptos::prelude::*;

use crate::i18n::Locale;

/// One placeholder block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bone {
    /// A poster or a QR: tall.
    Hero,
    /// A page title.
    Title,
    /// A line under the title.
    Sub,
    /// A button.
    Cta,
    /// A card.
    Card,
}

impl Bone {
    fn class(self) -> &'static str {
        match self {
            Bone::Hero => "pe-skel pe-skel-hero",
            Bone::Title => "pe-skel pe-skel-title",
            Bone::Sub => "pe-skel pe-skel-sub",
            Bone::Cta => "pe-skel pe-skel-cta",
            Bone::Card => "pe-skel pe-skel-card",
        }
    }
}

/// `/e/{slug}`: poster, name, line, the reserve button, two cards.
pub const EVENT: &[Bone] = &[
    Bone::Hero,
    Bone::Title,
    Bone::Sub,
    Bone::Cta,
    Bone::Card,
    Bone::Card,
];
/// `/ticket/…`: the status banner, the ticket (name, line, QR), a card.
pub const TICKET: &[Bone] = &[Bone::Card, Bone::Title, Bone::Sub, Bone::Hero, Bone::Card];
/// `/deposit/…` under its fixed title: the event, the amount card, a button.
pub const DEPOSIT: &[Bone] = &[Bone::Sub, Bone::Card, Bone::Cta];
/// A form page: title, intro, two field groups, the submit.
pub const FORM: &[Bone] = &[Bone::Title, Bone::Sub, Bone::Card, Bone::Card, Bone::Cta];
/// A list inside a page section (`/events`): three rows.
pub const LIST: &[Bone] = &[Bone::Card, Bone::Card, Bone::Card];

/// The skeleton for `bones`, announced to screen readers by `label`.
/// `full_page` keeps the footer below the fold while loading (`.issues/175`);
/// a list inside a section takes only its own height.
pub fn skeleton(
    bones: &'static [Bone],
    label: fn(Locale) -> &'static str,
    full_page: bool,
) -> AnyView {
    let class = match full_page {
        true => "pe-skeleton",
        false => "pe-skeleton pe-skeleton--inline",
    };
    view! {
        <div class=class role="status" aria-busy="true" aria-label=crate::locale::tr(label)>
            {bones.iter().map(|bone| view! { <div class=bone.class()></div> }).collect_view()}
        </div>
    }
    .into_any()
}
