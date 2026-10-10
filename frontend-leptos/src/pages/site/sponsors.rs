//! `/sponsors` (.plans/045 R4.6): the landing's sponsor section as its own
//! page. The `#contact` card anchor stays, so old links still land on it.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::i18n::td_string;
use crate::locale::tr;
use crate::pages::landing::frame::{SiteFrame, use_site_auth};
use crate::pages::landing::sponsors::Sponsors;

use super::doors::SitePage;

#[component]
pub fn SponsorsPage() -> impl IntoView {
    let (auth_state, _) = use_site_auth();
    view! {
        <Title text=tr(|l| td_string!(l, landing.site.title_sponsors)) />
        <SiteFrame here=SitePage::Sponsors auth_state=auth_state>
            <Sponsors page_title=true />
        </SiteFrame>
    }
}
