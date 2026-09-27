//! CSV export of the attendee roster.

use crate::api::AttendeeListItem;

/// Generate CSV content from a filtered attendee list.
pub(super) fn generate_csv(attendees: &[AttendeeListItem]) -> String {
    let mut csv = String::from(
        "Name,Email,Ticket,Participation,Status,Checked In At,Checked In By,API ID,Deposit Status,Deposit Amount,Deposit TX,NFT,Refund Status,Attendance Answer\n",
    );
    for a in attendees {
        let status = if a.checked_in_at.is_some() {
            "Checked In"
        } else {
            "Pending"
        };
        let checked_at = a.checked_in_at.as_deref().unwrap_or("");
        let checked_by = a.checked_in_by.as_deref().unwrap_or("");
        let deposit_status = a.deposit_status.as_deref().unwrap_or("");
        let deposit_amount = a.deposit_amount.as_deref().unwrap_or("");
        let deposit_tx = a.deposit_tx_signature.as_deref().unwrap_or("");
        let nft = if a.nft_proof_url.is_some() { "Yes" } else { "" };
        let refund_status = a.refund_status.as_deref().unwrap_or("");
        let answer = a.attendance_answer.map_or("", |x| x.label());
        // Escape CSV fields containing commas or quotes
        let escape = |s: &str| -> String {
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        };
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            escape(&a.name),
            escape(&a.email),
            escape(&a.ticket_name),
            escape(&a.participation_type),
            status,
            escape(checked_at),
            escape(checked_by),
            escape(&a.api_id),
            deposit_status,
            deposit_amount,
            escape(deposit_tx),
            nft,
            refund_status,
            escape(answer),
        ));
    }
    csv
}

/// Trigger CSV file download in browser using proper web_sys APIs.
pub(crate) fn download_csv(filename: &str, content: &str) {
    use js_sys::{Array, Uint8Array};

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    // Encode CSV content as UTF-8 bytes
    let bytes = content.as_bytes();
    let uint8 = Uint8Array::new_with_length(bytes.len() as u32);
    uint8.copy_from(bytes);

    // Create Blob from byte array
    let parts = Array::new();
    parts.push(&uint8.buffer());

    let blob_options = web_sys::BlobPropertyBag::new();
    blob_options.set_type("text/csv;charset=utf-8;");

    let blob = match web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &blob_options) {
        Ok(b) => b,
        Err(_) => return,
    };

    // Create object URL
    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(_) => return,
    };

    // Create temporary <a> element, trigger click, cleanup
    if let Ok(a) = document.create_element("a") {
        let _ = a.set_attribute("href", &url);
        let _ = a.set_attribute("download", filename);
        if let Some(body) = document.body() {
            let _ = body.append_child(&a);
            // Cast Element → HtmlElement for .click()
            use wasm_bindgen::JsCast;
            a.unchecked_ref::<web_sys::HtmlElement>().click();
            let _ = body.remove_child(&a);
        }
    }

    web_sys::Url::revoke_object_url(&url).unwrap_or(());
}
