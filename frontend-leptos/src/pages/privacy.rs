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
                        "Last updated: June 2026"
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
                        "Some events may photograph or record attendees. When this applies, a separate photo consent checkbox will appear during registration. You may decline photo consent without affecting your registration."
                    </p>

                    // Data Sharing
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"7. Data Sharing"</h2>
                    <p class="pe-detail-secondary">
                        "Your data is shared with:"
                    </p>
                    <ul class="pe-detail-secondary" style="padding-left: 1.5rem; list-style: disc;">
                        <li>"Event organizers (via Google Sheets) — for event management"</li>
                        <li>"Google (Sheets storage, OAuth authentication)"</li>
                        <li>"Solana RPC providers (Helius) — for blockchain transactions"</li>
                    </ul>
                    <p class="pe-detail-secondary">
                        "We do not sell your personal data to third parties."
                    </p>

                    // Data Retention
                    <h2 class="pe-section-title" style="font-size: 1.1rem; margin-top: 1.25rem;">"8. Data Retention"</h2>
                    <p class="pe-detail-secondary">
                        "Personal data in Google Sheets is retained until event conclusion plus 90 days for refund processing and dispute resolution. After this period, PII fields (name, email, phone, contact) are cleared. On-chain data cannot be deleted. Cloudflare logs auto-expire after 72 hours."
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
