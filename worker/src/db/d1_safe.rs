//! Safe D1 query helpers that bypass the worker crate's `results()` panic on NULL values.
//!
//! The worker crate (0.8.x) `D1Result::results()` calls
//! `serde_wasm_bindgen::from_value(row).unwrap()` for each row. When a column
//! value is SQL `NULL` (JS `null`), `serde_wasm_bindgen` rejects it with
//! "invalid type: JsValue(null)" and the `.unwrap()` panics — killing the worker
//! with a bare 500 (HTML error page, not JSON).
//!
//! These helpers use raw JS interop + `JSON.stringify` (the same workaround
//! already used in `get_thb_deposit` / `get_deposit_status`) to safely extract
//! rows as `serde_json::Value`, which handles nulls gracefully.

use worker::D1PreparedStatement;
use worker::d1::D1Type;

/// Run a bound D1 statement and return all rows as `serde_json::Value`.
///
/// This bypasses the worker crate's `results()` to avoid the NULL-value panic.
/// Pass a freshly-prepared, already-bound statement.
pub async fn safe_all_rows(stmt: &D1PreparedStatement) -> Result<Vec<serde_json::Value>, String> {
    // Call `.all()` on the inner JS prepared statement directly.
    let raw_result = wasm_bindgen_futures::JsFuture::from(
        stmt.inner()
            .all()
            .map_err(|e| format!("D1 safe_all_rows all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 safe_all_rows all() await: {e:?}"))?;

    // The D1 result object has a `results` property (a JS Array of row objects).
    let key = wasm_bindgen::JsValue::from("results");
    let results_val = js_sys::Reflect::get(&raw_result, &key)
        .map_err(|e| format!("D1 safe_all_rows get results: {e:?}"))?;

    if results_val.is_null() || results_val.is_undefined() {
        return Ok(Vec::new());
    }

    let results_arr = js_sys::Array::from(&results_val);
    let mut rows = Vec::with_capacity(results_arr.length() as usize);

    for row_js in results_arr.iter() {
        if row_js.is_null() || row_js.is_undefined() {
            continue;
        }
        let json_str = js_sys::JSON::stringify(&row_js)
            .map(|s| s.as_string().unwrap_or_default())
            .unwrap_or_default();

        if json_str.is_empty() {
            continue;
        }

        let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            tracing::warn!(
                error = %e,
                json = %json_str.chars().take(300).collect::<String>(),
                "D1 safe_all_rows: deserialize failed"
            );
            format!("D1 safe_all_rows deserialize: {e}")
        })?;

        rows.push(row);
    }

    Ok(rows)
}

/// Convenience: prepare + bind + safe_all_rows for a single-text-parameter query.
#[allow(dead_code)]
pub async fn query_rows_by_text(
    db: &worker::D1Database,
    sql: &str,
    param: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let stmt = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(param)])
        .map_err(|e| format!("D1 query_rows_by_text bind: {e:?}"))?;
    safe_all_rows(&stmt).await
}
