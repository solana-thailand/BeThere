use super::content::Message;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::Env;

pub struct Sender {
    binding: JsValue,
    send: js_sys::Function,
    from: String,
    pub base_url: String,
}
impl Sender {
    pub fn from_env(env: &Env) -> Result<Self, String> {
        let from = env
            .var("NOTIFICATION_FROM")
            .map_err(|_| "NOTIFICATION_FROM missing")?
            .to_string();
        if !event_checkin_domain::validation::is_plausible_email(&from) {
            return Err("NOTIFICATION_FROM invalid".into());
        }
        let base_url = env
            .var("SERVER_URL")
            .map_err(|_| "SERVER_URL missing")?
            .to_string();
        let url = worker::Url::parse(&base_url).map_err(|_| "SERVER_URL invalid")?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || url.path() != "/"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("SERVER_URL must be an HTTPS origin".into());
        }
        let binding = js_sys::Reflect::get(env, &JsValue::from_str("EMAIL"))
            .map_err(|_| "EMAIL binding missing")?;
        let send = js_sys::Reflect::get(&binding, &JsValue::from_str("send"))
            .map_err(|_| "EMAIL.send missing")?
            .dyn_into::<js_sys::Function>()
            .map_err(|_| "EMAIL.send invalid")?;
        Ok(Self {
            binding,
            send,
            from,
            base_url: base_url.trim_end_matches('/').into(),
        })
    }
    pub async fn send(&self, message: &Message) -> Result<String, String> {
        let mut body = serde_json::to_value(message).map_err(|_| "E_VALIDATION_ERROR")?;
        body["from"] = serde_json::json!({"email":self.from,"name":"BeThere"});
        let payload = js_sys::JSON::parse(&body.to_string()).map_err(|_| "E_VALIDATION_ERROR")?;
        let promise = self
            .send
            .call1(&self.binding, &payload)
            .map_err(error_code)?
            .dyn_into::<js_sys::Promise>()
            .map_err(|_| "UNKNOWN")?;
        let result = JsFuture::from(promise).await.map_err(error_code)?;
        js_sys::Reflect::get(&result, &JsValue::from_str("messageId"))
            .ok()
            .and_then(|v| v.as_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "UNKNOWN".into())
    }
}
fn error_code(e: JsValue) -> String {
    // Store only recognized code-shaped values, never raw provider errors with PII.
    js_sys::Reflect::get(&e, &JsValue::from_str("code"))
        .ok()
        .and_then(|v| v.as_string())
        .filter(|s| {
            s.len() < 80
                && s.starts_with("E_")
                && s.chars().all(|c| c.is_ascii_uppercase() || c == '_')
        })
        .unwrap_or_else(|| "UNKNOWN".into())
}
