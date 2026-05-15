use std::collections::HashMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckInStatus {
    PendingApproval,
    Approved,
    Invited,
    CheckedIn,
}

impl FromStr for CheckInStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.trim().to_lowercase().as_str() {
            "approved" => Self::Approved,
            "pending_approval" => Self::PendingApproval,
            "invited" => Self::Invited,
            "checked_in" | "checked in" => Self::CheckedIn,
            _ => Self::PendingApproval,
        })
    }
}

impl CheckInStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PendingApproval => "pending_approval",
            Self::Approved => "approved",
            Self::Invited => "invited",
            Self::CheckedIn => "checked_in",
        }
    }
}

impl std::fmt::Display for CheckInStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attendee {
    pub api_id: String,
    pub first_name: String,
    pub last_name: String,
    pub name: String,
    pub email: String,
    pub ticket_name: String,
    pub approval_status: CheckInStatus,
    pub participation_type: String,
    pub phone: Option<String>,
    pub contact_channel: Option<String>,
    pub contact_handle: Option<String>,
    pub deposit_agreed: Option<String>,
    pub deposit_method: Option<String>,
    pub deposit_amount: Option<String>,
    pub deposit_tx_signature: Option<String>,
    pub deposit_verified: Option<String>,
    pub checked_in_at: Option<String>,
    pub checked_in_by: Option<String>,
    pub solana_address: Option<String>,
    pub qr_code_url: Option<String>,
    pub claim_token: Option<String>,
    pub claimed_at: Option<String>,
    // Section 6: Bank & Refund (X–AB)
    pub bank_account: Option<String>,
    pub bank_name: Option<String>,
    pub account_name: Option<String>,
    pub refund_status: Option<String>,
    pub send_email_status: Option<String>,
    pub row_index: usize,
}

impl Attendee {
    pub fn is_approved(&self) -> bool {
        matches!(
            self.approval_status,
            CheckInStatus::Approved | CheckInStatus::CheckedIn
        )
    }

    pub fn is_checked_in(&self) -> bool {
        self.checked_in_at.is_some()
    }

    /// Check if attendee's participation type is "In-Person".
    /// Online attendees should not be checked in at the physical event.
    /// Uses substring matching since the sheet value may be longer
    /// (e.g. "In-Person (Physical Attendance)", "In Person", "IN-PERSON").
    ///
    /// Defaults to `true` when participation_type is empty — legacy events
    /// predate this field and were all in-person.
    pub fn is_in_person(&self) -> bool {
        let lower = self.participation_type.trim().to_lowercase();
        if lower.is_empty() {
            return true;
        }
        lower.contains("in-person") || lower.contains("in person")
    }

    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.email
        } else {
            &self.name
        }
    }
}

// ---------------------------------------------------------------------------
// Dynamic column mapping
// ---------------------------------------------------------------------------

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
    // Section 6: Bank & Refund (X–AB)
    BankAccount,
    BankName,
    AccountName,
    RefundStatus,
    SendEmailStatus,
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
            // Section 6: Bank & Refund (X–AB)
            ColumnKey::BankAccount,
            ColumnKey::BankName,
            ColumnKey::AccountName,
            ColumnKey::RefundStatus,
            ColumnKey::SendEmailStatus,
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
            // Section 6: Bank & Refund (X–AB)
            ColumnKey::BankAccount => &["bank_account", "bank_account_number"],
            ColumnKey::BankName => &["bank_name", "bank"],
            ColumnKey::AccountName => &["account_name", "account_holder"],
            ColumnKey::RefundStatus => &["refund_status", "refund_state"],
            ColumnKey::SendEmailStatus => &["send_email_status", "email_status", "email_sent"],
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
    /// Hardcoded fallback mapping for the 28-column layout (A–AB).
    /// Used for sheets without recognizable headers or when header reading fails.
    ///
    /// Layout:
    ///   Section 1 — Identity (A–E):  api_id, name, first_name, last_name, email
    ///   Section 2 — Registration (F–I):  ticket_name, registration_date, approval_status, participation_type
    ///   Section 3 — Contact (J–L):  phone, contact_channel, contact_handle
    ///   Section 4 — Deposit (M–Q):  deposit_agreed, deposit_method, deposit_amount, deposit_tx_signature, deposit_verified
    ///   Section 5 — Lifecycle (R–W):  checked_in_at, checked_in_by, solana_address, qr_code_url, claim_token, claimed_at
    ///   Section 6 — Bank & Refund (X–AB):  bank_account, bank_name, account_name, refund_status, send_email_status
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
        // Section 6: Bank & Refund (X–AB)
        map.insert("bank_account".into(), 23); // X
        map.insert("bank_name".into(), 24); // Y
        map.insert("account_name".into(), 25); // Z
        map.insert("refund_status".into(), 26); // AA
        map.insert("send_email_status".into(), 27); // AB
        Self {
            map,
            total_columns: 28,
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
}

/// Normalize a header string for comparison: lowercase, trim, replace `-`/` ` with `_`.
fn normalize_header(s: &str) -> String {
    s.trim().to_lowercase().replace(['-', ' '], "_")
}

/// Convert a 0-based column index to a spreadsheet column letter (A-Z, AA-AZ, ...).
fn index_to_column_letter(idx: usize) -> String {
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

/// Represents a raw row from Google Sheets.
///
/// Column mapping is dynamic via `ColumnMapping`. The hardcoded fallback
/// is the 28-column layout (A–AB) — see `ColumnMapping::hardcoded()`.
#[derive(Debug, Clone)]
pub struct AttendeeRow {
    pub api_id: String,
    pub first_name: String,
    pub last_name: String,
    pub name: String,
    pub email: String,
    pub ticket_name: String,
    pub approval_status: String,
    pub participation_type: String,
    pub phone: Option<String>,
    pub contact_channel: Option<String>,
    pub contact_handle: Option<String>,
    pub deposit_agreed: Option<String>,
    pub deposit_method: Option<String>,
    pub deposit_amount: Option<String>,
    pub deposit_tx_signature: Option<String>,
    pub deposit_verified: Option<String>,
    pub checked_in_at: Option<String>,
    pub checked_in_by: Option<String>,
    pub solana_address: Option<String>,
    pub qr_code_url: Option<String>,
    pub claim_token: Option<String>,
    pub claimed_at: Option<String>,
    // Section 6: Bank & Refund (X–AB)
    pub bank_account: Option<String>,
    pub bank_name: Option<String>,
    pub account_name: Option<String>,
    pub refund_status: Option<String>,
    pub send_email_status: Option<String>,
    pub row_index: usize,
}

impl AttendeeRow {
    /// Parse a row from Google Sheets values array.
    /// `values` is the full sheet data (including header row).
    /// `row_index` is the 1-based row number in the sheet (header is row 1).
    /// `mapping` provides dynamic column resolution; falls back to hardcoded indices.
    pub fn from_sheet_values(
        values: &[Vec<String>],
        row_index: usize,
        mapping: &ColumnMapping,
    ) -> Option<Self> {
        let row = values.get(row_index - 2)?; // Skip header row (row 1)

        if row.is_empty() {
            return None;
        }

        let get =
            |idx: usize| -> String { row.get(idx).cloned().unwrap_or_default().trim().to_string() };

        let get_opt = |idx: usize| -> Option<String> {
            let val = get(idx);
            if val.is_empty() { None } else { Some(val) }
        };

        let idx = |key: ColumnKey| -> usize { mapping.get_or_default(key) };

        let api_id = get(idx(ColumnKey::ApiId));
        if api_id.is_empty() {
            return None;
        }

        let participation_type = get(idx(ColumnKey::ParticipationType));
        let phone = get_opt(idx(ColumnKey::Phone));
        let contact_channel = get_opt(idx(ColumnKey::ContactChannel));
        let contact_handle = get_opt(idx(ColumnKey::ContactHandle));
        let deposit_agreed = get_opt(idx(ColumnKey::DepositAgreed));
        let deposit_method = get_opt(idx(ColumnKey::DepositMethod));
        let deposit_amount = get_opt(idx(ColumnKey::DepositAmount));
        let deposit_tx_signature = get_opt(idx(ColumnKey::DepositTxSignature));
        let deposit_verified = get_opt(idx(ColumnKey::DepositVerified));
        let solana_address = get_opt(idx(ColumnKey::SolanaAddress));
        let qr_code_url = get_opt(idx(ColumnKey::QrCodeUrl));
        let claim_token = get_opt(idx(ColumnKey::ClaimToken));
        let claimed_at = get_opt(idx(ColumnKey::ClaimedAt));
        let checked_in_at = get_opt(idx(ColumnKey::CheckedInAt));
        let checked_in_by = get_opt(idx(ColumnKey::CheckedInBy));

        // Section 6: Bank & Refund
        let bank_account = get_opt(idx(ColumnKey::BankAccount));
        let bank_name = get_opt(idx(ColumnKey::BankName));
        let account_name = get_opt(idx(ColumnKey::AccountName));
        let refund_status = get_opt(idx(ColumnKey::RefundStatus));
        let send_email_status = get_opt(idx(ColumnKey::SendEmailStatus));

        let first_name_col = idx(ColumnKey::FirstName);
        let name_col = idx(ColumnKey::Name);

        Some(Self {
            api_id,
            first_name: get(first_name_col),
            last_name: get(idx(ColumnKey::LastName)),
            name: {
                let col_name = get(name_col);
                if !col_name.is_empty() {
                    col_name
                } else {
                    get(first_name_col)
                }
            },
            email: get(idx(ColumnKey::Email)),
            ticket_name: get(idx(ColumnKey::TicketName)),
            approval_status: get(idx(ColumnKey::ApprovalStatus)),
            participation_type,
            phone,
            contact_channel,
            contact_handle,
            deposit_agreed,
            deposit_method,
            deposit_amount,
            deposit_tx_signature,
            deposit_verified,
            checked_in_at,
            checked_in_by,
            solana_address,
            qr_code_url,
            claim_token,
            claimed_at,
            bank_account,
            bank_name,
            account_name,
            refund_status,
            send_email_status,
            row_index,
        })
    }

    /// Convert raw row into a typed Attendee
    pub fn to_attendee(&self) -> Attendee {
        let mut status = self.approval_status.parse::<CheckInStatus>().unwrap();
        if self.checked_in_at.is_some() && status == CheckInStatus::Approved {
            status = CheckInStatus::CheckedIn;
        }

        Attendee {
            api_id: self.api_id.clone(),
            first_name: self.first_name.clone(),
            last_name: self.last_name.clone(),
            name: self.name.clone(),
            email: self.email.clone(),
            ticket_name: self.ticket_name.clone(),
            approval_status: status,
            participation_type: self.participation_type.clone(),
            phone: self.phone.clone(),
            contact_channel: self.contact_channel.clone(),
            contact_handle: self.contact_handle.clone(),
            deposit_agreed: self.deposit_agreed.clone(),
            deposit_method: self.deposit_method.clone(),
            deposit_amount: self.deposit_amount.clone(),
            deposit_tx_signature: self.deposit_tx_signature.clone(),
            deposit_verified: self.deposit_verified.clone(),
            checked_in_at: self.checked_in_at.clone(),
            checked_in_by: self.checked_in_by.clone(),
            solana_address: self.solana_address.clone(),
            qr_code_url: self.qr_code_url.clone(),
            claim_token: self.claim_token.clone(),
            claimed_at: self.claimed_at.clone(),
            bank_account: self.bank_account.clone(),
            bank_name: self.bank_name.clone(),
            account_name: self.account_name.clone(),
            refund_status: self.refund_status.clone(),
            send_email_status: self.send_email_status.clone(),
            row_index: self.row_index,
        }
    }
}

// ---------------------------------------------------------------------------
// Walk-in attendee types
// ---------------------------------------------------------------------------

/// Walk-in attendee registered on-the-spot by staff.
///
/// Stored in KV under `walkin:{event_id}:{email_lower}` with a 90-day TTL.
/// A reverse mapping `claim_walkin:{claim_token}` → `{event_id}:{email_lower}`
/// enables claim-token lookup without scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkinAttendee {
    pub event_id: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub claim_token: String,
    /// ISO 8601 timestamp when the walk-in was registered.
    pub checked_in_at: String,
    /// Email of the staff member who registered the walk-in.
    pub checked_in_by: String,
    /// Solana wallet address — set later when the attendee claims their NFT.
    pub wallet_address: Option<String>,
    /// ISO 8601 timestamp when the NFT was claimed.
    pub claimed_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_attendee(participation_type: &str) -> Attendee {
        Attendee {
            api_id: "gst-test".to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            ticket_name: "General".to_string(),
            approval_status: CheckInStatus::Approved,
            participation_type: participation_type.to_string(),
            phone: None,
            contact_channel: None,
            contact_handle: None,
            deposit_agreed: None,
            deposit_method: None,
            deposit_amount: None,
            deposit_tx_signature: None,
            deposit_verified: None,
            checked_in_at: None,
            checked_in_by: None,
            solana_address: None,
            qr_code_url: None,
            claim_token: None,
            claimed_at: None,
            bank_account: None,
            bank_name: None,
            account_name: None,
            refund_status: None,
            send_email_status: None,
            row_index: 2,
        }
    }

    #[test]
    fn test_is_in_person_exact() {
        assert!(make_attendee("In-Person").is_in_person());
    }

    #[test]
    fn test_is_in_person_case_insensitive() {
        assert!(make_attendee("in-person").is_in_person());
        assert!(make_attendee("IN-PERSON").is_in_person());
        assert!(make_attendee("In-person").is_in_person());
    }

    #[test]
    fn test_is_in_person_with_spaces() {
        assert!(make_attendee("In Person").is_in_person());
        assert!(make_attendee("in person").is_in_person());
        assert!(make_attendee("IN PERSON").is_in_person());
    }

    #[test]
    fn test_is_in_person_long_value() {
        assert!(make_attendee("In-Person (Physical Attendance)").is_in_person());
        assert!(make_attendee("In-Person - On Site").is_in_person());
        assert!(make_attendee("  In-Person  ").is_in_person());
        assert!(make_attendee("In Person Participant").is_in_person());
    }

    #[test]
    fn test_is_not_in_person_online() {
        assert!(!make_attendee("Online").is_in_person());
        assert!(!make_attendee("online").is_in_person());
        assert!(!make_attendee("ONLINE").is_in_person());
    }

    #[test]
    fn test_is_not_in_person_virtual() {
        assert!(!make_attendee("Virtual").is_in_person());
        assert!(!make_attendee("Hybrid").is_in_person());
    }

    #[test]
    fn test_is_in_person_empty_defaults_true() {
        // Empty participation_type defaults to in-person (legacy events)
        assert!(make_attendee("").is_in_person());
        assert!(make_attendee("   ").is_in_person());
    }

    #[test]
    fn test_is_not_in_person_other() {
        assert!(!make_attendee("Unknown").is_in_person());
        assert!(!make_attendee("TBD").is_in_person());
    }

    // -----------------------------------------------------------------------
    // ColumnMapping tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_column_mapping_hardcoded() {
        let mapping = ColumnMapping::hardcoded();
        // Section 1: Identity
        assert_eq!(mapping.get(ColumnKey::ApiId), Some(0)); // A
        assert_eq!(mapping.get(ColumnKey::Email), Some(4)); // E
        // Section 2: Registration
        assert_eq!(mapping.get(ColumnKey::ParticipationType), Some(8)); // I
        // Section 3: Contact
        assert_eq!(mapping.get(ColumnKey::Phone), Some(9)); // J
        assert_eq!(mapping.get(ColumnKey::ContactChannel), Some(10)); // K
        assert_eq!(mapping.get(ColumnKey::ContactHandle), Some(11)); // L
        // Section 4: Deposit
        assert_eq!(mapping.get(ColumnKey::DepositAgreed), Some(12)); // M
        assert_eq!(mapping.get(ColumnKey::DepositMethod), Some(13)); // N
        assert_eq!(mapping.get(ColumnKey::DepositTxSignature), Some(15)); // P
        assert_eq!(mapping.get(ColumnKey::DepositVerified), Some(16)); // Q
        // Section 5: Lifecycle
        assert_eq!(mapping.get(ColumnKey::CheckedInAt), Some(17)); // R
        assert_eq!(mapping.get(ColumnKey::SolanaAddress), Some(19)); // T
        assert_eq!(mapping.get(ColumnKey::ClaimToken), Some(21)); // V
        // Section 6: Bank & Refund
        assert_eq!(mapping.get(ColumnKey::BankAccount), Some(23)); // X
        assert_eq!(mapping.get(ColumnKey::BankName), Some(24)); // Y
        assert_eq!(mapping.get(ColumnKey::AccountName), Some(25)); // Z
        assert_eq!(mapping.get(ColumnKey::RefundStatus), Some(26)); // AA
        assert_eq!(mapping.get(ColumnKey::SendEmailStatus), Some(27)); // AB
        assert_eq!(mapping.total_columns, 28);
    }

    #[test]
    fn test_column_mapping_from_headers_exact() {
        let headers: Vec<String> = vec![
            "api_id".into(),
            "name".into(),
            "first_name".into(),
            "last_name".into(),
            "email".into(),
            "ticket_name".into(),
            "registration_date".into(),
            "approval_status".into(),
            "participation_type".into(),
            "phone".into(),
            "contact_channel".into(),
            "contact_handle".into(),
            "deposit_agreed".into(),
            "deposit_method".into(),
            "deposit_amount".into(),
            "deposit_tx_signature".into(),
            "deposit_verified".into(),
            "checked_in_at".into(),
            "checked_in_by".into(),
            "solana_address".into(),
            "qr_code_url".into(),
            "claim_token".into(),
            "claimed_at".into(),
        ];
        let mapping = ColumnMapping::from_headers(&headers);
        assert!(mapping.is_valid());
        assert_eq!(mapping.get(ColumnKey::ApiId), Some(0));
        assert_eq!(mapping.get(ColumnKey::Email), Some(4));
        assert_eq!(mapping.get(ColumnKey::ParticipationType), Some(8));
        assert_eq!(mapping.get(ColumnKey::Phone), Some(9));
        assert_eq!(mapping.get(ColumnKey::CheckedInAt), Some(17));
        assert_eq!(mapping.mapped_count(), 23);
    }

    #[test]
    fn test_column_mapping_from_headers_case_insensitive() {
        let headers: Vec<String> = vec![
            "API_ID".into(),
            "Name".into(),
            "FIRSTNAME".into(),
            "Last Name".into(),
            "Email".into(),
            "Status".into(),
            "Checked In At".into(),
        ];
        let mapping = ColumnMapping::from_headers(&headers);
        assert_eq!(mapping.get(ColumnKey::ApiId), Some(0));
        assert_eq!(mapping.get(ColumnKey::FirstName), Some(2));
        assert_eq!(mapping.get(ColumnKey::LastName), Some(3));
        assert_eq!(mapping.get(ColumnKey::CheckedInAt), Some(6));
        assert_eq!(mapping.get(ColumnKey::ApprovalStatus), Some(5));
    }

    #[test]
    fn test_column_mapping_partial_headers() {
        // Only some columns have headers we recognize
        let headers: Vec<String> = vec![
            "api_id".into(),
            "something".into(),
            "email".into(),
            "another".into(),
            "checked_in_at".into(),
        ];
        let mapping = ColumnMapping::from_headers(&headers);
        assert!(mapping.is_valid());
        assert_eq!(mapping.get(ColumnKey::ApiId), Some(0));
        assert_eq!(mapping.get(ColumnKey::Email), Some(2));
        assert_eq!(mapping.get(ColumnKey::CheckedInAt), Some(4));
        // Unmapped columns return None
        assert_eq!(mapping.get(ColumnKey::Name), None);
        assert_eq!(mapping.get(ColumnKey::ParticipationType), None);
    }

    #[test]
    fn test_column_mapping_get_or_default_fallback() {
        // Mapping with only 2 columns mapped
        let headers: Vec<String> = vec!["api_id".into(), "email".into()];
        let mapping = ColumnMapping::from_headers(&headers);
        // Unmapped keys fall back to hardcoded
        assert_eq!(mapping.get_or_default(ColumnKey::ApiId), 0);
        assert_eq!(mapping.get_or_default(ColumnKey::Email), 1);
        assert_eq!(mapping.get_or_default(ColumnKey::CheckedInAt), 17); // hardcoded fallback
        assert_eq!(mapping.get_or_default(ColumnKey::ParticipationType), 8); // hardcoded fallback
    }

    #[test]
    fn test_index_to_column_letter() {
        assert_eq!(index_to_column_letter(0), "A");
        assert_eq!(index_to_column_letter(1), "B");
        assert_eq!(index_to_column_letter(8), "I");
        assert_eq!(index_to_column_letter(24), "Y");
        assert_eq!(index_to_column_letter(25), "Z");
        assert_eq!(index_to_column_letter(26), "AA");
        assert_eq!(index_to_column_letter(27), "AB");
    }

    #[test]
    fn test_column_letter_via_mapping() {
        let mapping = ColumnMapping::hardcoded();
        assert_eq!(mapping.column_letter(ColumnKey::ApiId), "A");
        assert_eq!(mapping.column_letter(ColumnKey::ParticipationType), "I");
        assert_eq!(mapping.column_letter(ColumnKey::CheckedInAt), "R");
        assert_eq!(mapping.column_letter(ColumnKey::SolanaAddress), "T");
        assert_eq!(mapping.column_letter(ColumnKey::ClaimToken), "V");
    }

    #[test]
    fn test_from_sheet_values_with_mapping() {
        let headers: Vec<String> = vec![
            "api_id".into(),               // 0
            "name".into(),                 // 1
            "first_name".into(),           // 2
            "last_name".into(),            // 3
            "email".into(),                // 4
            "ticket_name".into(),          // 5
            "registration_date".into(),    // 6
            "approval_status".into(),      // 7
            "participation_type".into(),   // 8
            "phone".into(),                // 9
            "contact_channel".into(),      // 10
            "contact_handle".into(),       // 11
            "deposit_agreed".into(),       // 12
            "deposit_method".into(),       // 13
            "deposit_amount".into(),       // 14
            "deposit_tx_signature".into(), // 15
            "deposit_verified".into(),     // 16
            "checked_in_at".into(),        // 17
            "checked_in_by".into(),        // 18
            "solana_address".into(),       // 19
            "qr_code_url".into(),          // 20
            "claim_token".into(),          // 21
            "claimed_at".into(),           // 22
        ];
        let mapping = ColumnMapping::from_headers(&headers);

        let data_rows: Vec<Vec<String>> = vec![vec![
            "gst-123".into(),
            "John Doe".into(),
            "John".into(),
            "Doe".into(),
            "john@test.com".into(),
            "VIP".into(),
            "2025-01-01".into(),
            "Approved".into(),
            "In-Person".into(),
            "".into(),
            "Telegram".into(),
            "@johndoe".into(),
            "Yes".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            "tok-abc".into(),
            "".into(),
        ]];

        let row = AttendeeRow::from_sheet_values(&data_rows, 2, &mapping).unwrap();
        assert_eq!(row.api_id, "gst-123");
        assert_eq!(row.name, "John Doe");
        assert_eq!(row.first_name, "John");
        assert_eq!(row.last_name, "Doe");
        assert_eq!(row.email, "john@test.com");
        assert_eq!(row.ticket_name, "VIP");
        assert_eq!(row.approval_status, "Approved");
        assert_eq!(row.participation_type, "In-Person");
        assert_eq!(row.contact_channel, Some("Telegram".into()));
        assert_eq!(row.contact_handle, Some("@johndoe".into()));
        assert_eq!(row.deposit_agreed, Some("Yes".into()));
        assert_eq!(row.claim_token, Some("tok-abc".into()));
        assert_eq!(row.checked_in_at, None);
    }

    #[test]
    fn test_from_sheet_values_hardcoded_compat() {
        // 28-column layout using hardcoded mapping
        let mapping = ColumnMapping::hardcoded();

        let mut row_data = vec![String::new(); 28];
        row_data[0] = "gst-legacy".into(); // A: api_id
        row_data[1] = "Jane Smith".into(); // B: name
        row_data[2] = "Jane".into(); // C: first_name
        row_data[3] = "Smith".into(); // D: last_name
        row_data[4] = "jane@legacy.com".into(); // E: email
        row_data[5] = "General".into(); // F: ticket_name
        row_data[7] = "Approved".into(); // H: approval_status
        row_data[8] = "In-Person".into(); // I: participation_type
        row_data[17] = "2025-01-01T00:00:00Z".into(); // R: checked_in_at
        row_data[19] = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA85T".into(); // T: solana_address
        row_data[21] = "tok-legacy".into(); // V: claim_token

        let data_rows: Vec<Vec<String>> = vec![row_data];

        let row = AttendeeRow::from_sheet_values(&data_rows, 2, &mapping).unwrap();
        assert_eq!(row.api_id, "gst-legacy");
        assert_eq!(row.name, "Jane Smith");
        assert_eq!(row.email, "jane@legacy.com");
        assert_eq!(row.participation_type, "In-Person");
        assert_eq!(row.checked_in_at, Some("2025-01-01T00:00:00Z".into()));
        assert_eq!(
            row.solana_address,
            Some("7xKXtg2CW87d97TXJSDpbD5jBkheTqA85T".into())
        );
        assert_eq!(row.claim_token, Some("tok-legacy".into()));
    }
}
