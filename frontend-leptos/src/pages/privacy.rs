use leptos::prelude::*;
use leptos_meta::Title;

use crate::icons::{Icon, IconName};

/// Privacy Policy page — PDPA compliance.
#[component]
pub fn Privacy() -> impl IntoView {
    view! {
        <Title text="Privacy Policy — BeThere" />
        <div class="center-page">
            <div class="container" style="max-width: 720px;">
                <div class="pe-card">
                    <h1 class="pe-section-title" style="margin-bottom: 0.5rem;">
                        <Icon icon=IconName::Lock class="icon-md" />" Privacy Policy"
                    </h1>
                    <p class="pe-detail-secondary" style="margin-bottom: 1.5rem;">
                        "Last updated: September 2026"
                    </p>

                    // Data Controller
                    <h2 class="pe-section-title" style="font-size: 1.1rem;">"1. Data Controller"</h2>
                    <p class="pe-detail-secondary">
                        "BeThere is operated by Solana Thailand. For data privacy inquiries, contact us via Telegram or email listed on our event pages."
                    </p>

                    // Data Collected
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"2. Data We Collect"</h2>
                    <p class="pe-detail-secondary">
                        "When you register for an event, we may collect:"
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>"Name and email address (via Google Sign-In)"</li>
                        <li>"Contact channel and handle (Telegram, Line, Facebook, or X) if required by the event"</li>
                        <li>"Participation type (In-Person or Online)"</li>
                        <li>"Deposit payment information (transaction signature on Solana or PromptPay slip)"</li>
                        <li>"Wallet address for NFT issuance and refunds"</li>
                        <li>"Photo/video consent status (when the event collects this)"</li>
                    </ul>

                    // Purpose
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"3. Purpose of Collection"</h2>
                    <p class="pe-detail-secondary">
                        "Your personal data is collected solely for:"
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>"Event registration and capacity management"</li>
                        <li>"Check-in verification at the venue"</li>
                        <li>"NFT badge issuance (commemorative proof of attendance)"</li>
                        <li>"Deposit commitment and refund processing"</li>
                        <li>"Staff follow-up for event logistics"</li>
                    </ul>

                    // Legal Basis
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"4. Legal Basis"</h2>
                    <p class="pe-detail-secondary">
                        "We collect personal data based on your explicit consent (Thailand PDPA Section 19) and for contract performance (event registration and service delivery)."
                    </p>

                    // Blockchain Data
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"5. Blockchain Data"</h2>
                    <p class="pe-detail-secondary">
                        "When you connect a Solana wallet for deposit, refund, or NFT claim, your wallet address and transaction signatures are recorded on the Solana blockchain. This data is:"
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>"Public and visible to anyone"</li>
                        <li>"Immutable and cannot be deleted"</li>
                        <li>"Not controlled by BeThere"</li>
                    </ul>
                    <p class="pe-detail-secondary">
                        "This is a technical characteristic of blockchain technology and is disclosed under PDPA Section 37 (technical impossibility exemption)."
                    </p>

                    // Photo/Media
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"6. Photo & Media Consent"</h2>
                    <p class="pe-detail-secondary">
                        "Some events may photograph or record attendees. The consent checkbox you tick when registering includes consent to this, and an event can make it a condition of registering. If you do not want to appear in photos, tell the event staff at check-in."
                    </p>

                    // Data Sharing
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"7. Data Sharing"</h2>
                    <p class="pe-detail-secondary">
                        "Your data is shared with:"
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>"Event organizers and their staff (the event's Google Sheet and the admin pages) — for event management"</li>
                        <li>"Cloudflare — hosting, the database and file storage, and page-view analytics"</li>
                        <li>"Google — sign-in (OAuth) and Google Sheets storage"</li>
                        <li>"Helius (Solana RPC) — wallet addresses and transactions"</li>
                        <li>"Crossmint — your wallet address, to mint your NFT badge (no name or email)"</li>
                        <li>"GitHub or Telegram — only if you link that account"</li>
                        <li>"Slack — our internal error alerts"</li>
                    </ul>
                    <p class="pe-detail-secondary">
                        "These providers are based outside Thailand, so your data may be processed abroad. Your browser also loads fonts from Google Fonts and a Solana library from public CDNs (unpkg, jsDelivr). We do not sell your personal data to third parties."
                    </p>

                    // Data Retention
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"8. Data Retention"</h2>
                    <p class="pe-detail-secondary">
                        "Deposit records are deleted 90 days after the event's refund deadline; the deposit amounts are kept for accounting. Quiz and adventure progress is deleted 30 days after the event. Other personal data, such as your registration, contact details, bank details for refunds, the event's Google Sheet and uploaded slip images, has no fixed deletion date yet: it is kept until you ask us to delete it (see Your Rights). On-chain data cannot be deleted."
                    </p>

                    // Your Rights
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"9. Your Rights"</h2>
                    <p class="pe-detail-secondary">
                        "Under Thailand's PDPA, you have the right to:"
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>"Access your personal data held by us"</li>
                        <li>"Request correction of inaccurate data"</li>
                        <li>"Request deletion of your data (subject to technical limitations)"</li>
                        <li>"Withdraw consent at any time"</li>
                    </ul>
                    <p class="pe-detail-secondary">
                        "To exercise these rights, contact us via the email or Telegram listed on the event page."
                    </p>

                    // Cookies
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"10. Cookies"</h2>
                    <p class="pe-detail-secondary">
                        "BeThere uses a session cookie (JWT) for authentication after Google Sign-In. No tracking cookies are used."
                    </p>

                    // Contact
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"11. Contact"</h2>
                    <p class="pe-detail-secondary">
                        "For privacy-related questions or data requests, reach out through the contact information on the event page or via Solana Thailand community channels."
                    </p>
                </div>

                <div style="text-align: center; margin-top: 1rem;">
                    <a href="/data-privacy" class="btn btn-outline" style="margin-right: 0.5rem;">"Manage My Data"</a>
                    <a href="/" class="btn btn-outline">"← Back to Home"</a>
                </div>
            </div>
        </div>
    }
}
