//! Notification enrollment must consume the signed verification attestation,
//! not a wallet→email lookup or the legacy credit identity heuristic.
#[test]
fn only_verified_google_callback_can_mint_email_attestation() {
    let callback = include_str!("../src/handlers/auth.rs");
    assert_eq!(callback.matches("create_verified_email_jwt(").count(), 1);
    let issuer = callback.find("create_verified_email_jwt(").unwrap();
    assert!(callback[..issuer].contains("auth::handle_callback(&code, &state).await"));
    let validator = include_str!("../src/auth.rs");
    assert!(validator.contains("validate_google_identity(&user_info)?"));
    let wallet_flow = callback.split("pub async fn wallet_verify").last().unwrap();
    assert!(!wallet_flow.contains("create_verified_email_jwt("));
}

#[test]
fn registration_uses_attestation_instead_of_wallet_link() {
    let signup = include_str!("../src/handlers/register/signup.rs");
    let write = signup
        .split("crate::db::attendees::upsert_attendee(")
        .nth(1)
        .unwrap()
        .split(").await")
        .next()
        .unwrap();
    // rustfmt puts .await on its own line; bound the inspected call at that token.
    let write = write.split(".await").next().unwrap();
    assert!(write.contains("claims.email_verified"));
    assert!(!write.contains("credit_identity_ok"));
}
