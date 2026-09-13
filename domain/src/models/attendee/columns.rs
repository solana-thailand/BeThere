//! Google Sheet column keys and the dynamic header→index mapping.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Well-known column keys used throughout the application.
/// Each key maps to one or more possible header names in the Google Sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKey {
    // Section 1: Attendee Identity (A–E)
    ApiId,
    Name,
    FirstName,
    LastName,
    Email,
    // Section 2: Registration Metadata (F–I)
    TicketName,
    RegistrationDate,
    ApprovalStatus,
    ParticipationType,
    // Section 3: Contact & Comms (J–L)
    Phone,
    ContactChannel,
    ContactHandle,
    // Section 4: Deposit & Payment (M–Q)
    DepositAgreed,
    DepositMethod,
    DepositAmount,
    DepositTxSignature,
    DepositVerified,
    // Section 5: Check-in & NFT Lifecycle (R–W)
    CheckedInAt,
    CheckedInBy,
    SolanaAddress,
    QrCodeUrl,
    ClaimToken,
    ClaimedAt,
    NftProofUrl,
    // Section 6: Bank & Refund (Y–AD)
    BankAccount,
    BankName,
    AccountName,
    RefundStatus,
    RefundLink,
    SendEmailStatus,
    // Section 7: Consent & Compliance (AE–AG)
    ConsentGiven,
    PhotoConsent,
    ConsentMarketing,
}

impl ColumnKey {
    /// All known column keys.
    pub fn all() -> &'static [ColumnKey] {
        &[
            // Section 1: Attendee Identity (A–E)
            ColumnKey::ApiId,
            ColumnKey::Name,
            ColumnKey::FirstName,
            ColumnKey::LastName,
            ColumnKey::Email,
            // Section 2: Registration Metadata (F–I)
            ColumnKey::TicketName,
            ColumnKey::RegistrationDate,
            ColumnKey::ApprovalStatus,
            ColumnKey::ParticipationType,
            // Section 3: Contact & Comms (J–L)
            ColumnKey::Phone,
            ColumnKey::ContactChannel,
            ColumnKey::ContactHandle,
            // Section 4: Deposit & Payment (M–Q)
            ColumnKey::DepositAgreed,
            ColumnKey::DepositMethod,
            ColumnKey::DepositAmount,
            ColumnKey::DepositTxSignature,
            ColumnKey::DepositVerified,
            // Section 5: Check-in & NFT Lifecycle (R–W)
            ColumnKey::CheckedInAt,
            ColumnKey::CheckedInBy,
            ColumnKey::SolanaAddress,
            ColumnKey::QrCodeUrl,
            ColumnKey::ClaimToken,
            ColumnKey::ClaimedAt,
            ColumnKey::NftProofUrl,
            // Section 6: Bank & Refund (Y–AD)
            ColumnKey::BankAccount,
            ColumnKey::BankName,
            ColumnKey::AccountName,
            ColumnKey::RefundStatus,
            ColumnKey::RefundLink,
            ColumnKey::SendEmailStatus,
            // Section 7: Consent & Compliance (AE–AG)
            ColumnKey::ConsentGiven,
            ColumnKey::PhotoConsent,
            ColumnKey::ConsentMarketing,
        ]
    }

    /// Header name candidates for this key (lowercase, checked case-insensitively).
    /// Order matters: earlier entries are preferred.
    pub fn header_candidates(&self) -> &'static [&'static str] {
        match self {
            // Section 1: Attendee Identity (A–E)
            ColumnKey::ApiId => &["api_id", "id"],
            ColumnKey::Name => &["name", "full_name"],
            ColumnKey::FirstName => &["first_name", "firstname", "given_name"],
            ColumnKey::LastName => &["last_name", "lastname", "family_name", "surname"],
            ColumnKey::Email => &["email", "e-mail"],
            // Section 2: Registration Metadata (F–I)
            ColumnKey::TicketName => &["ticket_name", "ticket", "ticket type", "ticket_type"],
            ColumnKey::RegistrationDate => &["registration_date", "registered_at", "created_at"],
            ColumnKey::ApprovalStatus => &["approval_status", "status"],
            ColumnKey::ParticipationType => &[
                "participation_type",
                "participant_type",
                "attendance_type",
                "attendance",
            ],
            // Section 3: Contact & Comms (J–L)
            ColumnKey::Phone => &["phone", "phone_number", "tel", "mobile"],
            ColumnKey::ContactChannel => {
                &["contact_channel", "preferred_contact", "contact_method"]
            }
            ColumnKey::ContactHandle => &[
                "contact_handle",
                "contact_username",
                "contact_link",
                "contact_url",
            ],
            // Section 4: Deposit & Payment (M–Q)
            ColumnKey::DepositAgreed => &["deposit_agreed", "deposit_accepted", "deposit_consent"],
            ColumnKey::DepositMethod => &["deposit_method", "payment_method"],
            ColumnKey::DepositAmount => &["deposit_amount", "payment_amount"],
            ColumnKey::DepositTxSignature => &[
                "deposit_tx_signature",
                "tx_signature",
                "transaction_id",
                "slip_reference",
            ],
            ColumnKey::DepositVerified => &["deposit_verified", "deposit_confirmed"],
            // Section 5: Check-in & NFT Lifecycle (R–W)
            ColumnKey::CheckedInAt => &["checked_in_at", "check_in_time", "checkin_time"],
            ColumnKey::CheckedInBy => &["checked_in_by", "check_in_by", "checkin_by"],
            ColumnKey::SolanaAddress => &["solana_address", "wallet", "wallet_address"],
            ColumnKey::QrCodeUrl => &["qr_code_url", "qr_code", "qr_url"],
            ColumnKey::ClaimToken => &["claim_token"],
            ColumnKey::ClaimedAt => &["claimed_at"],
            ColumnKey::NftProofUrl => &["nft_proof_url", "nft_link", "proof_url"],
            // Section 6: Bank & Refund (Y–AC)
            ColumnKey::BankAccount => &["bank_account", "bank_account_number"],
            ColumnKey::BankName => &["bank_name", "bank"],
            ColumnKey::AccountName => &["account_name", "account_holder"],
            ColumnKey::RefundStatus => &["refund_status", "refund_state"],
            ColumnKey::RefundLink => &["refund_link", "refund_url", "refund_proof_link"],
            ColumnKey::SendEmailStatus => &["send_email_status", "email_status", "email_sent"],
            // Section 7: Consent & Compliance (AE)
            ColumnKey::ConsentGiven => &["consent_given", "pdpa_consent", "data_consent"],
            ColumnKey::PhotoConsent => &["photo_consent", "photo_consent_given", "media_consent"],
            ColumnKey::ConsentMarketing => {
                &["consent_marketing", "marketing_consent", "marketing_opt_in"]
            }
        }
    }
}

/// Maps well-known column keys to 0-based indices in a Google Sheet.
///
/// Built by reading row 1 headers and matching them to known candidates.
/// Falls back to hardcoded indices for unrecognized/legacy sheets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMapping {
    /// {ColumnKey variant name (snake_case) → 0-based column index}
    map: HashMap<String, usize>,
    /// Total number of columns detected in the header row.
    pub total_columns: usize,
}

impl ColumnMapping {
    /// Hardcoded fallback mapping for the 32-column layout (A–AF).
    /// Used for sheets without recognizable headers or when header reading fails.
    ///
    /// Layout:
    ///   Section 1 — Identity (A–E):  api_id, name, first_name, last_name, email
    ///   Section 2 — Registration (F–I):  ticket_name, registration_date, approval_status, participation_type
    ///   Section 3 — Contact (J–L):  phone, contact_channel, contact_handle
    ///   Section 4 — Deposit (M–Q):  deposit_agreed, deposit_method, deposit_amount, deposit_tx_signature, deposit_verified
    ///   Section 5 — Lifecycle (R–X):  checked_in_at, checked_in_by, solana_address, qr_code_url, claim_token, claimed_at, nft_proof_url
    ///   Section 6 — Bank & Refund (Y–AD):  bank_account, bank_name, account_name, refund_status, refund_link, send_email_status
    ///   Section 7 — Consent (AE–AF):  consent_given, photo_consent
    pub fn hardcoded() -> Self {
        // Section 1: Attendee Identity (A–E)
        let mut map = HashMap::new();
        map.insert("api_id".into(), 0); // A
        map.insert("name".into(), 1); // B
        map.insert("first_name".into(), 2); // C
        map.insert("last_name".into(), 3); // D
        map.insert("email".into(), 4); // E
        // Section 2: Registration Metadata (F–I)
        map.insert("ticket_name".into(), 5); // F
        map.insert("registration_date".into(), 6); // G
        map.insert("approval_status".into(), 7); // H
        map.insert("participation_type".into(), 8); // I
        // Section 3: Contact & Comms (J–L)
        map.insert("phone".into(), 9); // J
        map.insert("contact_channel".into(), 10); // K
        map.insert("contact_handle".into(), 11); // L
        // Section 4: Deposit & Payment (M–Q)
        map.insert("deposit_agreed".into(), 12); // M
        map.insert("deposit_method".into(), 13); // N
        map.insert("deposit_amount".into(), 14); // O
        map.insert("deposit_tx_signature".into(), 15); // P
        map.insert("deposit_verified".into(), 16); // Q
        // Section 5: Check-in & NFT Lifecycle (R–W)
        map.insert("checked_in_at".into(), 17); // R
        map.insert("checked_in_by".into(), 18); // S
        map.insert("solana_address".into(), 19); // T
        map.insert("qr_code_url".into(), 20); // U
        map.insert("claim_token".into(), 21); // V
        map.insert("claimed_at".into(), 22); // W
        map.insert("nft_proof_url".into(), 23); // X
        // Section 6: Bank & Refund (Y–AC)
        map.insert("bank_account".into(), 24); // Y
        map.insert("bank_name".into(), 25); // Z
        map.insert("account_name".into(), 26); // AA
        map.insert("refund_status".into(), 27); // AB
        map.insert("refund_link".into(), 28); // AC
        map.insert("send_email_status".into(), 29); // AD
        // Section 7: Consent & Compliance (AE–AG)
        map.insert("consent_given".into(), 30); // AE
        map.insert("photo_consent".into(), 31); // AF
        map.insert("consent_marketing".into(), 32); // AG
        Self {
            map,
            total_columns: 33,
        }
    }

    /// Build a mapping from a header row (row 1 values).
    /// Each header is matched case-insensitively and with `_`/` `/`-` normalized.
    pub fn from_headers(headers: &[String]) -> Self {
        let mut map = HashMap::new();

        for key in ColumnKey::all() {
            let key_name = serde_json::to_value(key)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| format!("{:?}", key).to_lowercase());

            for candidate in key.header_candidates() {
                let normalized_candidate = normalize_header(candidate);
                for (idx, header) in headers.iter().enumerate() {
                    let normalized_header = normalize_header(header);
                    if normalized_header == normalized_candidate {
                        map.entry(key_name.clone()).or_insert(idx);
                        break;
                    }
                }
                if map.contains_key(&key_name) {
                    break;
                }
            }
        }

        Self {
            map,
            total_columns: headers.len(),
        }
    }

    /// Get the 0-based column index for a key. Returns `None` if unmapped.
    pub fn get(&self, key: ColumnKey) -> Option<usize> {
        let key_name = column_key_name(key);
        self.map.get(&key_name).copied()
    }

    /// Get the column index, falling back to the hardcoded default.
    pub fn get_or_default(&self, key: ColumnKey) -> usize {
        self.get(key)
            .unwrap_or_else(|| ColumnMapping::hardcoded().get(key).unwrap_or(0))
    }

    /// Get the column letter (A, B, ..., Z, AA, AB, ...) for a key.
    /// Used for Google Sheets API range references like `"{sheet_name}!I{row}"`.
    pub fn column_letter(&self, key: ColumnKey) -> String {
        let idx = self.get_or_default(key);
        index_to_column_letter(idx)
    }

    /// Number of recognized columns successfully mapped.
    pub fn mapped_count(&self) -> usize {
        self.map.len()
    }

    /// Whether this is likely a valid mapping (at least api_id and email).
    pub fn is_valid(&self) -> bool {
        self.get(ColumnKey::ApiId).is_some() && self.get(ColumnKey::Email).is_some()
    }

    /// Returns the spreadsheet column letter for the last mapped column.
    ///
    /// Uses `total_columns` (the header row width) so the Sheets API range
    /// covers only the columns we actually need, reducing response payload.
    pub fn last_column_letter(&self) -> String {
        if self.total_columns == 0 {
            return "Z".to_string();
        }
        index_to_column_letter(self.total_columns - 1)
    }
}

/// Normalize a header string for comparison: lowercase, trim, replace `-`/` ` with `_`.
fn normalize_header(s: &str) -> String {
    s.trim().to_lowercase().replace(['-', ' '], "_")
}

/// Convert a 0-based column index to a spreadsheet column letter (A-Z, AA-AZ, ...).
pub(super) fn index_to_column_letter(idx: usize) -> String {
    let mut result = String::new();
    let mut n = idx;
    loop {
        result.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = (n / 26) - 1;
    }
    result
}

/// Get the snake_case name of a ColumnKey for map lookups.
fn column_key_name(key: ColumnKey) -> String {
    serde_json::to_value(key)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| format!("{:?}", key).to_lowercase())
}
