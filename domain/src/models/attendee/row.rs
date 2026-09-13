//! Raw Google Sheets row wrapper, read through a `ColumnMapping`.

use super::columns::{ColumnKey, ColumnMapping};
use super::core::Attendee;
use super::status::CheckInStatus;

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
    pub registration_date: Option<String>,
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
    pub nft_proof_url: Option<String>,
    // Section 6: Bank & Refund (Y–AD)
    pub bank_account: Option<String>,
    pub bank_name: Option<String>,
    pub account_name: Option<String>,
    pub refund_status: Option<String>,
    pub refund_link: Option<String>,
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
        let registration_date = get_opt(idx(ColumnKey::RegistrationDate));
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
        let nft_proof_url = get_opt(idx(ColumnKey::NftProofUrl));
        let checked_in_at = get_opt(idx(ColumnKey::CheckedInAt));
        let checked_in_by = get_opt(idx(ColumnKey::CheckedInBy));

        // Section 6: Bank & Refund
        let bank_account = get_opt(idx(ColumnKey::BankAccount));
        let bank_name = get_opt(idx(ColumnKey::BankName));
        let account_name = get_opt(idx(ColumnKey::AccountName));
        let refund_status = get_opt(idx(ColumnKey::RefundStatus));
        let refund_link = get_opt(idx(ColumnKey::RefundLink));
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
            registration_date,
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
            nft_proof_url,
            bank_account,
            bank_name,
            account_name,
            refund_status,
            refund_link,
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
            registration_date: self.registration_date.clone(),
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
            nft_proof_url: self.nft_proof_url.clone(),
            bank_account: self.bank_account.clone(),
            bank_name: self.bank_name.clone(),
            account_name: self.account_name.clone(),
            refund_status: self.refund_status.clone(),
            refund_link: self.refund_link.clone(),
            send_email_status: self.send_email_status.clone(),
            row_index: self.row_index,
        }
    }
}
