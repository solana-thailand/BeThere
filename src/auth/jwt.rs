use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};

use crate::models::auth::Claims;

/// Create a JWT token for an authenticated staff member.
pub fn create_jwt(
    email: &str,
    sub: &str,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = Claims::new(email.to_string(), sub.to_string());
    let encoding_key = EncodingKey::from_secret(secret.as_bytes());
    jsonwebtoken::encode(&Header::default(), &claims, &encoding_key)
}

/// Verify and decode a JWT token, returning the claims.
pub fn verify_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let decoding_key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::default();
    let token_data = jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation)?;
    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_verify_jwt() {
        let secret = "test-secret-key";
        let email = "staff@example.com";
        let sub = "google-user-id-123";

        let token = create_jwt(email, sub, secret).unwrap();
        let claims = verify_jwt(&token, secret).unwrap();

        assert_eq!(claims.email, email);
        assert_eq!(claims.sub, sub);
        assert!(!claims.is_expired());
    }

    #[test]
    fn test_verify_invalid_token() {
        let secret = "test-secret-key";
        let result = verify_jwt("invalid.token.here", secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_wrong_secret() {
        let secret = "correct-secret";
        let wrong_secret = "wrong-secret";
        let token = create_jwt("test@test.com", "sub123", secret).unwrap();
        let result = verify_jwt(&token, wrong_secret);
        assert!(result.is_err());
    }
}
