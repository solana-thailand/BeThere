use leptos::prelude::*;
use leptos_meta::Title;

use crate::i18n::{t, t_string, use_i18n};
use crate::icons::{Icon, IconName};

/// Privacy Policy page — PDPA compliance.
///
/// The notice is bilingual (`locales/{en,th}/privacy.json`). The TH text is a
/// translation of the EN one, clause for clause; change both together.
#[component]
pub fn Privacy() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <Title text=move || t_string!(i18n, privacy.page_title) />
        <div class="center-page">
            <div class="container" style="max-width: 720px;">
                <div class="pe-card">
                    <h1 class="pe-section-title" style="margin-bottom: 0.5rem;">
                        <Icon icon=IconName::Lock class="icon-md" />" "{t!(i18n, privacy.title)}
                    </h1>
                    <p class="pe-detail-secondary" style="margin-bottom: 1.5rem;">
                        {t!(i18n, privacy.last_updated)}
                    </p>

                    // Data Controller
                    <h2 class="pe-section-title" style="font-size: 1.1rem;">{t!(i18n, privacy.controller_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.controller_body)}
                    </p>

                    // Data Collected
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{t!(i18n, privacy.collect_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.collect_intro)}
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>{t!(i18n, privacy.collect.name)}</li>
                        <li>{t!(i18n, privacy.collect.contact)}</li>
                        <li>{t!(i18n, privacy.collect.participation)}</li>
                        <li>{t!(i18n, privacy.collect.deposit)}</li>
                        <li>{t!(i18n, privacy.collect.wallet)}</li>
                        <li>{t!(i18n, privacy.collect.photo)}</li>
                        <li>{t!(i18n, privacy.collect.marketing)}</li>
                    </ul>

                    // Purpose
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{t!(i18n, privacy.purpose_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.purpose_intro)}
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>{t!(i18n, privacy.purpose.registration)}</li>
                        <li>{t!(i18n, privacy.purpose.checkin)}</li>
                        <li>{t!(i18n, privacy.purpose.nft)}</li>
                        <li>{t!(i18n, privacy.purpose.deposit)}</li>
                        <li>{t!(i18n, privacy.purpose.followup)}</li>
                        <li>{t!(i18n, privacy.purpose.emails)}</li>
                    </ul>

                    // Legal Basis
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{t!(i18n, privacy.basis_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.basis_body)}
                    </p>

                    // Blockchain Data
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{t!(i18n, privacy.chain_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.chain_intro)}
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>{t!(i18n, privacy.chain.public)}</li>
                        <li>{t!(i18n, privacy.chain.immutable)}</li>
                        <li>{t!(i18n, privacy.chain.not_controlled)}</li>
                    </ul>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.chain_note)}
                    </p>

                    // Photo/Media
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{t!(i18n, privacy.photo_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.photo_body)}
                    </p>

                    // Data Sharing
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{t!(i18n, privacy.sharing_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.sharing_intro)}
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>{t!(i18n, privacy.sharing.organizers)}</li>
                        <li>{t!(i18n, privacy.sharing.cloudflare)}</li>
                        <li>{t!(i18n, privacy.sharing.google)}</li>
                        <li>{t!(i18n, privacy.sharing.helius)}</li>
                        <li>{t!(i18n, privacy.sharing.crossmint)}</li>
                        <li>{t!(i18n, privacy.sharing.social)}</li>
                        <li>{t!(i18n, privacy.sharing.slack)}</li>
                    </ul>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.sharing_note)}
                    </p>

                    // Data Retention
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{t!(i18n, privacy.retention_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.retention_body)}
                    </p>

                    // Your Rights
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{t!(i18n, privacy.rights_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.rights_intro)}
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>{t!(i18n, privacy.rights.access)}</li>
                        <li>{t!(i18n, privacy.rights.correction)}</li>
                        <li>{t!(i18n, privacy.rights.deletion)}</li>
                        <li>{t!(i18n, privacy.rights.withdraw)}</li>
                    </ul>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.rights_how)}
                    </p>

                    // Cookies
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{t!(i18n, privacy.cookies_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.cookies_body)}
                    </p>

                    // Contact
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">{t!(i18n, privacy.contact_title)}</h2>
                    <p class="pe-detail-secondary">
                        {t!(i18n, privacy.contact_body)}
                    </p>
                </div>

                <div style="text-align: center; margin-top: 1rem;">
                    <a href="/data-privacy" class="btn btn-outline" style="margin-right: 0.5rem;">{t!(i18n, privacy.manage_data)}</a>
                    <a href="/" class="btn btn-outline">{t!(i18n, privacy.back_home)}</a>
                </div>
            </div>
        </div>
    }
}
