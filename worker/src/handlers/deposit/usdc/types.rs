//! Request/response types and the verification outcome enum shared by the
//! USDC deposit handlers and helpers.

/// Query parameters for the Solana Pay TX callback.
#[derive(Debug, serde::Deserialize)]
pub struct DepositTxQuery {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet: String,
}

/// Solana Pay Transaction Request response.
///
/// Returned when a wallet calls the callback URL from a Solana Pay QR code.
/// Contains a base64-encoded serialized transaction for the wallet to sign and submit.
#[derive(Debug, serde::Serialize)]
pub struct DepositTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet adds signature).
    pub transaction: String,
    /// Human-readable message shown in the wallet confirmation UI.
    pub message: String,
}

/// Query parameters for checking deposit confirmation.
#[derive(Debug, serde::Deserialize)]
pub struct ConfirmDepositQuery {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
}

/// Response for deposit confirmation check.
#[derive(Debug, serde::Serialize)]
pub struct ConfirmDepositResponse {
    /// Whether the deposit has been confirmed on-chain.
    pub confirmed: bool,
    /// Transaction signature if confirmed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_signature: Option<String>,
    /// Solana Pay URL to retry (if not yet confirmed and TX not sent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solana_pay_url: Option<String>,
}

/// Helius webhook payload for transaction notification.
/// See: https://docs.helius.dev/webhooks/webhook-payload
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct HeliusWebhookPayload {
    /// Array of transaction notifications.
    #[serde(default)]
    pub data: Vec<HeliusTransactionData>,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct HeliusTransactionData {
    /// Transaction signature.
    pub signature: String,
    /// Transaction type.
    #[serde(default)]
    pub r#type: String,
    /// Description (human-readable).
    #[serde(default)]
    pub description: String,
}

/// Response body for updating deposit status with TX signature.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UpdateDepositSignatureRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
    /// On-chain transaction signature.
    pub tx_signature: String,
}

/// Outcome of an on-chain TX verification with signer cross-check.
///
/// Returned by [`verify_tx_with_signer`] so callers can distinguish between:
/// - **Confirmed (matched)** — TX verified AND signer matches expected wallet,
///   safe to mark deposit as verified.
/// - **Confirmed (mismatch)** — TX verified but signer is NOT the expected
///   attendee wallet — caller should refuse verification (impersonation guard).
/// - **Pending** — TX not found or not yet confirmed, keep polling.
/// - **RpcError** — RPC infrastructure failure (timeout, network, parse);
///   callers should keep polling but log it differently, since this is a
///   transient infrastructure issue rather than a TX-state issue.
///
/// Callers should gate verification on [`Self::is_confirmed_and_matched`] to
/// close the impersonation gap where a malicious user submits someone else's
/// TX signature to get verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VerifyWithSignerOutcome {
    /// TX confirmed/finalized on-chain. `signer_matched` indicates whether the
    /// extracted fee-payer (`message.accountKeys[0]`) matched the expected
    /// attendee wallet. The caller decides how to act on a mismatch.
    Confirmed {
        /// Whether the actual signer matched the expected attendee wallet.
        signer_matched: bool,
        /// The actual signer (base58 pubkey) extracted from the TX.
        signer: String,
    },
    /// TX not found yet, or found but not yet confirmed — caller should keep polling.
    Pending,
    /// RPC infrastructure failure (timeout, network, parse) — caller should retry.
    RpcError,
}

impl VerifyWithSignerOutcome {
    /// Returns `true` only when the TX is confirmed on-chain (regardless of signer match).
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed { .. })
    }

    /// Returns `true` only when the TX is confirmed AND the signer matched
    /// the expected attendee wallet. This is the gate that deposit verification
    /// should use before marking the deposit as verified.
    pub fn is_confirmed_and_matched(&self) -> bool {
        matches!(
            self,
            Self::Confirmed {
                signer_matched: true,
                ..
            }
        )
    }

    /// Returns the extracted signer address, if confirmed.
    pub fn signer(&self) -> Option<&str> {
        match self {
            Self::Confirmed { signer, .. } => Some(signer.as_str()),
            _ => None,
        }
    }
}
