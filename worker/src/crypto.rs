//! SubtleCrypto bridge for RSA-SHA256 signing and HMAC-SHA256 JWT operations.
//!
//! Replaces the `rsa` + `jsonwebtoken` crates which don't compile to `wasm32-unknown-unknown`.
//! Uses the Workers runtime's built-in `crypto.subtle` (V8 SubtleCrypto) via `wasm-bindgen`.

use base64::Engine;
use js_sys::{ArrayBuffer, Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use event_checkin_domain::models::auth::Claims;

// ---------------------------------------------------------------------------
// Helpers: access crypto.subtle in the Workers runtime
// ---------------------------------------------------------------------------

/// Get the global `crypto.subtle` object.
fn get_subtle_crypto() -> Result<Object, String> {
    let global = js_sys::global();

    let crypto_val = Reflect::get(&global, &JsValue::from_str("crypto"))
        .map_err(|e| format!("failed to get global crypto: {e:?}"))?;

    let subtle_val = Reflect::get(&crypto_val, &JsValue::from_str("subtle"))
        .map_err(|e| format!("failed to get crypto.subtle: {e:?}"))?;

    Object::try_from(&subtle_val)
        .cloned()
        .ok_or_else(|| "crypto.subtle is not an object".to_string())
}

/// Convert a JS Uint8Array or ArrayBuffer into a Rust Vec<u8>.
fn js_buffer_to_vec(val: &JsValue) -> Result<Vec<u8>, String> {
    if val.is_instance_of::<ArrayBuffer>() {
        let arr = ArrayBuffer::from(val.clone());
        let view = Uint8Array::new(&arr);
        let mut buf = vec![0u8; view.length() as usize];
        view.copy_to(&mut buf);
        return Ok(buf);
    }

    if val.is_instance_of::<Uint8Array>() {
        let view = Uint8Array::from(val.clone());
        let mut buf = vec![0u8; view.length() as usize];
        view.copy_to(&mut buf);
        return Ok(buf);
    }

    Err("expected ArrayBuffer or Uint8Array".to_string())
}

/// URL-safe Base64 encoding (no padding).
fn base64_url_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// URL-safe Base64 decoding (no padding).
fn base64_url_decode(s: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|e| format!("base64 decode failed: {e}"))
}

/// Parse a PEM-encoded RSA private key and return the DER bytes.
fn pem_to_der(pem_str: &str) -> Result<Vec<u8>, String> {
    let normalized = pem_str.replace("\\n", "\n").replace("\\r", "\r");

    let begin_marker = "-----BEGIN PRIVATE KEY-----";
    let end_marker = "-----END PRIVATE KEY-----";

    let start = normalized
        .find(begin_marker)
        .ok_or_else(|| "PEM: missing BEGIN PRIVATE KEY marker".to_string())?
        + begin_marker.len();
    let end = normalized
        .find(end_marker)
        .ok_or_else(|| "PEM: missing END PRIVATE KEY marker".to_string())?;

    let b64_content: String = normalized[start..end]
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();

    base64::engine::general_purpose::STANDARD
        .decode(b64_content)
        .map_err(|e| format!("failed to decode PEM base64: {e}"))
}

/// Helper: call a SubtleCrypto method by name with positional args, await the result.
async fn subtle_call(method: &str, args: &[JsValue]) -> Result<JsValue, String> {
    let subtle = get_subtle_crypto()?;

    let fn_val = Reflect::get(&subtle, &JsValue::from_str(method))
        .map_err(|e| format!("subtle.{method} not found: {e:?}"))?;

    let fn_js = js_sys::Function::from(fn_val);

    let result = match args.len() {
        0 => fn_js.call0(&subtle),
        1 => fn_js.call1(&subtle, &args[0]),
        2 => fn_js.call2(&subtle, &args[0], &args[1]),
        3 => fn_js.call3(&subtle, &args[0], &args[1], &args[2]),
        4 => fn_js.call4(&subtle, &args[0], &args[1], &args[2], &args[3]),
        5 => fn_js.call5(&subtle, &args[0], &args[1], &args[2], &args[3], &args[4]),
        _ => return Err(format!("subtle_call: too many args ({})", args.len())),
    }
    .map_err(|e| format!("subtle.{method}() call failed: {e:?}"))?;

    JsFuture::from(js_sys::Promise::from(result))
        .await
        .map_err(|e| format!("subtle.{method}() promise rejected: {e:?}"))
}

// ---------------------------------------------------------------------------
// RSA-SHA256 signing (for Google service account JWT assertion)
// ---------------------------------------------------------------------------

/// Import an RSA PKCS#8 private key into SubtleCrypto for signing.
async fn import_rsa_private_key(der: &[u8]) -> Result<JsValue, String> {
    let key_data = Uint8Array::new_with_length(der.len() as u32);
    key_data.copy_from(der);

    // { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" }
    let algorithm = Object::new();
    Reflect::set(
        &algorithm,
        &JsValue::from_str("name"),
        &JsValue::from_str("RSASSA-PKCS1-v1_5"),
    )
    .map_err(|e| format!("importKey: failed to set name: {e:?}"))?;
    Reflect::set(
        &algorithm,
        &JsValue::from_str("hash"),
        &JsValue::from_str("SHA-256"),
    )
    .map_err(|e| format!("importKey: failed to set hash: {e:?}"))?;

    let usages = js_sys::Array::new();
    usages.push(&JsValue::from_str("sign"));

    // subtle.importKey("pkcs8", keyData, algorithm, false, ["sign"])
    subtle_call(
        "importKey",
        &[
            JsValue::from_str("pkcs8"),
            key_data.into(),
            algorithm.into(),
            JsValue::from_bool(false),
            usages.into(),
        ],
    )
    .await
}

/// Sign data with RSA-SHA256 (RS256) using a PEM-encoded private key.
///
/// Returns the raw signature bytes.
pub async fn sign_rs256(private_key_pem: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    let der = pem_to_der(private_key_pem)?;
    let key = import_rsa_private_key(&der).await?;

    let data_arr = Uint8Array::new_with_length(data.len() as u32);
    data_arr.copy_from(data);

    // { name: "RSASSA-PKCS1-v1_5" }
    let algorithm = Object::new();
    Reflect::set(
        &algorithm,
        &JsValue::from_str("name"),
        &JsValue::from_str("RSASSA-PKCS1-v1_5"),
    )
    .map_err(|e| format!("sign: failed to set name: {e:?}"))?;

    // subtle.sign(algorithm, key, data)
    let sig_buf = subtle_call("sign", &[algorithm.into(), key, data_arr.into()]).await?;

    js_buffer_to_vec(&sig_buf)
}

/// Build a signed JWT assertion for Google service account authentication.
///
/// Takes the base64url-encoded header and payload, signs the `header.payload`
/// string with RS256, and returns the full JWT `header.payload.signature`.
pub async fn sign_jwt_assertion(
    header_b64: &str,
    payload_b64: &str,
    private_key_pem: &str,
) -> Result<String, String> {
    let sign_input = format!("{header_b64}.{payload_b64}");
    let signature = sign_rs256(private_key_pem, sign_input.as_bytes()).await?;
    let sig_b64 = base64_url_encode(&signature);
    Ok(format!("{sign_input}.{sig_b64}"))
}

// ---------------------------------------------------------------------------
// HMAC-SHA256 (for JWT session tokens)
// ---------------------------------------------------------------------------

/// Import an HMAC-SHA256 key into SubtleCrypto.
async fn import_hmac_key(key_bytes: &[u8]) -> Result<JsValue, String> {
    let key_data = Uint8Array::new_with_length(key_bytes.len() as u32);
    key_data.copy_from(key_bytes);

    // { name: "HMAC", hash: "SHA-256" }
    let algorithm = Object::new();
    Reflect::set(
        &algorithm,
        &JsValue::from_str("name"),
        &JsValue::from_str("HMAC"),
    )
    .map_err(|e| format!("importHMACKey: failed to set name: {e:?}"))?;
    Reflect::set(
        &algorithm,
        &JsValue::from_str("hash"),
        &JsValue::from_str("SHA-256"),
    )
    .map_err(|e| format!("importHMACKey: failed to set hash: {e:?}"))?;

    let usages = js_sys::Array::new();
    usages.push(&JsValue::from_str("sign"));
    usages.push(&JsValue::from_str("verify"));

    // subtle.importKey("raw", keyData, algorithm, false, ["sign","verify"])
    subtle_call(
        "importKey",
        &[
            JsValue::from_str("raw"),
            key_data.into(),
            algorithm.into(),
            JsValue::from_bool(false),
            usages.into(),
        ],
    )
    .await
}

/// Compute HMAC-SHA256 of the given data using the provided key.
async fn hmac_sha256(key_bytes: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let key = import_hmac_key(key_bytes).await?;

    let data_arr = Uint8Array::new_with_length(data.len() as u32);
    data_arr.copy_from(data);

    // { name: "HMAC" }
    let algorithm = Object::new();
    Reflect::set(
        &algorithm,
        &JsValue::from_str("name"),
        &JsValue::from_str("HMAC"),
    )
    .map_err(|e| format!("hmac sign: failed to set name: {e:?}"))?;

    // subtle.sign(algorithm, key, data)
    let mac_buf = subtle_call("sign", &[algorithm.into(), key, data_arr.into()]).await?;

    js_buffer_to_vec(&mac_buf)
}

/// Verify HMAC-SHA256 of the given data using the provided key.
#[allow(dead_code)]
async fn hmac_sha256_verify(
    key_bytes: &[u8],
    data: &[u8],
    signature: &[u8],
) -> Result<bool, String> {
    let key = import_hmac_key(key_bytes).await?;

    let data_arr = Uint8Array::new_with_length(data.len() as u32);
    data_arr.copy_from(data);

    let sig_arr = Uint8Array::new_with_length(signature.len() as u32);
    sig_arr.copy_from(signature);

    // { name: "HMAC" }
    let algorithm = Object::new();
    Reflect::set(
        &algorithm,
        &JsValue::from_str("name"),
        &JsValue::from_str("HMAC"),
    )
    .map_err(|e| format!("hmac verify: failed to set name: {e:?}"))?;

    // subtle.verify(algorithm, key, signature, data)
    let verified = subtle_call(
        "verify",
        &[algorithm.into(), key, sig_arr.into(), data_arr.into()],
    )
    .await?;

    Ok(verified.is_truthy())
}

// ---------------------------------------------------------------------------
// JWT session token create/verify (HS256 using HMAC-SHA256)
// ---------------------------------------------------------------------------

/// JWT header for HS256 tokens.
const JWT_HEADER_B64: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";

/// Create a JWT session token for an authenticated staff member using HMAC-SHA256.
///
/// This replaces `jsonwebtoken::encode` from the Axum build.
pub async fn create_jwt(email: &str, sub: &str, secret: &str) -> Result<String, String> {
    let claims = Claims::new(email.to_string(), sub.to_string());
    let payload_bytes =
        serde_json::to_vec(&claims).map_err(|e| format!("failed to serialize JWT claims: {e}"))?;
    let payload_b64 = base64_url_encode(&payload_bytes);

    let sign_input = format!("{JWT_HEADER_B64}.{payload_b64}");
    let signature = hmac_sha256(secret.as_bytes(), sign_input.as_bytes()).await?;
    let sig_b64 = base64_url_encode(&signature);

    Ok(format!("{sign_input}.{sig_b64}"))
}

/// Verify and decode a JWT session token, returning the claims.
///
/// This replaces `jsonwebtoken::decode` from the Axum build.
pub async fn verify_jwt(token: &str, secret: &str) -> Result<Claims, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("invalid JWT format: expected 3 parts".to_string());
    }

    // Verify header
    if parts[0] != JWT_HEADER_B64 {
        return Err("invalid JWT header: expected HS256".to_string());
    }

    // Compute expected signature
    let sign_input = format!("{}.{}", parts[0], parts[1]);
    let expected_sig = hmac_sha256(secret.as_bytes(), sign_input.as_bytes()).await?;
    let actual_sig = base64_url_decode(parts[2])?;

    // Constant-time comparison of signature bytes
    if expected_sig.len() != actual_sig.len() {
        return Err("JWT signature verification failed".to_string());
    }

    let mut diff = 0u8;
    for (a, b) in expected_sig.iter().zip(actual_sig.iter()) {
        diff |= a ^ b;
    }
    if diff != 0 {
        return Err("JWT signature verification failed".to_string());
    }

    // Decode payload
    let payload_bytes = base64_url_decode(parts[1])?;
    let claims: Claims = serde_json::from_slice(&payload_bytes)
        .map_err(|e| format!("failed to deserialize JWT claims: {e}"))?;

    // Check expiration
    let now = chrono::Utc::now().timestamp() as u64;
    if claims.exp < now {
        return Err("JWT token has expired".to_string());
    }

    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_url_encode_decode_roundtrip() {
        let data = b"hello world 12345!@#$%";
        let encoded = base64_url_encode(data);
        let decoded = base64_url_decode(&encoded).unwrap();
        assert_eq!(data.as_slice(), decoded.as_slice());
    }

    #[test]
    fn test_base64_url_encode_no_padding() {
        // 16 bytes → no padding needed
        let data = [0xAAu8; 16];
        let encoded = base64_url_encode(&data);
        assert!(!encoded.contains('='));

        // 17 bytes → would normally have padding
        let data = [0xBBu8; 17];
        let encoded = base64_url_encode(&data);
        assert!(!encoded.contains('='));
    }

    #[test]
    fn test_pem_to_der_valid() {
        // A minimal valid PEM for testing parsing logic
        let pem = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEINTuctv5AzsmYJNPdBR5EExUuqCVdJXSHBAv6WMC\n-----END PRIVATE KEY-----";
        let result = pem_to_der(pem);
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_pem_to_der_escaped_newlines() {
        let pem = "-----BEGIN PRIVATE KEY-----\\nMC4CAQAwBQYDK2VwBCIEINTuctv5AzsmYJNPdBR5EExUuqCVdJXSHBAv6WMC\\n-----END PRIVATE KEY-----";
        let result = pem_to_der(pem);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pem_to_der_missing_begin() {
        let result = pem_to_der("not a pem");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("BEGIN PRIVATE KEY"));
    }

    #[test]
    fn test_pem_to_der_missing_end() {
        let pem = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEINTuctv5AzsmYJNPdBR5EExUuqCVdJXSHBAv6WMC";
        let result = pem_to_der(pem);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("END PRIVATE KEY"));
    }

    #[test]
    fn test_jwt_header_b64_decodes_correctly() {
        let header_bytes = base64_url_decode(JWT_HEADER_B64).unwrap();
        let header: serde_json::Value = serde_json::from_slice(&header_bytes).unwrap();
        assert_eq!(header["alg"], "HS256");
        assert_eq!(header["typ"], "JWT");
    }
}
