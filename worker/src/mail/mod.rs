//! Outbound mail from bethere.sol@gmail.com through the Gmail API (.plans/045
//! R4.12; owner 2026-10-08, the free path: Cloudflare Email Sending needs
//! Workers Paid and a domain). `message` builds the RFC 5322 text (pure,
//! tested natively); `gmail` refreshes the token and sends it.

pub mod gmail;
pub mod message;
