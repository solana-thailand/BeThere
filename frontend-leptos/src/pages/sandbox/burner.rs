//! Bridge to `js/sandbox_burner.js`: the devnet burner wallet.

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/js/sandbox_burner.js")]
extern "C" {
    #[wasm_bindgen(js_name = "burnerAddress")]
    fn burner_address_js() -> js_sys::Promise;

    #[wasm_bindgen(js_name = "burnerSignAndSend")]
    fn burner_sign_and_send_js(rpc_url: &str, transaction_b64: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = "burnerSupported")]
    fn burner_supported_js() -> js_sys::Promise;

    #[wasm_bindgen(js_name = "forgetBurner")]
    fn forget_burner_js();

    #[wasm_bindgen(js_name = "confirmSignature")]
    fn confirm_signature_js(rpc_url: &str, signature: &str) -> js_sys::Promise;
}

/// The text of a rejected promise: an `Error`'s message, or the value itself.
fn rejection(value: JsValue) -> String {
    js_sys::Reflect::get(&value, &JsValue::from_str("message"))
        .ok()
        .and_then(|m| m.as_string())
        .or_else(|| value.as_string())
        .unwrap_or_else(|| "the wallet could not finish".to_string())
}

async fn string_of(promise: js_sys::Promise) -> Result<String, String> {
    let value = JsFuture::from(promise).await.map_err(rejection)?;
    value
        .as_string()
        .ok_or_else(|| "the wallet returned nothing".to_string())
}

/// The burner's address, making the burner on first use.
pub async fn burner_address() -> Result<String, String> {
    string_of(burner_address_js()).await
}

/// Sign the Worker-built transaction with the burner, send it, and wait for
/// `confirmed`. Returns the signature.
pub async fn burner_sign_and_send(rpc_url: &str, transaction_b64: &str) -> Result<String, String> {
    string_of(burner_sign_and_send_js(rpc_url, transaction_b64)).await
}

/// Whether this browser has WebCrypto Ed25519.
pub async fn burner_supported() -> bool {
    JsFuture::from(burner_supported_js())
        .await
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Drop the burner; the next call makes a new one.
pub fn forget_burner() {
    forget_burner_js();
}

/// Wait for a signature a real wallet sent to reach `confirmed`.
pub async fn confirm_signature(rpc_url: &str, signature: &str) -> Result<String, String> {
    string_of(confirm_signature_js(rpc_url, signature)).await
}
