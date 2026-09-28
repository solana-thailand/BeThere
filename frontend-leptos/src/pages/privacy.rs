use leptos::prelude::*;
use leptos_meta::Title;

use crate::icons::{Icon, IconName};

/// Privacy Policy page — PDPA compliance.
///
/// The notice is bilingual (`locales/{en,th}/privacy.json`). The TH text is a
/// translation of the EN one, clause for clause; change both together.
#[component]
pub fn Privacy() -> impl IntoView {
    view! {
        <Title text=crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.page_title)) />
        <div class="center-page">
            <div class="container" style="max-width: 720px;">
                <div class="pe-card">
                    <h1 class="pe-section-title" style="margin-bottom: 0.5rem;">
                        <Icon icon=IconName::Lock class="icon-md" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.title))}
                    </h1>
                    <p class="pe-detail-secondary" style="margin-bottom: 1.5rem;">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.last_updated))}
                    </p>

                    // Data Controller
                    <h2 class="pe-section-title" style="font-size: 1.1rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.controller_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.controller_body))}
                    </p>

                    // Data Collected
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.collect_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.collect_intro))}
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.collect.name))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.collect.contact))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.collect.participation))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.collect.deposit))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.collect.wallet))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.collect.photo))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.collect.marketing))}</li>
                    </ul>

                    // Purpose
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.purpose_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.purpose_intro))}
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.purpose.registration))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.purpose.checkin))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.purpose.nft))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.purpose.deposit))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.purpose.followup))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.purpose.emails))}</li>
                    </ul>

                    // Legal Basis
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.basis_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.basis_body))}
                    </p>

                    // Blockchain Data
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.chain_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.chain_intro))}
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.chain.public))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.chain.immutable))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.chain.not_controlled))}</li>
                    </ul>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.chain_note))}
                    </p>

                    // Photo/Media
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.photo_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.photo_body))}
                    </p>

                    // Data Sharing
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.sharing_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.sharing_intro))}
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.sharing.organizers))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.sharing.cloudflare))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.sharing.google))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.sharing.helius))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.sharing.crossmint))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.sharing.social))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.sharing.slack))}</li>
                    </ul>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.sharing_note))}
                    </p>

                    // Data Retention
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.retention_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.retention_body))}
                    </p>

                    // Your Rights
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.rights_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.rights_intro))}
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.rights.access))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.rights.correction))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.rights.deletion))}</li>
                        <li>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.rights.withdraw))}</li>
                    </ul>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.rights_how))}
                    </p>

                    // Cookies
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.cookies_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.cookies_body))}
                    </p>

                    // Contact
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.contact_title))}</h2>
                    <p class="pe-detail-secondary">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.contact_body))}
                    </p>
                </div>

                <div style="text-align: center; margin-top: 1rem;">
                    <a href="/data-privacy" class="btn btn-outline" style="margin-right: 0.5rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.manage_data))}</a>
                    <a href="/" class="btn btn-outline">{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.back_home))}</a>
                </div>
            </div>
        </div>
    }
}
