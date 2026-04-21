use base64::Engine;
use qrcode::QrCode;

use crate::models::attendee::Attendee;

/// Generate a QR code as a base64-encoded SVG data URI.
/// Returns a string like `data:image/svg+xml;base64,...`
pub fn generate_qr_base64(data: &str) -> Result<String, String> {
    let code =
        QrCode::new(data.as_bytes()).map_err(|e| format!("failed to create qr code: {e}"))?;

    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(256, 256)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .quiet_zone(false)
        .build();

    let encoded = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
    Ok(format!("data:image/svg+xml;base64,{encoded}"))
}

/// Build a check-in URL for a given attendee API ID.
/// Format: `{server_url}/staff.html?scan={api_id}`
pub fn build_checkin_url(api_id: &str, server_url: &str) -> String {
    format!("{server_url}/staff.html?scan={api_id}")
}

/// Generate QR code URLs for all approved attendees that don't have one yet.
/// Returns a list of (row_index, qr_code_url) tuples ready for batch update.
///
/// If `force` is true, regenerates URLs even for attendees that already have one.
pub fn generate_qr_urls(
    attendees: &[Attendee],
    server_url: &str,
    force: bool,
) -> Vec<(usize, String)> {
    attendees
        .iter()
        .filter(|a| a.is_approved())
        .filter(|a| {
            force
                || a.qr_code_url.is_none()
                || a.qr_code_url
                    .as_ref()
                    .is_some_and(|u: &String| u.is_empty())
        })
        .map(|a: &Attendee| {
            let url = build_checkin_url(&a.api_id, server_url);
            (a.row_index, url)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::attendee::CheckInStatus;

    fn mock_attendee(api_id: &str, status: CheckInStatus, qr_url: Option<&str>) -> Attendee {
        Attendee {
            api_id: api_id.to_string(),
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            ticket_name: "General".to_string(),
            approval_status: status,
            checked_in_at: None,
            qr_code_url: qr_url.map(|s| s.to_string()),
            solana_address: None,
            participation_type: "In-Person".to_string(),
            row_index: 2,
        }
    }

    #[test]
    fn test_build_checkin_url() {
        let url = build_checkin_url("gst-abc123", "https://checkin.example.com");
        assert_eq!(
            url,
            "https://checkin.example.com/staff.html?scan=gst-abc123"
        );
    }

    #[test]
    fn test_generate_qr_base64_returns_data_uri() {
        let result = generate_qr_base64("https://example.com/checkin?id=123");
        assert!(result.is_ok());
        let uri = result.unwrap();
        assert!(uri.starts_with("data:image/svg+xml;base64,"));
        assert!(uri.len() > 100);
    }

    #[test]
    fn test_generate_qr_urls_only_approved() {
        let attendees = vec![
            mock_attendee("id-1", CheckInStatus::Approved, None),
            mock_attendee("id-2", CheckInStatus::PendingApproval, None),
            mock_attendee("id-3", CheckInStatus::Approved, None),
            mock_attendee("id-4", CheckInStatus::Invited, None),
        ];

        let urls = generate_qr_urls(&attendees, "https://example.com", false);
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn test_generate_qr_urls_skips_existing() {
        let attendees = vec![
            mock_attendee("id-1", CheckInStatus::Approved, None),
            mock_attendee("id-2", CheckInStatus::Approved, Some("https://old-url.com")),
        ];

        let urls = generate_qr_urls(&attendees, "https://example.com", false);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].0, 2);
    }

    #[test]
    fn test_generate_qr_urls_force_regenerates() {
        let attendees = vec![
            mock_attendee("id-1", CheckInStatus::Approved, None),
            mock_attendee("id-2", CheckInStatus::Approved, Some("https://old-url.com")),
        ];

        let urls = generate_qr_urls(&attendees, "https://example.com", true);
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn test_generate_qr_urls_skips_empty_string() {
        let attendees = vec![mock_attendee("id-1", CheckInStatus::Approved, Some(""))];

        let urls = generate_qr_urls(&attendees, "https://example.com", false);
        assert_eq!(urls.len(), 1);
    }
}
