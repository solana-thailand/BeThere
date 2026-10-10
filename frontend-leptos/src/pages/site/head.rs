//! A site page's head in the home's room (.plans/045, prototype
//! `bethere-ux/site/site.css` `.dark-head`): the RTM #6 room as a still,
//! dim dither behind the page's kicker, two-line title and sub-line, so
//! every page opens in the same place the home does. The live, lit room
//! stays on the home (`landing/room.rs`).

use leptos::prelude::*;

use crate::i18n::Locale;
use crate::locale::tr;
use crate::pages::landing::hero::Markup;

/// A catalog string, picked in the reader's language.
pub type Catalog = fn(Locale) -> &'static str;

/// The head: kicker, title (a second line in the brand colour when given),
/// sub-line (copy markup), then anything the page adds (its call to action).
#[component]
pub fn PageHead(
    kicker: Catalog,
    title: Catalog,
    #[prop(optional)] title_2: Option<Catalog>,
    sub: Catalog,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <header class="lp-head" id="top">
            <div class="lp-wrap">
                <p class="lp-kicker">{tr(kicker)}</p>
                <h1 class="lp-head-h1">
                    <span>{tr(title)}</span>
                    {title_2.map(|t| view! { <br /><span class="lp-head-h1-2">{tr(t)}</span> })}
                </h1>
                <p class="lp-head-sub"><Markup text=tr(sub) /></p>
                {children.map(|c| c())}
            </div>
        </header>
    }
}
